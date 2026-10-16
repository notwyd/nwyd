use super::test_support::{add_model, empty_scene};
use super::*;
use formats::texture::texture_list::FormatFlag;

const FLAGS: [FormatFlag; 4] = [
    FormatFlag::Alpha,
    FormatFlag::AlphaLower,
    FormatFlag::None_,
    FormatFlag::ColorKey,
];

#[test]
fn fish_draws_use_model_cutoff_and_preserve_opaque_textures() {
    let mut scene = empty_scene();

    let model = add_model(&mut scene, 70, &FLAGS);

    scene.fish_scratch.insert(
        model,
        FrameTree::build(&scene.models[model].animation.skeleton),
    );

    let mut fish = fish::Fish::school(0, 0x158, [0.0; 3], 0)[0];

    fish.model = Some(model);

    scene.school.fish.push(fish);

    let mut objects = Vec::new();

    let mut palettes = Vec::new();

    let mut draws = Vec::new();

    scene.fish_frame(
        0,
        &Matrix4::IDENTITY,
        Reach::at(Vec3::ZERO),
        &[],
        SkinFog::on(0.0, 100.0),
        &mut objects,
        &mut palettes,
        &mut draws,
    );

    assert_eq!(objects.len(), 4);
    assert_eq!(palettes.len(), 4);
    assert_eq!(draws.len(), 4);

    for (index, force_opaque) in [0.0, 0.0, 1.0, 1.0].into_iter().enumerate() {
        assert_eq!(objects[index].flags[1], 170.0 / 255.0);
        assert_eq!(objects[index].flags[3], force_opaque);
        assert_eq!(objects[index].stage[0], 0.0);
        assert_eq!(draws[index].part, index);
        assert_eq!(draws[index].texture, Some(index));
        assert_eq!(draws[index].pass, SkinPass::Blended);
        assert!(!draws[index].pass.depth_write());
    }
}

#[test]
fn butterfly_cutoff_follows_appearance_without_clipping_special_effects() {
    let mut scene = empty_scene();

    let butterfly_model = add_model(&mut scene, 69, &FLAGS);

    let bird_model = add_model(&mut scene, 24, &FLAGS);

    for model in [butterfly_model, bird_model] {
        scene.butterfly_scratch.insert(
            model,
            FrameTree::build(&scene.models[model].animation.skeleton),
        );
    }

    let cases = [
        (0x157, 0, 170.0 / 255.0),
        (0x157, 1, 170.0 / 255.0),
        (0x157, 2, 170.0 / 255.0),
        (4, 0, 0.0),
        (4, 1, 0.0),
        (4, 2, 0.0),
        (6, 0, 0.0),
        (6, 1, 0.0),
        (6, 2, 0.0),
        (7, 3, 0.0),
    ];

    for (kind, motion, _) in cases {
        let mut butterfly = butterflies::Butterfly::flight(0, kind, [0.0; 3])[0];

        butterfly.arm = motion;

        butterfly.model = Some(if kind == 7 {
            bird_model
        } else {
            butterfly_model
        });

        scene.flight.butterflies.push(butterfly);
    }

    let mut objects = Vec::new();

    let mut palettes = Vec::new();

    let mut draws = Vec::new();

    scene.butterfly_frame(
        0,
        &Matrix4::IDENTITY,
        Reach::at(Vec3::ZERO),
        SkinFog::on(0.0, 100.0),
        &mut objects,
        &mut palettes,
        &mut draws,
    );

    assert_eq!(objects.len(), cases.len() * 4);
    assert_eq!(palettes.len(), objects.len());
    assert_eq!(draws.len(), objects.len());

    for (instance, (kind, motion, expected_cutoff)) in cases.into_iter().enumerate() {
        for (part, force_opaque) in [0.0, 0.0, 1.0, 1.0].into_iter().enumerate() {
            let index = instance * 4 + part;

            assert_eq!(
                objects[index].flags[1], expected_cutoff,
                "appearance {kind:#x}, movement {motion}, part {part}"
            );
            assert_eq!(objects[index].flags[3], force_opaque);
            assert_eq!(objects[index].stage[0], 0.0);
            assert_eq!(draws[index].pass, SkinPass::Blended);
            assert!(!draws[index].pass.depth_write());
        }
    }
}
