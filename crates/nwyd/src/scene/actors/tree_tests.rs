use super::test_support::{add_model, empty_scene};
use super::*;
use formats::texture::texture_list::FormatFlag;

#[test]
fn tree_draws_use_foliage_cutoff_and_preserve_opaque_parts() {
    let mut scene = empty_scene();

    let flags = [
        FormatFlag::Alpha,
        FormatFlag::AlphaLower,
        FormatFlag::None_,
        FormatFlag::ColorKey,
    ];

    let model = add_model(&mut scene, 63, &flags);

    scene.tree_scratch.insert(
        model,
        FrameTree::build(&scene.models[model].animation.skeleton),
    );

    scene.grove.trees.push(trees::Tree {
        record: 0,
        kind: 0x14B,
        model_id: 63,
        model: Some(model),
        position: Vec3::ZERO,
        yaw: 0.0,
        phase_ms: 0,
        firefly_stamp_ms: 0,
        diffuse: [1.0; 4],
        emissive: [0.0; 4],
    });

    let mut objects = Vec::new();

    let mut palettes = Vec::new();

    let mut draws = Vec::new();

    let mut spawned = Vec::new();

    scene.tree_frame(
        0,
        &Matrix4::IDENTITY,
        Reach::at(Vec3::ZERO),
        &[],
        SkinFog::on(0.0, 100.0),
        &mut objects,
        &mut palettes,
        &mut draws,
        &mut spawned,
    );

    assert_eq!(objects.len(), 4);
    assert_eq!(palettes.len(), 4);
    assert_eq!(draws.len(), 4);

    for (index, force_opaque) in [0.0, 0.0, 1.0, 1.0].into_iter().enumerate() {
        assert_eq!(objects[index].flags[1], 170.0 / 255.0);
        assert_eq!(objects[index].flags[3], force_opaque);
        assert_eq!(objects[index].stage[0], 0.0);
        assert_eq!(draws[index].part, index);
        assert_eq!(draws[index].object, index as u32);
        assert_eq!(draws[index].palette, index as u32);
        assert_eq!(draws[index].texture, Some(index));
        assert_eq!(draws[index].pass, SkinPass::Opaque);
        assert!(draws[index].pass.depth_write());
    }
}
