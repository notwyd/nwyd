use super::*;
use crate::target::{COLOR_FORMAT, DepthBuffer, Offscreen};
use formats::world::trn::TrnCell;
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};
use std::time::Duration;

const WIDTH: u32 = 256;
const HEIGHT: u32 = 128;
const CLEAR: [u8; 4] = [255, 0, 255, 255];

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

fn field(zone_x: u8, height: i8) -> TerrainMesh {
    build_terrain(&Terrain {
        name: "synthetic-seam".into(),
        zone_x,
        zone_y: 0,
        cells: vec![
            TrnCell {
                height,
                uv_mode: 0,
                uv_mode2: 0,
                pad: 0,
                tile0: 0,
                tile1: 0,
                color: [255; 4],
            };
            TRN_DIM * TRN_DIM
        ],
    })
}

fn uniforms(mesh: &TerrainMesh) -> FrameUniforms {
    FrameUniforms::new(
        mesh.world,
        mesh.normal_matrix,
        Matrix4::look_at_lh(
            Vec3::new(140.0, 10.0, 64.0),
            Vec3::new(128.0, 1.0, 64.0),
            Vec3::Y,
        ),
        Matrix4::perspective_fov_lh(
            std::f32::consts::FRAC_PI_4,
            WIDTH as f32 / HEIGHT as f32,
            0.1,
            200.0,
        ),
        &Lighting {
            light0: [0.0; 4],
            light1: [0.0; 4],
            material: Material {
                emissive: [0.1, 0.6, 0.2, 0.0],
                ..Material::TERRAIN
            },
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

fn render(gpu: &Gpu, renderer: &mut TerrainRenderer, mesh: &TerrainMesh) -> Vec<u8> {
    let pixels = render_uniforms(gpu, renderer, mesh, &uniforms(mesh));

    assert_eq!(renderer.draw_count(), 2);

    pixels
}

fn render_uniforms(
    gpu: &Gpu,
    renderer: &mut TerrainRenderer,
    mesh: &TerrainMesh,
    uniforms: &FrameUniforms,
) -> Vec<u8> {
    let tiles = renderer.bind_null_tiles(gpu);
    let target = Offscreen::new(gpu, WIDTH, HEIGHT);
    let depth = DepthBuffer::with_format(gpu, WIDTH, HEIGHT, wgpu::TextureFormat::Depth32Float);

    renderer.prepare(gpu, &mesh.batches, &tiles, uniforms, false);

    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("terrain seam readback"),
        size: u64::from(WIDTH * HEIGHT * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("terrain seam regression"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 1.0,
                        g: 0.0,
                        b: 1.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(depth.attachment()),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        renderer.draw(&mut pass, &tiles);
    }

    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &target.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(WIDTH * 4),
                rows_per_image: Some(HEIGHT),
            },
        },
        wgpu::Extent3d {
            width: WIDTH,
            height: HEIGHT,
            depth_or_array_layers: 1,
        },
    );

    let submission = gpu.queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);

    slice.map_async(wgpu::MapMode::Read, move |result| {
        sender.send(result).expect("send terrain readback result");
    });

    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(30)),
        })
        .expect("wait for terrain draw");

    receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("receive terrain readback result")
        .expect("map terrain readback");

    let pixels = slice
        .get_mapped_range()
        .expect("access terrain readback")
        .to_vec();

    readback.unmap();

    pixels
}

fn fog_surface(columns: usize) -> TerrainMesh {
    let mut vertices = Vec::new();

    for column in 0..columns {
        for (x, y) in [
            (column, -1.0),
            (column, 1.0),
            (column + 1, -1.0),
            (column + 1, 1.0),
        ] {
            let t = x as f32 / columns as f32;

            vertices.push(WorldVertex {
                pos: [t * 2.0 - 1.0, y, 0.1 + 0.8 * t],
                normal: [0.0; 3],
                diffuse: 0xffffffff,
                uv0: [0.0; 2],
                uv1: [0.0; 2],
            });
        }
    }

    TerrainMesh {
        vertices,
        batches: vec![TerrainBatch {
            base_texture_id: 0,
            detail_texture_id: 0,
            material: TerrainMaterial::Base,
            first_quad: 0,
            quad_count: columns as u32,
            bounds: crate::cull::Aabb {
                min: [-1.0, -1.0, 0.1],
                max: [1.0, 1.0, 0.9],
            },
        }],
        world: Matrix4::IDENTITY,
        normal_matrix: Matrix4::IDENTITY,
        zone: (0, 0),
    }
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn distance_fog_preserves_detail_and_is_independent_of_tessellation() {
    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    let mut renderer = TerrainRenderer::new(&gpu, COLOR_FORMAT, wgpu::TextureFormat::Depth32Float);
    let surface = fog_surface(1);
    let mut frame = uniforms(&surface);

    frame.view = Matrix4::IDENTITY.shader_constants();
    frame.projection = Matrix4::IDENTITY.shader_constants();
    frame.material_emissive = [1.0; 4];
    frame.fog_color = [0.0; 4];
    frame.fog_params = [0.3, 0.7, 2.5, 1.0];

    renderer.upload_mesh(&gpu, &surface);

    let coarse = render_uniforms(&gpu, &mut renderer, &surface, &frame);
    let surface = fog_surface(64);

    renderer.upload_mesh(&gpu, &surface);

    let fine = render_uniforms(&gpu, &mut renderer, &surface, &frame);

    for (a, b) in coarse.iter().zip(&fine) {
        assert!(
            a.abs_diff(*b) <= 1,
            "fog changes with surface tessellation: {a} vs {b}"
        );
    }

    let channel = |x: u32| coarse[((HEIGHT / 2 * WIDTH + x) * 4) as usize];

    assert_eq!(channel(32), 255);
    assert!(
        (210..=218).contains(&channel(128)),
        "midrange detail: {}",
        channel(128)
    );
    assert_eq!(channel(224), 0);

    for x in 1..WIDTH {
        assert!(channel(x) <= channel(x - 1));
        assert!(channel(x).abs_diff(channel(x - 1)) <= 5);
    }

    frame.fog_params[3] = 0.0;

    let disabled = render_uniforms(&gpu, &mut renderer, &surface, &frame);

    assert!(disabled.chunks_exact(4).all(|pixel| pixel == [255; 4]));

    frame.fog_params = [0.24, 0.35, 1.0 / 0.11, 1.0];

    let legacy_range = render_uniforms(&gpu, &mut renderer, &surface, &frame);

    frame.fog_params = [0.24, 0.63, 1.0 / 0.39, 1.0];

    let extended_range = render_uniforms(&gpu, &mut renderer, &surface, &frame);
    let middle = ((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize;
    let far = ((HEIGHT / 2 * WIDTH + 200) * 4) as usize;

    assert_eq!(legacy_range[middle], 0);
    assert!(extended_range[middle] > 140);
    assert_eq!(extended_range[far], 0);
}

fn interior_clear_pixels(pixels: &[u8]) -> usize {
    let mut count = 0;

    for y in 16..HEIGHT - 16 {
        for x in 32..WIDTH - 32 {
            let start = ((y * WIDTH + x) * 4) as usize;

            count += usize::from(pixels[start..start + 4] == CLEAR);
        }
    }

    count
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn stitched_terrain_closes_gap_and_updates_resident_vertices() {
    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    let new_renderer =
        || TerrainRenderer::new(&gpu, COLOR_FORMAT, wgpu::TextureFormat::Depth32Float);
    let mut resident = new_renderer();
    let mut mesh = field(0, 20);
    let resident_vertices = mesh.vertices.len();

    resident.upload_mesh(&gpu, &mesh);

    mesh.append_neighbour(&field(1, 0), TRN_DIM as i32, 0);

    let mut unstitched = new_renderer();

    unstitched.upload_mesh(&gpu, &mesh);

    let before = render(&gpu, &mut unstitched, &mesh);
    let gap_pixels = interior_clear_pixels(&before);

    assert!(
        gap_pixels > WIDTH as usize,
        "expected a visible gap, found {gap_pixels} pixels"
    );
    assert!(gap_pixels < 96 * 192, "terrain must render around the gap");

    let changes = mesh.stitch_boundaries();

    assert!(changes.iter().any(|range| range.start < resident_vertices));

    resident.update_mesh(&gpu, &mesh, changes.into_iter());

    let updated = render(&gpu, &mut resident, &mesh);

    assert_eq!(
        interior_clear_pixels(&updated),
        0,
        "stitched edge must cover the background"
    );

    let mut fresh = new_renderer();

    fresh.upload_mesh(&gpu, &mesh);

    let expected = render(&gpu, &mut fresh, &mesh);

    assert_eq!(
        updated, expected,
        "resident patches must match a fresh upload"
    );
    assert!(resident.vertex_upload_stats().copied_bytes > 0);
}
