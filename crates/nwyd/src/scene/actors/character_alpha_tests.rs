use super::*;
use formats::texture::texture_list::FormatFlag;

fn emit(scene: &ActorScene, model: usize, draw: Draw<'_>, refined: bool) -> Frame {
    let model = &scene.models[model];
    let mut tree = FrameTree::build(&model.animation.skeleton);
    tree.combine(&Matrix4::IDENTITY.as_d3dx());

    let mut refinement = [None; appearance::PARTS];
    if refined {
        refinement[0] = Some(PartGlow {
            glow: glow::Glow {
                texture: 0,
                uv: glow::Uv::Mesh,
                blend: glow::Blend::AddSmooth,
                double: false,
                emissive: true,
            },
            sheet: 0,
            pair: 0,
        });
    }

    let mut frame = Frame::default();
    emit_with_glow(
        model,
        &tree,
        &scene.textures,
        &refinement,
        draw,
        (&mut frame.objects, &mut frame.palettes, &mut frame.draws),
    );
    frame
}

#[test]
fn character_cutouts_preserve_appearance_texture_and_invisibility_exceptions() {
    let mut scene = test_support::empty_scene();
    let flags = [
        FormatFlag::Alpha,
        FormatFlag::AlphaLower,
        FormatFlag::None_,
        FormatFlag::ColorKey,
    ];
    let models = [0, 7, 32, 43, 60].map(|id| test_support::add_model(&mut scene, id, &flags));

    for (class, body_texture, cutoff) in [
        (1, 0, 170.0 / 255.0),
        (0x20, 1, 170.0 / 255.0),
        (0x20, 2, 0.0),
        (0x20, 3, 170.0 / 255.0),
        (0x3B, 0, 0.0),
        (0x3B, 2, 0.0),
        (0x1F, 2, 170.0 / 255.0),
    ] {
        for model in models {
            for invisible in [false, true] {
                for field_scene in [false, true] {
                    for refined in [false, true] {
                        let frame = emit(
                            &scene,
                            model,
                            Draw {
                                view: &Matrix4::IDENTITY,
                                eye: Vec3::ZERO,
                                field_scene,
                                fog: SkinFog::on(0.0, 100.0),
                                class,
                                body_texture,
                                highlight: !invisible,
                                recent_hit: false,
                                invisible,
                            },
                            refined,
                        );

                        let copies = if invisible { 1 } else { 2 };
                        assert_eq!(frame.draws.len(), flags.len() * copies);
                        assert_eq!(frame.objects.len(), frame.draws.len());
                        assert_eq!(frame.palettes.len(), frame.draws.len());

                        for (part, flag) in flags.into_iter().enumerate() {
                            let expected = if invisible || flag == FormatFlag::ColorKey {
                                0.0
                            } else {
                                cutoff
                            };
                            for offset in 0..copies {
                                let draw = &frame.draws[part * copies + offset];
                                let object = &frame.objects[draw.object as usize];
                                assert_eq!(
                                    object.flags[1], expected,
                                    "class {class}, texture {body_texture}, flag {flag:?}, model {model}"
                                );
                                assert_eq!(
                                    object.flags[3],
                                    f32::from(u8::from(flag == FormatFlag::None_))
                                );
                                assert_eq!(object.stage[0], 0.0);
                                assert_eq!(
                                    draw.pass,
                                    if invisible {
                                        SkinPass::Invisible
                                    } else if offset == 0 {
                                        SkinPass::Opaque
                                    } else {
                                        SkinPass::Highlight
                                    }
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
