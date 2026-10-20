use super::*;
use render::texture::{MipPolicy, with_mipmaps};

fn game() -> GameDir {
    let mut texture = vec![0; 129];
    texture[..4].copy_from_slice(b"WS10");
    for (offset, value) in [(5, 124_u32), (13, 4), (17, 4), (81, 4)] {
        texture[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }
    texture[85..89].copy_from_slice(b"7x0s");
    texture.extend_from_slice(&0_u64.to_le_bytes());
    texture.extend_from_slice(&0xffff_u16.to_le_bytes());
    texture.extend_from_slice(&0xffff_u16.to_le_bytes());
    texture.extend_from_slice(&0_u32.to_le_bytes());
    GameDir::from_files(
        "synthetic-held-resources",
        [
            ("mesh/sample.msa".into(), alpha_tests::triangle_mesh(1)),
            ("mesh/tex0.wys".into(), texture),
        ],
    )
    .unwrap()
}

fn setup() -> (GameDir, ObjectScene) {
    let scene = alpha_tests::synthetic_scene(&[FormatFlag::ColorKey], 1);
    (game(), ObjectScene::empty(scene.registries.clone()))
}

#[test]
fn background_weapon_request_prepares_mask_mips_before_publishing_the_mesh() {
    let (game, mut target) = setup();
    let key = Key::Mesh(100, true);
    let bundle = key.prepare(&game, target.registries.clone()).unwrap();
    target.install_resources(bundle);
    assert!(key.resident(&target));
    let base = target.meshes[0].subset_textures[0].unwrap();
    let mask = target.held_mask_texture(base);
    assert_ne!(mask, base);
    let base_material = target.textures[base].prepared_material(false).unwrap();
    let mask_material = target.textures[mask].prepared_material(false).unwrap();
    assert_eq!(base_material.policy(), MipPolicy::Cutout(170));
    assert_eq!(mask_material.policy(), MipPolicy::Blend);
    let mip = mask_material.texture().mips.last().unwrap();
    assert_eq!(
        &mask_material.texture().data[mip.offset..mip.offset + mip.len],
        [255, 255, 255, 0]
    );
    assert_eq!(target.textures[base].texture, target.textures[mask].texture);
}

#[test]
fn a_resident_cutout_mesh_still_requests_missing_weapon_mask_materials() {
    let (game, mut target) = setup();
    let ordinary = Key::Mesh(100, false);
    let special = Key::Mesh(100, true);
    target.install_resources(ordinary.prepare(&game, target.registries.clone()).unwrap());
    assert!(ordinary.resident(&target));
    assert!(!special.resident(&target));
    target.install_resources(special.prepare(&game, target.registries.clone()).unwrap());
    assert!(ordinary.resident(&target));
    assert!(special.resident(&target));
    assert_eq!(target.meshes.len(), 1);
    let base = target.meshes[0].subset_textures[0].unwrap();
    let mask = target.held_mask_texture(base);
    assert_ne!(mask, base);
    let texture = &target.textures[mask];
    let mip = with_mipmaps(&texture.texture, texture.mip_policy(false));
    assert_eq!(&mip.data[mip.data.len() - 4..], [255, 255, 255, 0]);
}

#[test]
fn ordinary_background_meshes_keep_only_the_original_cutout_material() {
    let (game, mut target) = setup();
    let key = Key::Mesh(100, false);
    target.install_resources(key.prepare(&game, target.registries.clone()).unwrap());
    assert!(key.resident(&target));
    assert_eq!(target.textures.len(), 1);
    let base = target.meshes[0].subset_textures[0].unwrap();
    assert_eq!(target.held_mask_texture(base), base);
    let material = target.textures[base].prepared_material(false).unwrap();
    assert_eq!(material.policy(), MipPolicy::Cutout(170));
    let mip = material.texture().mips.last().unwrap();
    assert_eq!(
        &material.texture().data[mip.offset..mip.offset + mip.len],
        [0, 0, 0, 0]
    );
}

#[test]
fn equipping_an_evicted_weapon_mask_reloads_its_colors_without_changing_shared_cutouts() {
    let (game, mut target) = setup();
    let key = Key::Mesh(100, true);
    target.install_resources(key.prepare(&game, target.registries.clone()).unwrap());
    let base = target.meshes[0].subset_textures[0].unwrap();
    let mask = target.held_mask_texture(base);
    let original = target.textures[base].texture.clone();
    let mut loader = crate::scene::actors::texture_loading::Loader::new(&game, false);
    let now = Instant::now();
    let mut updates = Vec::new();
    loader.maintain(
        &mut target.textures,
        &mut updates,
        [base, mask].into_iter(),
        now,
    );
    loader.maintain(
        &mut target.textures,
        &mut updates,
        [base].into_iter(),
        now + Duration::from_secs(121),
    );
    assert_eq!(updates, [mask]);
    assert!(!target.textures[base].pending);
    assert!(target.textures[mask].pending);
    assert!(target.textures[mask].prepared.is_none());
    assert_eq!(target.textures[mask].texture.data, [0, 0, 0, 0]);
    loader.poll(&mut target.textures, &mut updates);
    assert!(target.textures[mask].pending);
    updates.clear();
    loader.maintain(
        &mut target.textures,
        &mut updates,
        [base, mask].into_iter(),
        now + Duration::from_secs(122),
    );
    let deadline = Instant::now() + Duration::from_secs(5);
    while target.textures[mask].pending {
        loader.poll(&mut target.textures, &mut updates);
        assert!(Instant::now() < deadline, "weapon mask reload timed out");
        if target.textures[mask].pending {
            std::thread::sleep(Duration::from_millis(1));
        }
    }
    assert_eq!(updates, [mask]);
    assert_eq!(target.held_mask_texture(base), mask);
    assert!(Arc::ptr_eq(&target.textures[base].texture, &original));
    assert_eq!(target.textures[mask].texture, original);
    let mask_material = target.textures[mask].prepared_material(false).unwrap();
    let base_material = target.textures[base].prepared_material(false).unwrap();
    assert_eq!(mask_material.policy(), MipPolicy::Blend);
    assert_eq!(base_material.policy(), MipPolicy::Cutout(170));
    let mask_mip = mask_material.texture().mips.last().unwrap();
    let base_mip = base_material.texture().mips.last().unwrap();
    assert_eq!(
        &mask_material.texture().data[mask_mip.offset..mask_mip.offset + mask_mip.len],
        [255, 255, 255, 0]
    );
    assert_eq!(
        &base_material.texture().data[base_mip.offset..base_mip.offset + base_mip.len],
        [0, 0, 0, 0]
    );
}
