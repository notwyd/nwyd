use super::*;
use crate::scene::actors::test_support::{add_model, empty_scene};
use formats::texture::texture_list::FormatFlag;
use render::texture::MipPolicy;

fn spec(model_id: u32) -> ModelSpec {
    ModelSpec {
        model_id,
        parts: [PartRef::EMPTY; appearance::PARTS],
        clip: ClipChoice::Fixed {
            slot: trees::TREE_SLOT,
            sub_frame_ms: trees::TREE_SUB_FRAME_MS,
        },
        variant: 0,
        costume_body: 0,
    }
}

fn prepared(share_texture: bool) -> Bundle {
    let mut scene = empty_scene();
    let tree = add_model(&mut scene, 63, &[FormatFlag::Alpha]);
    let mut by_spec = BTreeMap::from([(spec(63), Some(tree))]);

    if share_texture {
        let actor = add_model(&mut scene, 1, &[FormatFlag::Alpha]);

        scene.models[actor].parts[0].texture = scene.models[tree].parts[0].texture;

        by_spec.insert(spec(1), Some(actor));
    }

    let mut bundle = Bundle {
        models: scene.models,
        textures: scene.textures,
        by_spec,
        ..Default::default()
    };

    bundle.prepare_tree_textures(&[spec(63)]);

    bundle
}

#[test]
fn streamed_foliage_prepares_the_actual_texture_policy_before_gpu_staging() {
    let bundle = prepared(false);

    assert_eq!(bundle.textures.len(), 1);
    assert_eq!(bundle.models[0].parts[0].texture, Some(0));
    assert_eq!(bundle.textures[0].mip_policy(true), MipPolicy::Cutout(170));

    let mut scene = empty_scene();

    scene.install_resources(bundle);

    let tree = scene.by_spec[&spec(63)].unwrap();
    let texture = scene.models[tree].parts[0].texture.unwrap();
    let count = scene.textures.len();

    scene.prepare_tree_textures(Some(tree));

    assert_eq!(scene.textures.len(), count);
    assert_eq!(scene.models[tree].parts[0].texture, Some(texture));
    assert_eq!(
        scene.textures[texture].mip_policy(true),
        MipPolicy::Cutout(170)
    );
}

#[test]
fn streamed_foliage_keeps_shared_blended_textures_and_reuses_resident_variants() {
    let bundle = prepared(true);

    assert_eq!(bundle.textures.len(), 2);

    for (model, policy) in [(0, MipPolicy::Cutout(170)), (1, MipPolicy::Blend)] {
        let texture = bundle.models[model].parts[0].texture.unwrap();

        assert_eq!(bundle.textures[texture].mip_policy(true), policy);
    }

    let mut scene = empty_scene();
    let resident = add_model(&mut scene, 64, &[FormatFlag::Alpha]);

    scene.prepare_tree_textures(Some(resident));

    let cutout = scene.models[resident].parts[0].texture.unwrap();
    let count = scene.textures.len();

    scene.install_resources(bundle);

    let tree = scene.by_spec[&spec(63)].unwrap();
    let actor = scene.by_spec[&spec(1)].unwrap();

    assert_eq!(scene.textures.len(), count);
    assert_eq!(scene.models[tree].parts[0].texture, Some(cutout));
    assert_eq!(scene.models[actor].parts[0].texture, Some(0));
    assert_eq!(
        scene.textures[cutout].mip_policy(true),
        MipPolicy::Cutout(170)
    );
    assert_eq!(scene.textures[0].mip_policy(true), MipPolicy::Blend);
}

#[test]
fn later_generic_skin_loading_does_not_reuse_a_foliage_only_texture() {
    let mut scene = empty_scene();

    scene.install_resources(prepared(false));

    let mut bytes = vec![0; texture_list::LEGACY_RECORD_LEN];
    let path = b"synthetic-0";

    bytes[..path.len()].copy_from_slice(path);

    bytes[255] = FormatFlag::Alpha.to_byte();

    let textures = texture_list::parse_legacy(&bytes).unwrap();
    let game = GameDir::from_files("shared-foliage-textures", []).unwrap();

    scene.enable_background_textures(&game);

    let texture = scene
        .resource_loader()
        .bind_table_slot(&game, &textures, 0, false)
        .unwrap();

    assert_ne!(texture, 0);
    assert_eq!(scene.textures[0].mip_policy(true), MipPolicy::Cutout(170));
    assert_eq!(scene.textures[texture].mip_policy(true), MipPolicy::Blend);
    assert_eq!(scene.textures[texture].path, "synthetic-0");
    assert_eq!(
        scene
            .resource_loader()
            .bind_table_slot(&game, &textures, 0, false),
        Some(texture)
    );
}
