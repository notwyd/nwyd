use super::*;
use render::Matrix4;
use render::blit::Blitter;
use render::mesh::{
    Cull, DrawParams, MeshArena, MeshDraw, MeshVertex, ObjectUniforms, SubsetRange,
};
use render::target::Offscreen;
use render::timing::GpuTimings;
use render::world::{Fog, Lighting, Material, PointLights};
use std::time::{Duration, Instant};

fn read(gpu: &Gpu, target: &Offscreen) -> Vec<u8> {
    let bytes_per_row = (target.width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("frame timing regression readback"),
        size: u64::from(bytes_per_row) * u64::from(target.height),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        target.texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(bytes_per_row),
                rows_per_image: Some(target.height),
            },
        },
        target.texture.size(),
    );
    let submission = gpu.queue.submit([encoder.finish()]);
    let slice = buffer.slice(..);
    let (send, receive) = std::sync::mpsc::channel();
    slice.map_async(wgpu::MapMode::Read, move |result| {
        send.send(result).unwrap();
    });
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(30)),
        })
        .unwrap();
    receive
        .recv_timeout(Duration::from_secs(30))
        .unwrap()
        .unwrap();
    let pixels = slice
        .get_mapped_range()
        .unwrap()
        .chunks_exact(bytes_per_row as usize)
        .flat_map(|row| row[..target.width as usize * 4].iter().copied())
        .collect();
    buffer.unmap();
    pixels
}

fn queue(renderer: &mut SceneRenderer, gpu: &Gpu, world: bool) {
    renderer.begin();
    renderer.clear = wgpu::Color::BLUE;
    if world {
        let uniforms = FrameUniforms::new(
            Matrix4::IDENTITY,
            Matrix4::IDENTITY,
            Matrix4::IDENTITY,
            Matrix4::IDENTITY,
            &Lighting {
                light0: [0.0; 4],
                light1: [0.0; 4],
                material: Material::TERRAIN,
                global_ambient: 0,
                point_lights: PointLights::default(),
            },
            &Fog {
                enabled: false,
                color: 0,
                start: 100.0,
                end: 200.0,
            },
        );
        let mut object = ObjectUniforms::new(
            Matrix4::IDENTITY,
            &DrawParams::glow([1.0, 0.0, 0.0, 1.0]),
            0.0,
        );
        object.set_null_texture();
        renderer.queue_world_at(
            gpu,
            uniforms,
            &crate::scene::objects::Frame {
                objects: vec![object],
                draws: vec![MeshDraw {
                    pass: MeshPass::Opaque,
                    cull: Cull::None,
                    object: 0,
                    texture: render::mesh::NULL_TEXTURE,
                    stage1: None,
                    base_vertex: 0,
                    subset: SubsetRange {
                        start_index: 0,
                        index_count: 6,
                    },
                }],
                light_shafts: Vec::new(),
            },
            0,
        );
    }
    let mut quad = Quad::whole(8.0, 9.0, 1, 1, u32::MAX);
    quad.dst_w = 22.0;
    quad.dst_h = 4.0;
    quad.rotation = 0.3;
    renderer.push(&PlannedQuad {
        mode: RenderMode::NuiPanel2d,
        texture: TextureKey::White,
        quad,
        uv: UvRule::TexelEdge,
    });
}

#[test]
#[ignore = "requires a graphics adapter with timestamp queries"]
fn prepared_scene_and_blit_timings_preserve_pixels_and_recycle_slots() {
    let gpu = pollster::block_on(Gpu::open(Gpu::instance(), None)).expect("graphics adapter");
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut timings = GpuTimings::new(&gpu).expect("timestamp support");
    let size = (64, 48);
    let output_size = (96, 72);
    let scene = Offscreen::new(&gpu, size.0, size.1);
    let expected = Offscreen::new(&gpu, output_size.0, output_size.1);
    let measured = Offscreen::new(&gpu, output_size.0, output_size.1);
    let blitter = Blitter::new(&gpu, expected.format);
    let source = blitter.source(&gpu, &scene);
    let arena = MeshArena {
        vertices: [(-0.7, -0.6), (-0.7, 0.6), (0.7, -0.6), (0.7, 0.6)]
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

    for (mode, (world, motion)) in [(false, false), (false, true), (true, false), (true, true)]
        .into_iter()
        .enumerate()
    {
        let mut renderer = if motion {
            SceneRenderer::new_with_motion(&gpu, scene.format, size)
        } else {
            SceneRenderer::new(&gpu, scene.format)
        };
        assert_eq!(renderer.motion_vectors().is_some(), motion);
        renderer.meshes.upload_arena(&gpu, &arena);
        queue(&mut renderer, &gpu, world);
        assert_eq!(renderer.has_world(), world);
        renderer.prepare_frame(&gpu, size).submit(&scene.view);
        blitter.render(&gpu, &source, &expected.view, output_size);
        let pixels = read(&gpu, &expected);
        assert!(pixels.chunks_exact(4).any(|pixel| pixel[..3] == [255; 3]));
        assert_eq!(
            pixels
                .chunks_exact(4)
                .any(|pixel| pixel[..3] == [255, 0, 0]),
            world,
        );

        for repeat in 0..6 {
            let frame_id = (mode as u64 + 1) * 100 + repeat;
            let timing = timings.begin_frame(frame_id).expect("recycled timing slot");
            renderer.reset_temporal_history();
            queue(&mut renderer, &gpu, world);
            renderer
                .prepare_frame(&gpu, size)
                .submit_timed(&scene.view, Some(&timing));
            blitter.render_timed(&gpu, &source, &measured.view, output_size, Some(&timing));
            timings.finish_frame(timing);
            let deadline = Instant::now() + Duration::from_secs(5);
            let completed = loop {
                let samples = timings.poll(&gpu);
                if !samples.is_empty() {
                    break samples;
                }
                assert!(
                    Instant::now() < deadline,
                    "frame {frame_id} was not resolved"
                );
                std::thread::yield_now();
            };
            assert_eq!(completed.len(), 1);
            assert_eq!(completed[0].frame_id, frame_id);
            assert!(completed[0].scene_ms.is_finite() && completed[0].scene_ms >= 0.0);
            assert!(completed[0].blit_ms.is_finite() && completed[0].blit_ms >= 0.0);
            assert!(!timings.has_pending());
            assert!(timings.poll(&gpu).is_empty());
            assert_eq!(read(&gpu, &measured), pixels, "frame {frame_id}");
        }
    }
    assert!(pollster::block_on(scope.pop()).is_none());
}
