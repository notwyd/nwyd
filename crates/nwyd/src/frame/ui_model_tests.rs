use super::*;
use render::Matrix4;
use render::mesh::{
    Cull, DrawParams, MeshArena, MeshDraw, MeshVertex, ObjectUniforms, SubsetRange,
};
use render::target::Offscreen;
use render::world::{Fog, Lighting, Material, PointLights};
use std::time::Duration;

fn uniforms() -> FrameUniforms {
    FrameUniforms::new(
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
    )
}

fn mesh(
    arena: &mut MeshArena,
    frame: &mut crate::scene::objects::Frame,
    rect: [f32; 4],
    depth: f32,
    color: [f32; 4],
) {
    let [left, top, right, bottom] = rect;
    let base_vertex = arena.vertices.len() as i32;
    let start_index = arena.indices.len() as u32;
    arena.vertices.extend(
        [(left, top), (left, bottom), (right, top), (right, bottom)].map(|(x, y)| MeshVertex {
            position: [x, y, depth],
            normal: [0.0; 3],
            uv: [0.5; 2],
            diffuse: u32::MAX,
        }),
    );
    arena.indices.extend([0, 1, 2, 2, 1, 3]);
    let mut object = ObjectUniforms::new(Matrix4::IDENTITY, &DrawParams::glow(color), 0.0);
    object.set_null_texture();
    let index = frame.objects.len() as u32;
    frame.objects.push(object);
    frame.draws.push(MeshDraw {
        pass: MeshPass::Opaque,
        cull: Cull::None,
        object: index,
        texture: render::mesh::NULL_TEXTURE,
        stage1: None,
        base_vertex,
        subset: SubsetRange {
            start_index,
            index_count: 6,
        },
    });
}

fn pixels(gpu: &Gpu, target: &Offscreen) -> Vec<u8> {
    let stride = (target.width * 4).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(stride) * u64::from(target.height),
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
                bytes_per_row: Some(stride),
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
    let result = slice
        .get_mapped_range()
        .unwrap()
        .chunks_exact(stride as usize)
        .flat_map(|row| row[..target.width as usize * 4].iter().copied())
        .collect();
    buffer.unmap();
    result
}

#[test]
#[ignore = "requires a graphics adapter"]
fn ui_models_keep_their_own_depth_and_preserve_overlay_order_after_resize() {
    let gpu = pollster::block_on(Gpu::open(Gpu::instance(), None)).expect("graphics adapter");
    let scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let mut renderer = SceneRenderer::new(&gpu, render::target::COLOR_FORMAT);
    let mut arena = MeshArena::default();
    let mut frame = crate::scene::objects::Frame {
        objects: Vec::new(),
        draws: Vec::new(),
        light_shafts: Vec::new(),
    };
    mesh(
        &mut arena,
        &mut frame,
        [-0.8, -0.8, 0.8, 0.8],
        0.2,
        [1.0, 0.0, 0.0, 1.0],
    );
    mesh(
        &mut arena,
        &mut frame,
        [-0.8, -0.8, 0.8, 0.8],
        0.8,
        [0.0, 0.0, 1.0, 1.0],
    );
    mesh(
        &mut arena,
        &mut frame,
        [0.0, -0.8, 0.8, 0.8],
        0.9,
        [0.0, 1.0, 0.0, 1.0],
    );
    let mut models = UiModels {
        renderer: MeshRenderer::new_ui(&gpu, render::target::COLOR_FORMAT, renderer.vendor),
        depth: None,
        textures: Vec::new(),
        resident: None,
        spans: Vec::new(),
        prepared: false,
    };
    models.renderer.upload_arena(&gpu, &arena);
    renderer.ui_models = Some(models);

    let mut world_arena = MeshArena::default();
    let mut world_frame = crate::scene::objects::Frame {
        objects: Vec::new(),
        draws: Vec::new(),
        light_shafts: Vec::new(),
    };
    mesh(
        &mut world_arena,
        &mut world_frame,
        [-1.0, -1.0, 1.0, 1.0],
        0.05,
        [0.0, 0.0, 1.0, 1.0],
    );
    renderer.meshes.upload_arena(&gpu, &world_arena);

    for (size, world) in [((64, 48), false), ((128, 96), true)] {
        renderer.begin();
        if world {
            renderer.queue_world_at(&gpu, uniforms(), &world_frame, 0);
        }
        let models = renderer.ui_models.as_mut().unwrap();
        models
            .renderer
            .prepare(&gpu, &uniforms(), &frame.objects, &frame.draws);
        models.prepared = true;

        let mut background = Quad::whole(0.0, 0.0, 1, 1, 0xFF80_8080);
        background.dst_w = size.0 as f32;
        background.dst_h = size.1 as f32;
        renderer.push(&PlannedQuad {
            mode: RenderMode::NuiPanel2d,
            texture: TextureKey::White,
            quad: background,
            uv: UvRule::TexelEdge,
        });
        renderer.push_ui_model(0..2);
        renderer.push_ui_model(2..3);
        let mut foreground = Quad::whole(0.0, 0.0, 1, 1, 0xFFFF_FF00);
        foreground.dst_w = size.0 as f32 / 2.0;
        foreground.dst_h = size.1 as f32 / 3.0;
        renderer.push(&PlannedQuad {
            mode: RenderMode::NuiPanel2d,
            texture: TextureKey::White,
            quad: foreground,
            uv: UvRule::TexelEdge,
        });

        renderer.prepare_for_targets(&gpu, (size.0 / 2, size.1 / 2), size);
        assert!(
            renderer
                .ui_models
                .as_ref()
                .unwrap()
                .depth
                .as_ref()
                .unwrap()
                .fits(size.0, size.1)
        );
        if world {
            assert!(
                renderer
                    .depth
                    .as_ref()
                    .unwrap()
                    .fits(size.0 / 2, size.1 / 2)
            );
        }

        let target = Offscreen::new(&gpu, size.0, size.1);
        renderer.prepare_frame(&gpu, size).submit(&target.view);
        let image = pixels(&gpu, &target);
        let pixel = |x: u32, y: u32| {
            let offset = ((y * size.0 + x) * 4) as usize;
            &image[offset..offset + 4]
        };
        assert_eq!(pixel(size.0 / 4, size.1 / 2), &[255, 0, 0, 255]);
        assert_eq!(pixel(size.0 * 3 / 4, size.1 / 2), &[0, 255, 0, 255]);
        assert_eq!(pixel(size.0 / 4, size.1 / 4), &[255, 255, 0, 255]);
        assert_eq!(pixel(size.0 - 1, size.1 - 1), &[128, 128, 128, 255]);
    }
    assert!(pollster::block_on(scope.pop()).is_none());
}
