use super::*;
use formats::audio::ani_sound::AniSound;
use formats::data::item_list;
use formats::mesh::registry;
use formats::text::Codepage;
use formats::texture::{MipLevel, PixelFormat, texture_list};
use render::texture::MipPolicy;

fn empty_scene() -> ObjectScene {
    ObjectScene::empty(Arc::new(Registries {
        resource_names: formats::scene::rc::parse(&[]).unwrap(),
        entries: Vec::new(),
        valid: registry::parse_valid_index(&vec![0; registry::VALID_INDEX_LEN]).unwrap(),
        sounds: AniSound::default(),
        textures: texture_list::parse(&[]).unwrap(),
        effect_textures: texture_list::parse(&[]).unwrap(),
        items: item_list::ItemList::from_plain(vec![0; item_list::BODY_LEN], Codepage::Windows1252),
        list: Default::default(),
        arena_visual: Vec::new(),
    }))
}

fn texture(slot: usize, flag: FormatFlag, effect_table: bool) -> BoundTexture {
    BoundTexture {
        prepared: None,
        mip_policy_override: None,
        pending: false,
        effect_table,
        slot,
        path: format!("authored-{slot}.wys"),
        flag,
        texture: Texture {
            width: 4,
            height: 4,
            format: PixelFormat::Rgba8Unorm,
            mips: vec![
                MipLevel {
                    width: 4,
                    height: 4,
                    offset: 0,
                    len: 64,
                },
                MipLevel {
                    width: 2,
                    height: 2,
                    offset: 64,
                    len: 16,
                },
            ],
            data: [vec![32; 64], vec![192; 16]].concat(),
            source_len: 97,
        }
        .into(),
    }
}

#[test]
fn worker_bundle_keeps_authored_source_mips_and_static_effect_policies() {
    let mut scene = empty_scene();
    scene.textures = vec![
        texture(0, FormatFlag::Alpha, false),
        texture(0, FormatFlag::Alpha, true),
        texture(1, FormatFlag::None_, false),
    ];
    let originals: Vec<_> = scene
        .textures
        .iter()
        .map(|bound| bound.texture.clone())
        .collect();
    let bundle = Bundle::from_scene(scene);

    assert_eq!(bundle.textures.len(), 3);
    for (bound, original) in bundle.textures.iter().zip(originals) {
        assert_eq!(bound.texture, original);
        assert!(!bound.pending);
        let material = bound.prepared_material(false).unwrap();
        assert_eq!(material.policy(), bound.mip_policy(false));
        assert_eq!(
            material.texture(),
            render::texture::with_mipmaps(&original, bound.mip_policy(false)).as_ref()
        );
        assert_eq!(
            &material.texture().data[..original.data.len()],
            original.data
        );
        assert_eq!(material.texture().mips.len(), 3);
    }
    assert_eq!(
        bundle.textures[0].mip_policy(false),
        MipPolicy::Cutout(render::mesh::ALPHAREF_MESH_ALPHA_32BPP as u8)
    );
    assert_eq!(bundle.textures[1].mip_policy(false), MipPolicy::Blend);
    assert_eq!(bundle.textures[2].mip_policy(false), MipPolicy::Opaque);
}

#[test]
fn worker_bundle_replaces_pending_textures_without_overwriting_resident_tables() {
    let mut scene = empty_scene();
    let mut pending = texture(0, FormatFlag::Alpha, false);
    pending.pending = true;
    Arc::make_mut(&mut pending.texture).data.fill(0);
    let resident = texture(0, FormatFlag::Alpha, true);
    scene.textures = vec![pending, resident.clone()];
    let replacement = texture(0, FormatFlag::Alpha, false);
    let mut ignored = texture(0, FormatFlag::Alpha, true);
    Arc::make_mut(&mut ignored.texture).data.fill(255);
    let mut worker = empty_scene();
    worker.textures = vec![replacement.clone(), ignored];
    let bundle = Bundle::from_scene(worker);
    let prepared = bundle.textures[0].prepared.as_ref().unwrap().clone();

    scene.install_resources(bundle);

    assert_eq!(scene.textures.len(), 2);
    assert!(!scene.textures[0].pending);
    assert_eq!(scene.textures[0].texture, replacement.texture);
    assert_eq!(scene.textures[1].texture, resident.texture);
    assert_eq!(scene.texture_updates, [0]);
    assert!(Arc::ptr_eq(
        scene.textures[0].prepared.as_ref().unwrap(),
        &prepared
    ));
}

#[test]
fn worker_bundle_leaves_pending_texture_preparation_to_its_loader() {
    let mut scene = empty_scene();
    let mut pending = texture(0, FormatFlag::Alpha, false);
    pending.pending = true;
    scene.textures.push(pending.clone());

    let bundle = Bundle::from_scene(scene);

    assert!(bundle.textures[0].pending);
    assert!(bundle.textures[0].prepared.is_none());
    assert_eq!(bundle.textures[0].texture, pending.texture);
}
