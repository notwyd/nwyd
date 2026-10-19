use super::sky_tests::{block_on, readback};
use super::*;

#[test]
fn prepared_passes_preserve_order_and_reuse_storage_when_draws_change() {
    let mut indices = std::array::from_fn(|_| Vec::new());
    let draws: Vec<_> = [
        MeshPass::Blended,
        MeshPass::Opaque,
        MeshPass::Blended,
        MeshPass::Sky,
        MeshPass::Opaque,
    ]
    .into_iter()
    .map(|pass| MeshDraw {
        pass,
        cull: Cull::Ccw,
        object: 0,
        texture: 0,
        stage1: None,
        base_vertex: 0,
        subset: SubsetRange {
            start_index: 0,
            index_count: 3,
        },
    })
    .collect();
    let wanted = prepare_pass_draws(&mut indices, &draws);
    assert_eq!(indices[MeshPass::Opaque.index()], [1, 4]);
    assert_eq!(indices[MeshPass::Blended.index()], [0, 2]);
    assert_eq!(indices[MeshPass::Sky.index()], [3]);
    assert_eq!(indices.iter().map(Vec::len).sum::<usize>(), draws.len());
    assert_eq!(
        wanted,
        pipeline_bit(MeshPass::Opaque, Cull::Ccw)
            | pipeline_bit(MeshPass::Blended, Cull::Ccw)
            | pipeline_bit(MeshPass::Sky, Cull::Ccw)
    );
    let capacities = indices.each_ref().map(Vec::capacity);
    prepare_pass_draws(&mut indices, &draws[3..]);
    assert_eq!(indices[MeshPass::Opaque.index()], [1]);
    assert_eq!(indices[MeshPass::Sky.index()], [0]);
    assert!(indices[MeshPass::Blended.index()].is_empty());
    assert_eq!(indices.each_ref().map(Vec::capacity), capacities);
    assert_eq!(prepare_pass_draws(&mut indices, &[]), 0);
    assert!(indices.iter().all(Vec::is_empty));
    assert_eq!(indices.each_ref().map(Vec::capacity), capacities);
}

fn draw_reference(
    renderer: &MeshRenderer,
    pass: &mut wgpu::RenderPass<'_>,
    textures: &[MeshTexture],
    draws: &[MeshDraw],
) {
    pass.set_vertex_buffer(0, renderer.geometry.vertices.buffer.slice(..));
    pass.set_index_buffer(
        renderer.geometry.indices.buffer.slice(..),
        wgpu::IndexFormat::Uint16,
    );
    pass.set_bind_group(0, &renderer.frame_bind_group, &[]);

    for draw in draws {
        let Some(texture) = (if draw.texture == NULL_TEXTURE {
            Some(&renderer.null_texture)
        } else {
            textures.get(draw.texture)
        }) else {
            continue;
        };
        let Some((_, pipeline)) = renderer
            .pipelines
            .iter()
            .find(|(key, _)| *key == (draw.pass, draw.cull))
        else {
            continue;
        };
        let Some(stage1) = draw
            .stage1
            .map_or(Some(texture), |index| textures.get(index))
        else {
            continue;
        };
        pass.set_pipeline(pipeline);
        pass.set_bind_group(
            1,
            &renderer.object_bind_group,
            &[draw.object * renderer.object_stride],
        );
        pass.set_bind_group(2, &texture.bind_group, &[]);
        pass.set_bind_group(3, &stage1.bind_group, &[]);
        pass.draw_indexed(draw.subset.range(), draw.base_vertex, 0..1);
    }
}

fn textures(gpu: &Gpu, renderer: &MeshRenderer) -> Vec<MeshTexture> {
    [[255, 32, 16, 128], [24, 255, 48, 192], [48, 16, 255, 96]]
        .map(|pixel| {
            let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                label: Some("mesh draw regression texture"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            });
            gpu.queue.write_texture(
                texture.as_image_copy(),
                &pixel,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(4),
                    rows_per_image: Some(1),
                },
                texture.size(),
            );
            renderer.bind_texture(gpu, &texture.create_view(&Default::default()))
        })
        .into()
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn mesh_passes_ranges_and_bindings_match_ordered_reference_rendering() {
    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let color = crate::Offscreen::new(&gpu, 64, 1);
    let motion = crate::Offscreen::with_format(&gpu, 64, 1, crate::temporal::MOTION_FORMAT);
    let bias = crate::Offscreen::with_format(&gpu, 64, 1, crate::temporal::BIAS_FORMAT);
    let readback_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mesh draw regression readback"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let arena = MeshArena {
        vertices: [(-1.0, -1.0), (-1.0, 1.0), (1.0, -1.0), (1.0, 1.0)]
            .map(|(x, y)| MeshVertex {
                position: [x, y, 0.5],
                normal: [0.0; 3],
                uv: [0.5; 2],
                diffuse: u32::MAX,
            })
            .into(),
        indices: vec![0, 1, 2, 2, 1, 3],
        allocs: Vec::new(),
    };
    let mut frame = FrameUniforms::zeroed();
    frame.view = Matrix4::IDENTITY.shader_constants();
    frame.projection = Matrix4::IDENTITY.shader_constants();
    let objects: Vec<_> = (0..4)
        .map(|index| {
            let mut object = ObjectUniforms::new(
                Matrix4::translation(index as f32 * 0.4, 0.0, 0.0),
                &DrawParams::glow([0.5, 0.75, 1.0, 0.5]),
                0.0,
            );
            object.set_stage1(Stage1Blend::Modulate, 0.0);
            if index == 3 {
                object.set_null_texture();
            }
            object
        })
        .collect();
    let base = MeshDraw {
        pass: MeshPass::Blended,
        cull: Cull::None,
        object: 0,
        texture: 0,
        stage1: None,
        base_vertex: 0,
        subset: SubsetRange {
            start_index: 0,
            index_count: 6,
        },
    };
    let mut draws = Vec::new();
    for (index, kind) in MeshPass::ALL.into_iter().rev().enumerate() {
        let cull = if matches!(kind, MeshPass::Silhouette) {
            Cull::Cw
        } else if matches!(
            kind,
            MeshPass::Blended | MeshPass::Additive | MeshPass::Reflective
        ) {
            Cull::None
        } else {
            Cull::Ccw
        };
        draws.extend([
            MeshDraw {
                pass: kind,
                cull,
                object: (index % 3) as u32,
                texture: index % 3,
                ..base
            },
            MeshDraw {
                pass: kind,
                cull,
                object: (index % 3) as u32,
                texture: index % 3,
                stage1: Some((index + 1) % 3),
                ..base
            },
        ]);
    }
    draws.extend([
        base,
        MeshDraw {
            texture: 1,
            stage1: Some(2),
            ..base
        },
        MeshDraw {
            texture: 1,
            stage1: Some(NULL_TEXTURE),
            ..base
        },
        MeshDraw { texture: 1, ..base },
        MeshDraw {
            texture: 15,
            ..base
        },
        MeshDraw {
            pass: MeshPass::Additive,
            cull: Cull::Cw,
            ..base
        },
        MeshDraw {
            object: 3,
            texture: NULL_TEXTURE,
            ..base
        },
        MeshDraw {
            object: 1,
            texture: 1,
            stage1: Some(2),
            ..base
        },
        MeshDraw {
            object: 2,
            texture: 1,
            stage1: Some(2),
            ..base
        },
        MeshDraw {
            object: 2,
            texture: 1,
            stage1: None,
            ..base
        },
        MeshDraw { texture: 2, ..base },
    ]);

    for temporal in [false, true] {
        let mut camera = crate::temporal::TemporalCamera::new(&gpu);
        camera.prepare(&gpu, &frame, [64, 1], [0.0; 2], 0.0);
        let mut renderer = MeshRenderer::with_depth(
            &gpu,
            crate::COLOR_FORMAT,
            None,
            VendorClass::DEFAULT,
            temporal.then_some(&camera),
        );
        let mut interleaved = MeshRenderer::with_depth(
            &gpu,
            crate::COLOR_FORMAT,
            None,
            VendorClass::DEFAULT,
            temporal.then_some(&camera),
        );
        renderer.upload_arena(&gpu, &arena);
        interleaved.upload_arena(&gpu, &arena);
        let textures = textures(&gpu, &renderer);
        interleaved.prepare(&gpu, &frame, &objects, &[MeshDraw { texture: 2, ..base }]);
        let reversed: Vec<_> = draws.iter().copied().rev().collect();
        for (frame_index, draws) in [draws.as_slice(), &reversed[..8], &[]]
            .into_iter()
            .enumerate()
        {
            renderer.prepare(&gpu, &frame, &objects, draws);
            for mode in 0..3 {
                let mut results = Vec::new();
                for reference in [true, false] {
                    let mut encoder = gpu.device.create_command_encoder(&Default::default());
                    {
                        let views = if temporal {
                            vec![&color.view, &motion.view, &bias.view]
                        } else {
                            vec![&color.view]
                        };
                        let attachments: Vec<_> = views
                            .into_iter()
                            .map(|view| {
                                Some(wgpu::RenderPassColorAttachment {
                                    view,
                                    depth_slice: None,
                                    resolve_target: None,
                                    ops: wgpu::Operations {
                                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                                        store: wgpu::StoreOp::Store,
                                    },
                                })
                            })
                            .collect();
                        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                            label: Some("mesh draw regression"),
                            color_attachments: &attachments,
                            depth_stencil_attachment: None,
                            timestamp_writes: None,
                            occlusion_query_set: None,
                            multiview_mask: None,
                        });
                        if temporal {
                            pass.set_bind_group(4, &camera.binding, &[]);
                        }
                        if mode < 2 {
                            let kinds = if mode == 0 {
                                MeshPass::ALL.as_slice()
                            } else {
                                &[
                                    MeshPass::Additive,
                                    MeshPass::Opaque,
                                    MeshPass::Blended,
                                    MeshPass::Blended,
                                ]
                            };
                            if reference {
                                let ordered: Vec<_> = kinds
                                    .iter()
                                    .flat_map(|kind| {
                                        draws.iter().copied().filter(move |draw| draw.pass == *kind)
                                    })
                                    .collect();
                                draw_reference(&renderer, &mut pass, &textures, &ordered);
                            } else {
                                renderer.draw_passes(&mut pass, &textures, kinds);
                            }
                        } else {
                            let midpoint = draws.len() / 2;
                            for (part, range) in [
                                0..midpoint,
                                midpoint..draws.len(),
                                draws.len()..draws.len() + 1,
                            ]
                            .into_iter()
                            .enumerate()
                            {
                                if part == 1 {
                                    interleaved.draw(&mut pass, &textures);
                                }
                                if reference {
                                    draw_reference(
                                        &renderer,
                                        &mut pass,
                                        &textures,
                                        draws.get(range).unwrap_or(&[]),
                                    );
                                } else {
                                    renderer.draw_range(&mut pass, &textures, range);
                                }
                            }
                        }
                    }
                    encoder.copy_texture_to_buffer(
                        color.texture.as_image_copy(),
                        wgpu::TexelCopyBufferInfo {
                            buffer: &readback_buffer,
                            layout: wgpu::TexelCopyBufferLayout {
                                offset: 0,
                                bytes_per_row: Some(256),
                                rows_per_image: Some(1),
                            },
                        },
                        color.texture.size(),
                    );
                    results.push(readback(
                        &gpu,
                        &readback_buffer,
                        gpu.queue.submit([encoder.finish()]),
                    ));
                }
                assert_eq!(
                    results[0], results[1],
                    "temporal={temporal}, frame={frame_index}, mode={mode}"
                );
                if frame_index == 0 {
                    assert!(results[0].iter().any(|byte| *byte != 0));
                }
            }
        }
    }
    assert!(block_on(scope.pop()).is_none());
}
