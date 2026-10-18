use super::*;

#[test]
fn neural_material_masks_preserve_effects_and_explicit_foliage_weights() {
    let mut objects = vec![SkinObject::zeroed(); 4];
    objects[1].stage = [1.0, 0.25, 0.0, 0.0];
    objects[3].stage[1] = f32::NAN;
    let draws: Vec<_> = [
        SkinPass::Opaque,
        SkinPass::Opaque,
        SkinPass::EffectAlpha,
        SkinPass::Opaque,
    ]
    .into_iter()
    .enumerate()
    .map(|(object, pass)| SkinDraw {
        pass,
        object: object as u32,
        part: 0,
        palette: 0,
        texture: None,
    })
    .collect();
    let mut output = Vec::new();
    neural_objects(&objects, &draws, &mut output);
    assert_eq!(
        output
            .iter()
            .map(|object| object.stage[1])
            .collect::<Vec<_>>(),
        vec![0.0, 0.25, 1.0, 1.0]
    );
    assert_eq!(output[1].stage[0], 1.0);
    assert_eq!(objects[2].stage[1], 0.0);
    neural_objects(&objects[..1], &[], &mut output);
    assert_eq!(output.len(), 1);
    assert_eq!(output[0].stage[1], 0.0);
}

#[cfg(not(target_arch = "wasm32"))]
#[test]
#[ignore = "requires a compatible GPU adapter"]
fn temporal_skin_shader_writes_material_preservation_after_history_settles() {
    use std::future::Future;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};
    use std::time::Duration;

    struct ThreadWake(std::thread::Thread);

    impl Wake for ThreadWake {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }

    fn block_on<T>(future: impl Future<Output = T>) -> T {
        let waker = Waker::from(Arc::new(ThreadWake(std::thread::current())));
        let mut context = Context::from_waker(&waker);
        let mut future = std::pin::pin!(future);

        loop {
            match future.as_mut().poll(&mut context) {
                Poll::Ready(value) => return value,
                Poll::Pending => std::thread::park(),
            }
        }
    }

    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut camera = crate::temporal::TemporalCamera::new(&gpu);
    let mut world = crate::world::FrameUniforms::zeroed();
    world.view = Matrix4::IDENTITY.shader_constants();
    world.projection = Matrix4::IDENTITY.shader_constants();
    camera.prepare(&gpu, &world, [1, 1], [0.0; 2], 0.0);

    let mut renderer = SkinRenderer::with_temporal(
        &gpu,
        crate::COLOR_FORMAT,
        wgpu::TextureFormat::Depth32Float,
        VendorClass::DEFAULT,
        Some(&camera),
    );
    let arena = SkinArena {
        vertices: [
            [-1.0, -1.0, 0.25],
            [1.0, -1.0, 0.25],
            [-1.0, 1.0, 0.25],
            [1.0, 1.0, 0.25],
        ]
        .into_iter()
        .map(|position| SkinVertexGpu {
            position,
            normal: [0.0, 0.0, -1.0],
            uv: [0.0; 2],
            bone_indices: [0; 4],
            bone_weights: [1.0, 0.0, 0.0, 0.0],
        })
        .collect(),
        indices: vec![0, 1, 2, 2, 1, 3],
        allocs: vec![SkinAlloc {
            base_vertex: 0,
            start_index: 0,
            index_count: 6,
            vertex_count: 4,
            palette_len: 1,
        }],
    };
    renderer.upload_arena(&gpu, &arena);
    let color = crate::Offscreen::new(&gpu, 1, 1);
    let motion = crate::Offscreen::with_format(&gpu, 1, 1, crate::temporal::MOTION_FORMAT);
    let bias = crate::Offscreen::with_format(&gpu, 1, 1, crate::temporal::BIAS_FORMAT);
    let depth = crate::DepthBuffer::with_format(&gpu, 1, 1, wgpu::TextureFormat::Depth32Float);
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("temporal skin preservation readback"),
        size: 512,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    for (pass, preservation, expected) in [
        (SkinPass::Opaque, 0.0, 0_u8),
        (SkinPass::Opaque, 0.25, 64),
        (SkinPass::Opaque, 1.0, 255),
        (SkinPass::EffectAlpha, 0.0, 255),
    ] {
        let object = SkinObject::new(
            &SkinParams {
                diffuse: [1.0, 1.0, 1.0, 0.5],
                ambient: [0.0; 4],
                emissive: [0.0; 4],
                unlit: true,
                fog_start: 100.0,
                fog_end: 200.0,
                fog_enabled: false,
                force_opaque: false,
                modulate_alpha: false,
                glow: None,
            },
            1,
            0.0,
        )
        .with_neural_preservation(preservation);
        let draw = SkinDraw {
            pass,
            object: 0,
            part: 0,
            palette: 0,
            texture: None,
        };
        renderer.reset_motion_history();

        for _ in 0..2 {
            renderer.prepare_with_history(
                &gpu,
                &SkinFrame::new(&Matrix4::IDENTITY, &Matrix4::IDENTITY, [0.0; 4]),
                &[object],
                &[BonePalette::default()],
                &[draw],
                0,
                &[Some((7, 11))],
            );
        }

        assert_eq!(renderer.draw_count(), 1);
        assert!(!renderer.drops().any());

        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let attachments = [&color.view, &motion.view, &bias.view].map(|view| {
                Some(wgpu::RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })
            });
            let mut render = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("temporal skin material preservation"),
                color_attachments: &attachments,
                depth_stencil_attachment: Some(depth.attachment()),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            render.set_bind_group(4, &camera.binding, &[]);
            renderer.draw(&mut render, &[]);
        }

        for (texture, offset) in [(&bias.texture, 0), (&color.texture, 256)] {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(1),
                    },
                },
                texture.size(),
            );
        }

        let submission = gpu.queue.submit([encoder.finish()]);
        let slice = readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).expect("send temporal skin readback");
        });
        gpu.device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(30)),
            })
            .expect("wait for temporal skin draw");
        receiver
            .recv_timeout(Duration::from_secs(30))
            .expect("receive temporal skin readback")
            .expect("map temporal skin readback");
        let mapped = slice
            .get_mapped_range()
            .expect("access temporal skin readback");
        assert!(mapped[256] > 0, "skin geometry must cover the pixel");
        assert_eq!(mapped[0], expected, "{pass:?}, preservation {preservation}");
        drop(mapped);
        readback.unmap();
    }

    assert!(block_on(scope.pop()).is_none());
}
