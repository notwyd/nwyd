use super::*;
use crate::frame::zone_upload::Budget;
use formats::texture::{MipLevel, PixelFormat, Texture, texture_list::FormatFlag};
use render::quirk::VendorClass;
use std::time::{Duration, Instant};

fn material(slot: usize, skin: bool) -> BoundTexture {
    let mut texture = BoundTexture {
        pending: false,
        effect_table: false,
        slot,
        path: format!("synthetic/{slot}.wys"),
        flag: FormatFlag::Alpha,
        texture: Texture {
            width: 2,
            height: 2,
            format: PixelFormat::Rgba8Unorm,
            data: [40, 180, 80, 128].repeat(4),
            source_len: 16,
            mips: vec![MipLevel {
                width: 2,
                height: 2,
                offset: 0,
                len: 16,
            }],
        }
        .into(),
        mip_policy_override: None,
        prepared: None,
    };
    texture.prepare_material(skin);
    texture
}

#[test]
#[ignore = "requires a compatible graphics adapter"]
fn duplicate_material_keys_share_the_uploaded_view() {
    with_gpu(|gpu, mesh, skin| {
        let mut first = material(0, false);
        first.prepared = None;
        let mut second = first.clone();
        second.slot = 1;
        let textures = [first, second];
        let mut cache = WorldTextures::default();
        cache.sync_meshes(gpu, mesh, &textures);
        drain(&mut cache, gpu, mesh, &textures, skin, &[], &[]);
        assert_eq!(cache.uploads, (1, 0));
        assert_eq!(cache.mesh_views[0], cache.mesh_views[1]);
        assert_eq!(cache.stats().pending, 0);
    });
}

fn with_gpu(test: impl FnOnce(&Gpu, &MeshRenderer, &SkinRenderer)) {
    let gpu = pollster::block_on(Gpu::open(Gpu::instance(), None)).unwrap();
    let validation = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mesh = MeshRenderer::new(
        &gpu,
        render::target::COLOR_FORMAT,
        render::target::DEPTH_FORMAT,
        VendorClass::DEFAULT,
    );
    let skin = SkinRenderer::new(
        &gpu,
        render::target::COLOR_FORMAT,
        render::target::DEPTH_FORMAT,
        VendorClass::DEFAULT,
    );
    test(&gpu, &mesh, &skin);
    gpu.queue.submit([]);
    assert!(pollster::block_on(validation.pop()).is_none());
}

#[allow(clippy::too_many_arguments)]
fn drain(
    cache: &mut WorldTextures,
    gpu: &Gpu,
    mesh: &MeshRenderer,
    meshes: &[BoundTexture],
    skin: &SkinRenderer,
    skins: &[BoundTexture],
    pairs: &[(usize, usize)],
) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while !cache.pending.queue.is_empty() {
        assert!(Instant::now() < deadline, "material uploads did not finish");
        cache.flush(gpu, mesh, meshes, skin, skins, pairs, &mut Budget::new());
        std::thread::yield_now();
    }
}

#[test]
fn texture_identity_distinguishes_renderer_alpha_and_mipmap_policy() {
    let texture = material(0, true);
    assert_ne!(key(&texture, true), key(&texture, false));
    let mut other = texture.clone();
    other.flag = FormatFlag::None_;
    assert_ne!(key(&texture, true), key(&other, true));
    other = texture.clone();
    other.mip_policy_override = Some(texture::MipPolicy::Cutout(170));
    assert_ne!(key(&texture, true), key(&other, true));
    assert!(other.prepared_material(true).is_none());
    other.prepare_material(true);
    assert_eq!(
        other.prepared_material(true).unwrap().policy(),
        texture::MipPolicy::Cutout(170)
    );
    assert_eq!(other.texture, texture.texture);
}

#[test]
fn pending_uploads_keep_fifo_order_and_deduplicate_each_renderer_slot() {
    let mut pending = PendingUploads::default();
    for key in [(false, 2), (true, 0), (false, 2), (false, 0), (true, 0)] {
        pending.push(key);
    }
    assert_eq!(
        pending.queue.iter().copied().collect::<Vec<_>>(),
        [(false, 2), (true, 0), (false, 0)]
    );
    assert_eq!(pending.queued.len(), 3);
    pending.pop();
    pending.push((false, 2));
    assert_eq!(
        pending.queue.iter().copied().collect::<Vec<_>>(),
        [(true, 0), (false, 0), (false, 2)]
    );
    for _ in 0..4 {
        pending.pop();
    }
    assert!(pending.queue.is_empty());
    assert!(pending.queued.is_empty());
}

#[test]
fn world_reset_cancels_queued_uploads_and_preserves_cumulative_statistics() {
    let mut cache = WorldTextures::default();
    cache.pending.push((false, 3));
    cache.pending.push((true, 1));
    cache.stats.uploads = 7;
    cache.stats.bytes = 2048;
    cache.stats.elapsed_ms = 1.5;
    cache.stats.longest_ms = 0.5;
    cache.clear();
    assert!(cache.pending.queue.is_empty());
    assert!(cache.pending.queued.is_empty());
    assert!(cache.resident.is_empty());
    assert_eq!(cache.stats().pending, 0);
    assert_eq!(cache.stats().uploads, 7);
    assert_eq!(cache.stats().bytes, 2048);
    assert_eq!(cache.stats().elapsed_ms, 1.5);
    assert_eq!(cache.stats().longest_ms, 0.5);
}

#[test]
#[ignore = "requires a compatible graphics adapter"]
fn staged_views_and_glow_indices_survive_appends_refreshes_and_world_reset() {
    with_gpu(|gpu, mesh, skin| {
        let meshes = [material(0, false)];
        let mut textures = vec![material(0, true), material(1, true)];
        let mut cache = WorldTextures::default();
        cache.stage(
            gpu,
            &meshes[0],
            false,
            meshes[0].prepared_material(false).unwrap(),
        );
        let staged = cache.staged.get(&key(&meshes[0], false)).unwrap().clone();
        cache.sync_meshes(gpu, mesh, &meshes);
        assert_eq!(cache.mesh_views[0], staged);
        assert_eq!(cache.uploads.0, 1);
        cache.sync_skins(gpu, skin, &textures, &[(0, 1)]);
        drain(&mut cache, gpu, mesh, &meshes, skin, &textures, &[(0, 1)]);
        let first = cache.skins[0].bind_group.clone();
        let second = cache.skins[1].bind_group.clone();
        let glow = cache.skins[2].bind_group.clone();
        textures.push(material(2, true));
        cache.sync_skins(gpu, skin, &textures, &[(0, 1), (1, 2)]);
        assert_eq!(cache.skins.len(), 5);
        assert_eq!(cache.skins[0].bind_group, first);
        assert_eq!(cache.skins[1].bind_group, second);
        assert_eq!(cache.skins[3].bind_group, glow);
        drain(
            &mut cache,
            gpu,
            mesh,
            &meshes,
            skin,
            &textures,
            &[(0, 1), (1, 2)],
        );
        let other_glow = cache.skins[4].bind_group.clone();
        cache.refresh_skins(gpu, skin, &textures, &[(0, 1), (1, 2)], &[0, 0], 3);
        drain(
            &mut cache,
            gpu,
            mesh,
            &meshes,
            skin,
            &textures,
            &[(0, 1), (1, 2)],
        );
        assert_ne!(cache.skins[0].bind_group, first);
        assert_eq!(cache.skins[1].bind_group, second);
        assert_ne!(cache.skins[3].bind_group, glow);
        assert_eq!(cache.skins[4].bind_group, other_glow);
        assert_eq!(cache.uploads, (1, 4));
        cache.clear();
        assert!(cache.meshes.is_empty() && cache.skins.is_empty());
        assert!(cache.staged.is_empty() && cache.resident.is_empty());
        assert!(cache.pending.queue.is_empty());
    });
}

#[test]
#[ignore = "requires a compatible graphics adapter"]
fn upload_byte_budget_defers_later_slots_and_eventually_replaces_every_placeholder() {
    with_gpu(|gpu, mesh, skin| {
        let meshes: Vec<_> = (0..3).map(|slot| material(slot, false)).collect();
        let mut cache = WorldTextures::default();
        cache.sync_meshes(gpu, mesh, &meshes);
        let placeholder = cache.mesh_views[0].clone();
        assert!(cache.mesh_views.iter().all(|view| *view == placeholder));
        assert_eq!(cache.stats().uploads, 0);
        for completed in 1..=3 {
            cache.flush(
                gpu,
                mesh,
                &meshes,
                skin,
                &[],
                &[],
                &mut Budget::with_limit(0),
            );
            assert_eq!(cache.stats().uploads, completed);
            assert_eq!(cache.stats().pending, 3 - completed);
            for (index, view) in cache.mesh_views.iter().enumerate() {
                assert_eq!(*view == placeholder, index >= completed);
                assert_eq!(
                    cache.resident.contains_key(&key(&meshes[index], false)),
                    index < completed
                );
            }
        }
        assert_eq!(
            cache.stats().bytes,
            meshes
                .iter()
                .map(|texture| texture.prepared_material(false).unwrap().upload_bytes() as u64)
                .sum::<u64>()
        );
    });
}

#[test]
#[ignore = "requires a compatible graphics adapter"]
fn queued_replacement_uploads_the_current_slot_only_once() {
    with_gpu(|gpu, mesh, skin| {
        let mut meshes = [material(0, false)];
        let old_key = key(&meshes[0], false);
        let mut cache = WorldTextures::default();
        cache.sync_meshes(gpu, mesh, &meshes);
        let placeholder = cache.mesh_views[0].clone();
        meshes[0] = material(7, false);
        cache.refresh_meshes(gpu, mesh, &meshes, &[0, 0], 1);
        assert_eq!(cache.stats().pending, 1);
        drain(&mut cache, gpu, mesh, &meshes, skin, &[], &[]);
        assert_ne!(cache.mesh_views[0], placeholder);
        assert!(cache.resident.contains_key(&key(&meshes[0], false)));
        assert!(!cache.resident.contains_key(&old_key));
        assert_eq!(cache.stats().uploads, 1);
    });
}

#[test]
#[ignore = "requires a compatible graphics adapter"]
fn skin_appends_keep_glow_pairs_correct_during_partial_uploads() {
    with_gpu(|gpu, mesh, skin| {
        let mut textures = vec![material(0, true), material(1, true)];
        let mut cache = WorldTextures::default();
        cache.sync_skins(gpu, skin, &textures, &[(0, 1)]);
        let placeholder = cache.skin_views[0].clone();
        let first_placeholder = cache.skins[0].bind_group.clone();
        let second_placeholder = cache.skins[1].bind_group.clone();
        let first_glow = cache.skins[2].bind_group.clone();
        cache.flush(
            gpu,
            mesh,
            &[],
            skin,
            &textures,
            &[(0, 1)],
            &mut Budget::with_limit(0),
        );
        let first = cache.skins[0].bind_group.clone();
        let first_glow_updated = cache.skins[2].bind_group.clone();
        assert_ne!(first, first_placeholder);
        assert_eq!(cache.skins[1].bind_group, second_placeholder);
        assert_ne!(first_glow_updated, first_glow);
        textures.push(material(2, true));
        let pairs = [(0, 1), (1, 2)];
        cache.sync_skins(gpu, skin, &textures, &pairs);
        assert_eq!(cache.skins.len(), 5);
        assert_eq!(cache.skins[0].bind_group, first);
        assert_eq!(cache.skins[1].bind_group, second_placeholder);
        assert_eq!(cache.skins[3].bind_group, first_glow_updated);
        let third_placeholder = cache.skins[2].bind_group.clone();
        let second_glow = cache.skins[4].bind_group.clone();
        cache.flush(
            gpu,
            mesh,
            &[],
            skin,
            &textures,
            &pairs,
            &mut Budget::with_limit(0),
        );
        let second = cache.skins[1].bind_group.clone();
        let first_glow_complete = cache.skins[3].bind_group.clone();
        let second_glow_partial = cache.skins[4].bind_group.clone();
        assert_eq!(cache.skins[0].bind_group, first);
        assert_ne!(second, second_placeholder);
        assert_eq!(cache.skins[2].bind_group, third_placeholder);
        assert_ne!(first_glow_complete, first_glow_updated);
        assert_ne!(second_glow_partial, second_glow);
        cache.flush(
            gpu,
            mesh,
            &[],
            skin,
            &textures,
            &pairs,
            &mut Budget::with_limit(0),
        );
        assert_eq!(cache.skins[0].bind_group, first);
        assert_eq!(cache.skins[1].bind_group, second);
        assert_ne!(cache.skins[2].bind_group, third_placeholder);
        assert_eq!(cache.skins[3].bind_group, first_glow_complete);
        assert_ne!(cache.skins[4].bind_group, second_glow_partial);
        assert!(cache.skin_views.iter().all(|view| *view != placeholder));
        assert_eq!(cache.stats().pending, 0);
        assert_eq!(cache.uploads.1, 3);
    });
}

#[test]
#[ignore = "requires a compatible graphics adapter"]
fn eviction_before_a_queued_refresh_restores_the_placeholder_without_an_upload() {
    with_gpu(|gpu, mesh, skin| {
        let mut meshes = [material(0, false)];
        let mut cache = WorldTextures::default();
        cache.sync_meshes(gpu, mesh, &meshes);
        let placeholder = cache.mesh_views[0].clone();
        drain(&mut cache, gpu, mesh, &meshes, skin, &[], &[]);
        assert_ne!(cache.mesh_views[0], placeholder);
        cache.refresh_meshes(gpu, mesh, &meshes, &[0], 1);
        meshes[0].texture = crate::scene::actors::texture_loading::placeholder().into();
        meshes[0].prepared = None;
        meshes[0].pending = true;
        cache.refresh_meshes(gpu, mesh, &meshes, &[0], 1);
        assert_eq!(cache.stats().pending, 1);
        drain(&mut cache, gpu, mesh, &meshes, skin, &[], &[]);
        assert_eq!(cache.mesh_views[0], placeholder);
        assert_eq!(cache.stats().uploads, 1);
        assert!(!cache.resident.contains_key(&key(&meshes[0], false)));
    });
}

#[test]
#[ignore = "requires a compatible graphics adapter"]
fn uncached_material_preparation_eventually_replaces_the_placeholder() {
    with_gpu(|gpu, mesh, skin| {
        let meshes = [material(0, true)];
        assert!(meshes[0].prepared_material(false).is_none());
        let original = meshes[0].texture.clone();
        let mut cache = WorldTextures::default();
        cache.sync_meshes(gpu, mesh, &meshes);
        let placeholder = cache.mesh_views[0].clone();
        drain(&mut cache, gpu, mesh, &meshes, skin, &[], &[]);
        assert_ne!(cache.mesh_views[0], placeholder);
        assert_eq!(cache.stats().uploads, 1);
        assert_eq!(meshes[0].texture, original);
        assert_eq!(
            cache.stats().bytes,
            texture::PreparedMaterial::new(&original, meshes[0].mip_policy(false)).upload_bytes()
                as u64
        );
    });
}

#[test]
#[ignore = "requires a compatible graphics adapter"]
fn redundant_preparation_does_not_block_later_uncached_materials() {
    with_gpu(|gpu, mesh, skin| {
        let cached = material(0, false);
        let mut uncached = cached.clone();
        uncached.prepared = None;
        let mut textures = vec![uncached, cached];
        let mut cache = WorldTextures::default();
        cache.sync_meshes(gpu, mesh, &textures);
        cache.flush(gpu, mesh, &textures, skin, &[], &[], &mut Budget::new());
        assert_eq!(cache.stats().uploads, 1);
        drain(&mut cache, gpu, mesh, &textures, skin, &[], &[]);
        assert_eq!(cache.mesh_views[0], cache.mesh_views[1]);

        let mut later = material(2, false);
        later.prepared = None;
        textures.push(later);
        cache.sync_meshes(gpu, mesh, &textures);
        let placeholder = cache.mesh_views[2].clone();
        drain(&mut cache, gpu, mesh, &textures, skin, &[], &[]);
        assert_ne!(cache.mesh_views[2], placeholder);
        assert_eq!(cache.stats().uploads, 2);
    });
}

#[test]
#[ignore = "requires a compatible graphics adapter"]
fn pending_mesh_publication_reuses_its_staged_view() {
    with_gpu(|gpu, mesh, skin| {
        let ready = material(0, false);
        let mut pending = ready.clone();
        pending.pending = true;
        pending.prepared = None;
        pending.texture = crate::scene::actors::texture_loading::placeholder().into();
        let mut textures = [pending];
        let mut cache = WorldTextures::default();
        cache.sync_meshes(gpu, mesh, &textures);
        cache.stage(gpu, &ready, false, ready.prepared_material(false).unwrap());
        let staged = cache.staged[&key(&ready, false)].clone();
        textures[0] = ready;
        cache.refresh_meshes(gpu, mesh, &textures, &[0, 0], 1);
        drain(&mut cache, gpu, mesh, &textures, skin, &[], &[]);
        assert_eq!(cache.mesh_views[0], staged);
        assert_eq!(cache.stats().uploads, 1);
    });
}

#[test]
#[ignore = "requires a compatible graphics adapter"]
fn pending_skin_publication_reuses_staged_views_and_updates_its_glow_pair() {
    with_gpu(|gpu, mesh, skin| {
        let ready = material(0, true);
        let mut pending = ready.clone();
        pending.pending = true;
        pending.prepared = None;
        pending.texture = crate::scene::actors::texture_loading::placeholder().into();
        let mut textures = [pending, material(1, true)];
        let mut cache = WorldTextures::default();
        let pairs = [(0, 1)];
        cache.sync_skins(gpu, skin, &textures, &pairs);
        drain(&mut cache, gpu, mesh, &[], skin, &textures, &pairs);
        let glow = cache.skins[2].bind_group.clone();
        let sheet = cache.skin_views[1].clone();
        cache.stage(gpu, &ready, true, ready.prepared_material(true).unwrap());
        let staged = cache.staged[&key(&ready, true)].clone();
        textures[0] = ready;
        cache.refresh_skins(gpu, skin, &textures, &pairs, &[0, 0], 2);
        drain(&mut cache, gpu, mesh, &[], skin, &textures, &pairs);
        assert_eq!(cache.skin_views[0], staged);
        assert_eq!(cache.skin_views[1], sheet);
        assert_ne!(cache.skins[2].bind_group, glow);
        assert_eq!(cache.stats().uploads, 2);
    });
}

#[test]
#[ignore = "requires a compatible graphics adapter"]
fn replacing_a_ready_slot_invalidates_staged_data_with_the_same_key() {
    with_gpu(|gpu, mesh, skin| {
        let mut textures = [material(0, false)];
        let mut cache = WorldTextures::default();
        cache.sync_meshes(gpu, mesh, &textures);
        cache.stage(
            gpu,
            &textures[0],
            false,
            textures[0].prepared_material(false).unwrap(),
        );
        let stale = cache.staged[&key(&textures[0], false)].clone();
        std::sync::Arc::make_mut(&mut textures[0].texture).data[0] = 255;
        textures[0].prepared = None;
        textures[0].prepare_material(false);
        cache.refresh_meshes(gpu, mesh, &textures, &[0], 1);
        drain(&mut cache, gpu, mesh, &textures, skin, &[], &[]);
        assert_ne!(cache.mesh_views[0], stale);
        assert_eq!(cache.stats().uploads, 2);
    });
}
