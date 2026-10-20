use super::super::*;
use crate::scene::actors::{ActorMeshPlacement, HeldDraw};
use formats::texture::{MipLevel, PixelFormat};
use render::texture::{MipPolicy, with_mipmaps};

fn source(alpha: [u8; 4]) -> Arc<Texture> {
    Texture {
        width: 2,
        height: 2,
        format: PixelFormat::Rgba8Unorm,
        source_len: 16,
        mips: vec![MipLevel {
            width: 2,
            height: 2,
            offset: 0,
            len: 16,
        }],
        data: alpha.into_iter().flat_map(|a| [233, 171, 67, a]).collect(),
    }
    .into()
}

fn scene(flag: FormatFlag, alpha: [u8; 4]) -> ObjectScene {
    let mut scene = alpha_tests::synthetic_scene(&[flag], 1);
    scene.textures[0].pending = false;
    scene.textures[0].texture = source(alpha);
    scene.textures.push(BoundTexture {
        pending: false,
        effect_table: true,
        slot: 0,
        path: "synthetic-shine".into(),
        flag: FormatFlag::None_,
        texture: source([255; 4]),
        mip_policy_override: None,
        prepared: None,
    });
    scene
}

fn held(glow: Option<usize>, silhouette: bool) -> HeldDraw {
    HeldDraw {
        motion_key: (0, 0),
        mesh_id: 100,
        world: Matrix4::IDENTITY,
        placement: ActorMeshPlacement::Hand,
        silhouette,
        glow,
        ordinary_glow: None,
    }
}

fn last_mip(bound: &BoundTexture) -> Vec<u8> {
    let texture = with_mipmaps(&bound.texture, bound.mip_policy(false));
    let mip = texture.mips.last().unwrap();
    texture.data[mip.offset..mip.offset + mip.len].to_vec()
}

#[test]
fn opaque_weapon_shine_keeps_colored_zero_alpha_texels_in_generated_mips() {
    for flag in [
        FormatFlag::ColorKey,
        FormatFlag::Alpha,
        FormatFlag::AlphaLower,
    ] {
        let mut scene = scene(flag, [0; 4]);
        scene.prepare_held_materials(100);
        let frame = alpha_tests::frame(&scene, &[held(Some(0), false)], &[], &[]);
        let draw = &frame.draws[0];
        let material = &scene.textures[draw.texture];
        assert_eq!(last_mip(material), [233, 171, 67, 0], "{flag:?}");
        assert_eq!(material.mip_policy(false), MipPolicy::Blend);
        assert_eq!(draw.pass, MeshPass::Opaque);
        assert_eq!(draw.cull, Cull::None);
        assert_eq!(draw.stage1, Some(1));
        assert_eq!(frame.objects[draw.object as usize].alpha[0], 0.0);
        assert_eq!(frame.objects[draw.object as usize].alpha[1], 0.0);
        assert_eq!(scene.textures[0].mip_policy(false), MipPolicy::Cutout(170));
        assert_eq!(last_mip(&scene.textures[0]), [0; 4]);
        assert!(Arc::ptr_eq(&material.texture, &scene.textures[0].texture));
    }
}

#[test]
fn weapon_mask_mips_average_shine_strength_without_coverage_adjustment() {
    let mut scene = scene(FormatFlag::ColorKey, [0, 170, 170, 170]);
    scene.prepare_held_materials(100);
    let frame = alpha_tests::frame(&scene, &[held(Some(0), false)], &[], &[]);
    assert_eq!(
        last_mip(&scene.textures[frame.draws[0].texture]),
        [233, 171, 67, 128]
    );
    assert_eq!(
        scene.textures[0].texture.data,
        source([0, 170, 170, 170]).data
    );
}

#[test]
fn shared_weapon_and_scenery_keep_distinct_material_policies_in_one_frame() {
    let mut scene = scene(FormatFlag::ColorKey, [0; 4]);
    alpha_tests::place(&mut scene);
    let mut ordinary_shine = scene.textures[1].clone();
    ordinary_shine.slot = 154;
    scene.textures.push(ordinary_shine);
    scene.prepare_held_materials(100);
    let ordinary = HeldDraw {
        ordinary_glow: Some(crate::scene::weapon_glow::Ordinary {
            refine: 1,
            class: 5,
            mmulti: false,
            costume: false,
        }),
        ..held(None, false)
    };
    let frame = alpha_tests::frame(
        &scene,
        &[ordinary, held(Some(0), false), held(Some(0), true)],
        &[],
        &[],
    );
    assert_eq!(frame.draws.len(), 4);
    for index in [0, 1, 3] {
        let draw = &frame.draws[index];
        assert_eq!(draw.texture, 0);
        assert_eq!(
            scene.textures[draw.texture].mip_policy(false),
            MipPolicy::Cutout(170)
        );
    }
    assert_eq!(frame.draws[0].pass, MeshPass::Cutout);
    assert_eq!(frame.draws[1].pass, MeshPass::Cutout);
    assert_eq!(frame.draws[1].stage1, Some(2));
    assert_eq!(
        frame.objects[frame.draws[1].object as usize].alpha[0],
        170.0 / 255.0
    );
    assert_eq!(frame.draws[3].pass, MeshPass::Silhouette);
    assert_ne!(frame.draws[2].texture, 0);
    assert_eq!(
        last_mip(&scene.textures[frame.draws[2].texture]),
        [233, 171, 67, 0]
    );
}

#[test]
fn missing_shine_resource_keeps_the_original_cutout_draw() {
    let mut scene = scene(FormatFlag::ColorKey, [0; 4]);
    scene.prepare_held_materials(100);
    let frame = alpha_tests::frame(&scene, &[held(Some(999), false)], &[], &[]);
    assert_eq!(frame.draws[0].texture, 0);
    assert_eq!(frame.draws[0].pass, MeshPass::Cutout);
    assert_eq!(
        frame.objects[frame.draws[0].object as usize].alpha[0],
        170.0 / 255.0
    );
    assert_eq!(frame.draws[0].stage1, None);
}

#[test]
fn repeated_weapon_preparation_reuses_mask_variants_and_preserves_opaque_materials() {
    let mut scene = scene(FormatFlag::Alpha, [0; 4]);
    scene.prepare_held_materials(100);
    let count = scene.textures.len();
    let variant = scene.held_mask_texture(0);
    assert_ne!(variant, 0);
    scene.by_id_meshes.insert(101, 0);
    scene.prepare_held_materials(100);
    scene.prepare_held_materials(101);
    assert_eq!(scene.textures.len(), count);
    assert_eq!(scene.held_mask_texture(0), variant);
    let mut scene = self::scene(FormatFlag::None_, [0; 4]);
    scene.prepare_held_materials(100);
    let frame = alpha_tests::frame(&scene, &[held(Some(0), false)], &[], &[]);
    assert_eq!(scene.textures.len(), 2);
    assert_eq!(frame.draws[0].texture, 0);
    assert_eq!(scene.textures[0].mip_policy(false), MipPolicy::Opaque);
}

#[test]
fn installing_weapon_worker_resources_keeps_mask_variants_separate_from_shared_cutouts() {
    let mut target = scene(FormatFlag::ColorKey, [0; 4]);
    target.textures[0].pending = true;
    target.textures[0].texture = source([255; 4]);
    let mut unrelated = target.textures[1].clone();
    unrelated.slot = 77;
    unrelated.path = "unrelated-effect".into();
    target.textures.push(unrelated);
    target.prepare_held_materials(100);
    let variant = target.held_mask_texture(0);
    assert!(target.textures[variant].pending);
    let mut worker = scene(FormatFlag::ColorKey, [0; 4]);
    worker.prepare_held_materials(100);
    assert_ne!(worker.held_mask_texture(0), variant);
    let bundle = resources::Bundle::from_scene(worker);
    target.install_resources(bundle);
    assert_eq!(target.held_mask_texture(0), variant);
    assert!(!target.textures[0].pending);
    assert!(!target.textures[variant].pending);
    assert_eq!(target.textures[0].mip_policy(false), MipPolicy::Cutout(170));
    assert_eq!(target.textures[variant].mip_policy(false), MipPolicy::Blend);
    assert_eq!(last_mip(&target.textures[0]), [0; 4]);
    assert_eq!(last_mip(&target.textures[variant]), [233, 171, 67, 0]);
    let frame = alpha_tests::frame(&target, &[held(Some(0), false)], &[], &[]);
    assert_eq!(frame.draws[0].texture, variant);
    assert!(target.texture_updates.contains(&0));
    assert!(target.texture_updates.contains(&variant));
}
