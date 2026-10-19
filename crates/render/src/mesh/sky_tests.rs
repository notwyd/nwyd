use super::*;
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

pub(super) fn block_on<T>(future: impl Future<Output = T>) -> T {
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

#[test]
fn sky_uses_background_depth_without_writing_scene_depth() {
    let preset = Preset::lit_3d(VendorClass::DEFAULT);
    let state = MeshPass::Sky
        .depth_state(&preset, wgpu::TextureFormat::Depth32Float)
        .unwrap();
    assert_eq!(state.depth_write_enabled, Some(false));
    assert_eq!(state.depth_compare, Some(wgpu::CompareFunction::LessEqual));
    assert_eq!(
        MeshPass::Opaque
            .depth_state(&preset, wgpu::TextureFormat::Depth32Float)
            .unwrap()
            .depth_write_enabled,
        Some(true)
    );
}

fn scene() -> (MeshArena, Vec<ObjectUniforms>, Vec<MeshDraw>) {
    let mut arena = MeshArena::default();
    let mut objects = Vec::new();
    let mut draws = Vec::new();

    for (pass, bounds, distance, color) in [
        (MeshPass::Opaque, [-1.0, -0.4], 140.0, [1.0, 0.0, 0.0, 1.0]),
        (MeshPass::Sky, [-1.1, 1.1], 80.0, [0.0, 0.0, 1.0, 1.0]),
        (MeshPass::Opaque, [0.4, 1.0], 180.0, [0.0, 1.0, 0.0, 1.0]),
    ] {
        let base_vertex = arena.vertices.len() as i32;
        let start_index = arena.indices.len() as u32;
        let [left, right] = bounds;
        arena.vertices.extend(
            [(left, -1.1), (left, 1.1), (right, -1.1), (right, 1.1)].map(|(x, y)| MeshVertex {
                position: [x * distance, y * distance, distance],
                normal: [0.0; 3],
                uv: [0.0; 2],
                diffuse: 0xffffffff,
            }),
        );
        arena.indices.extend([0, 1, 2, 2, 1, 3]);
        let mut object = ObjectUniforms::new(Matrix4::IDENTITY, &DrawParams::glow(color), 0.0);
        object.set_null_texture();
        draws.push(MeshDraw {
            pass,
            cull: Cull::Ccw,
            object: objects.len() as u32,
            texture: NULL_TEXTURE,
            stage1: None,
            base_vertex,
            subset: SubsetRange {
                start_index,
                index_count: 6,
            },
        });
        objects.push(object);
    }

    (arena, objects, draws)
}

pub(super) fn readback(
    gpu: &Gpu,
    buffer: &wgpu::Buffer,
    submission: wgpu::SubmissionIndex,
) -> Vec<u8> {
    let slice = buffer.slice(..);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);
    slice.map_async(wgpu::MapMode::Read, move |result| {
        sender.send(result).unwrap();
    });
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(30)),
        })
        .unwrap();
    receiver
        .recv_timeout(Duration::from_secs(30))
        .unwrap()
        .unwrap();
    let data = slice.get_mapped_range().unwrap().to_vec();
    buffer.unmap();
    data
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn sky_stays_behind_distant_geometry_and_survives_far_plane_changes() {
    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let color = crate::Offscreen::new(&gpu, 64, 1);
    let motion = crate::Offscreen::with_format(&gpu, 64, 1, crate::temporal::MOTION_FORMAT);
    let bias = crate::Offscreen::with_format(&gpu, 64, 1, crate::temporal::BIAS_FORMAT);
    let depth = crate::DepthBuffer::with_format(&gpu, 64, 1, wgpu::TextureFormat::Depth32Float);
    let readback_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("sky depth readback"),
        size: 1024,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });

    for temporal in [false, true] {
        let mut camera = crate::temporal::TemporalCamera::new(&gpu);
        let mut renderer = MeshRenderer::with_temporal(
            &gpu,
            crate::COLOR_FORMAT,
            wgpu::TextureFormat::Depth32Float,
            VendorClass::DEFAULT,
            temporal.then_some(&camera),
        );
        let (arena, mut objects, draws) = scene();
        renderer.upload_arena(&gpu, &arena);

        for far in [240.0, 60.0, 120.0, 480.0] {
            let mut frame = FrameUniforms::zeroed();
            frame.view = Matrix4::IDENTITY.shader_constants();
            frame.projection =
                Matrix4::perspective_fov_lh(std::f32::consts::FRAC_PI_2, 1.0, 1.0, far)
                    .shader_constants();
            objects[1].world = Matrix4::IDENTITY.shader_constants();
            camera.prepare(&gpu, &frame, [64, 1], [0.0; 2], 0.0);
            renderer.reset_motion_history();

            for sky_translation in [0.0, 1.0] {
                objects[1].world =
                    Matrix4::translation(sky_translation, 0.0, 0.0).shader_constants();
                renderer.prepare_with_history(
                    &gpu,
                    &frame,
                    &objects,
                    &draws,
                    &[Some((1, 0)), Some((2, 0)), Some((3, 0))],
                );
                let mut encoder = gpu.device.create_command_encoder(&Default::default());
                {
                    let views: Vec<_> = if temporal {
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
                                    load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                                    store: wgpu::StoreOp::Store,
                                },
                            })
                        })
                        .collect();
                    let mut render = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                        label: Some("sky behind world and actors"),
                        color_attachments: &attachments,
                        depth_stencil_attachment: Some(depth.attachment()),
                        timestamp_writes: None,
                        occlusion_query_set: None,
                        multiview_mask: None,
                    });
                    if temporal {
                        render.set_bind_group(4, &camera.binding, &[]);
                    }
                    renderer.draw_range(&mut render, &[], 0..1);
                    renderer.draw_range(&mut render, &[], 1..2);
                    renderer.draw_range(&mut render, &[], 2..3);
                }
                let mut textures = vec![
                    (&color.texture, wgpu::TextureAspect::All),
                    (&depth.texture, wgpu::TextureAspect::DepthOnly),
                ];
                if temporal {
                    textures.extend([
                        (&motion.texture, wgpu::TextureAspect::All),
                        (&bias.texture, wgpu::TextureAspect::All),
                    ]);
                }
                for (index, (texture, aspect)) in textures.into_iter().enumerate() {
                    encoder.copy_texture_to_buffer(
                        wgpu::TexelCopyTextureInfo {
                            texture,
                            mip_level: 0,
                            origin: wgpu::Origin3d::ZERO,
                            aspect,
                        },
                        wgpu::TexelCopyBufferInfo {
                            buffer: &readback_buffer,
                            layout: wgpu::TexelCopyBufferLayout {
                                offset: index as u64 * 256,
                                bytes_per_row: Some(256),
                                rows_per_image: Some(1),
                            },
                        },
                        texture.size(),
                    );
                }
                let pixels = readback(&gpu, &readback_buffer, gpu.queue.submit([encoder.finish()]));
                for (x, expected) in [(8, [255, 0, 0, 255]), (56, [0, 255, 0, 255])] {
                    let expected = if far < 180.0 {
                        [0, 0, 255, 255]
                    } else {
                        expected
                    };
                    assert_eq!(
                        &pixels[x * 4..x * 4 + 4],
                        &expected,
                        "temporal {temporal}, far {far}, x {x}"
                    );
                }
                assert_eq!(&pixels[128..132], &[0, 0, 255, 255]);
                assert_eq!(
                    f32::from_le_bytes(pixels[384..388].try_into().unwrap()),
                    1.0
                );
                if temporal {
                    let motion_x = u16::from_le_bytes(pixels[640..642].try_into().unwrap());
                    if sky_translation == 0.0 {
                        assert_eq!(motion_x & 0x7fff, 0);
                    } else {
                        assert!(
                            motion_x.abs_diff(0xb666) <= 1,
                            "sky motion {motion_x:x}, expected b666"
                        );
                    }
                    let motion_y = u16::from_le_bytes(pixels[642..644].try_into().unwrap());
                    assert_eq!(motion_y & 0x7fff, 0);
                    assert_eq!(pixels[800], 255);
                }
            }
        }
    }
    assert!(block_on(scope.pop()).is_none());
}
