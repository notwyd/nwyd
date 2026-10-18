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

fn projection() -> Matrix4 {
    Matrix4::perspective_fov_lh(std::f32::consts::FRAC_PI_2, 2.0, 1.0, 20.0)
}

fn shaft() -> LightShaft {
    LightShaft {
        world: Matrix4::multiply(
            &Matrix4::scaling(1.5, 0.75, 2.0),
            &Matrix4::translation(0.0, 0.0, 5.0),
        ),
        color: [1.0; 3],
        density: 0.2,
        source_width: [1.0; 2],
        texture: 0,
        uv_scale_offset: [1.0, 1.0, 0.0, 0.0],
    }
}

#[test]
fn projected_volumes_clip_offscreen_and_keep_near_plane_intersections() {
    let projection = Mat4::from_cols_array(&projection().shader_constants());
    let bounds = |world: Matrix4| {
        projected_bounds(
            Mat4::from_cols_array(&world.shader_constants()) * projection,
            [64, 32],
            [0.0; 2],
        )
    };
    let [x, y, width, height] = bounds(shaft().world).unwrap();
    assert!(x > 0 && y > 0 && width < 64 && height < 32);
    assert_eq!(bounds(Matrix4::translation(50.0, 0.0, 5.0)), None);
    assert_eq!(bounds(Matrix4::translation(0.0, 0.0, 30.0)), None);
    assert_eq!(bounds(Matrix4::translation(0.0, 0.0, -5.0)), None);
    assert_eq!(
        bounds(Matrix4::multiply(
            &Matrix4::scaling(0.1, 0.1, 0.1),
            &Matrix4::translation(0.0, 0.0, 0.5),
        )),
        None
    );
    assert_eq!(
        bounds(Matrix4::translation(0.0, 0.0, 1.0)),
        Some([0, 0, 64, 32])
    );
}

struct Scene {
    gpu: Gpu,
    renderer: LightShaftRenderer,
    color: crate::Offscreen,
    reactive: crate::Offscreen,
    depth: crate::DepthBuffer,
    textures: Vec<Option<LightShaftTexture>>,
    readback: wgpu::Buffer,
    frame: FrameUniforms,
}

impl Scene {
    fn new(depth_format: wgpu::TextureFormat) -> Self {
        let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
        let renderer = LightShaftRenderer::new(&gpu, crate::COLOR_FORMAT);
        let color = crate::Offscreen::new(&gpu, 64, 32);
        let reactive = crate::Offscreen::with_format(&gpu, 64, 32, crate::temporal::BIAS_FORMAT);
        let depth = crate::DepthBuffer::with_format(&gpu, 64, 32, depth_format);
        let textures = [[255, 255, 255, 0], [0, 0, 0, 255]]
            .map(|rgba| {
                let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
                    label: Some("synthetic shaft mask"),
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
                    &rgba,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(4),
                        rows_per_image: Some(1),
                    },
                    texture.size(),
                );
                Some(renderer.bind_texture(&gpu, &texture.create_view(&Default::default())))
            })
            .into();
        let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("shaft readback"),
            size: 256 * 32 * 3,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let frame = FrameUniforms::new(
            Matrix4::IDENTITY,
            Matrix4::IDENTITY,
            Matrix4::IDENTITY,
            projection(),
            &crate::world::Lighting {
                light0: [0.0; 4],
                light1: [0.0; 4],
                material: crate::world::Material::TERRAIN,
                global_ambient: 0,
                point_lights: crate::world::PointLights::default(),
            },
            &crate::world::Fog {
                enabled: false,
                color: 0,
                start: 0.0,
                end: 20.0,
            },
        );
        Self {
            gpu,
            renderer,
            color,
            reactive,
            depth,
            textures,
            readback,
            frame,
        }
    }

    fn render(
        &mut self,
        shafts: &[LightShaft],
        distance: f32,
        reactive: Option<f64>,
        jitter: [f32; 2],
    ) -> Vec<u8> {
        self.renderer
            .prepare(&self.gpu, &self.frame, shafts, &self.depth.view, jitter);
        let depth = projection()
            .transform_coord(Vec3::new(0.0, 0.0, distance))
            .z;
        let mut encoder = self.gpu.device.create_command_encoder(&Default::default());
        let color_attachment = |view, clear| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(clear),
                    store: wgpu::StoreOp::Store,
                },
            })
        };
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("synthetic shaft scene"),
                color_attachments: &[
                    color_attachment(&self.color.view, wgpu::Color::BLACK),
                    color_attachment(
                        &self.reactive.view,
                        wgpu::Color {
                            r: reactive.unwrap_or(0.0),
                            g: 0.0,
                            b: 0.0,
                            a: 1.0,
                        },
                    ),
                ],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &self.depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(depth),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
        }
        self.renderer.encode(
            &mut encoder,
            &self.color.view,
            &self.textures,
            reactive.map(|_| &self.reactive.view),
        );
        for (index, (texture, aspect)) in [
            (&self.color.texture, wgpu::TextureAspect::All),
            (&self.reactive.texture, wgpu::TextureAspect::All),
            (&self.depth.texture, wgpu::TextureAspect::DepthOnly),
        ]
        .into_iter()
        .enumerate()
        {
            encoder.copy_texture_to_buffer(
                wgpu::TexelCopyTextureInfo {
                    texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect,
                },
                wgpu::TexelCopyBufferInfo {
                    buffer: &self.readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: index as u64 * 256 * 32,
                        bytes_per_row: Some(256),
                        rows_per_image: Some(32),
                    },
                },
                texture.size(),
            );
        }
        let submission = self.gpu.queue.submit([encoder.finish()]);
        let slice = self.readback.slice(..);
        let (sender, receiver) = std::sync::mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap()
        });
        self.gpu
            .device
            .poll(wgpu::PollType::Wait {
                submission_index: Some(submission),
                timeout: Some(Duration::from_secs(30)),
            })
            .unwrap();
        receiver
            .recv_timeout(Duration::from_secs(30))
            .unwrap()
            .unwrap();
        let bytes = slice.get_mapped_range().unwrap().to_vec();
        self.readback.unmap();
        bytes
    }
}

fn center(bytes: &[u8]) -> u8 {
    bytes[16 * 256 + 32 * 4]
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn volumes_respect_opaque_depth_and_soften_intersections_in_both_depth_formats() {
    for format in [crate::DEPTH_FORMAT, wgpu::TextureFormat::Depth32Float] {
        let mut scene = Scene::new(format);
        let scope = scene
            .gpu
            .device
            .push_error_scope(wgpu::ErrorFilter::Validation);
        let mut previous = 0;
        for distance in [2.0, 3.05, 3.3, 4.0, 5.0, 8.0] {
            let bytes = scene.render(&[shaft()], distance, None, [0.0; 2]);
            let current = center(&bytes);
            assert!(
                current >= previous,
                "distance {distance}: {current} < {previous}"
            );
            if distance == 2.0 {
                assert_eq!(current, 0);
            }
            if distance == 3.05 {
                assert!(current <= 2);
            }
            if distance == 8.0 {
                assert!(current > 80);
            }
            previous = current;
            let empty = scene.render(&[], distance, None, [0.0; 2]);
            assert_eq!(&bytes[256 * 32 * 2..], &empty[256 * 32 * 2..]);
        }
        let masked = LightShaft {
            texture: 1,
            ..shaft()
        };
        assert_eq!(center(&scene.render(&[masked], 20.0, None, [0.0; 2])), 0);
        let missing = LightShaft {
            texture: 20,
            ..shaft()
        };
        assert_eq!(center(&scene.render(&[missing], 20.0, None, [0.0; 2])), 0);
        assert!(block_on(scope.pop()).is_none());
    }
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn volumes_keep_thickness_near_camera_and_preserve_reactive_masks() {
    let mut scene = Scene::new(wgpu::TextureFormat::Depth32Float);
    let scope = scene
        .gpu
        .device
        .push_error_scope(wgpu::ErrorFilter::Validation);
    for angle in [
        0.0,
        std::f32::consts::FRAC_PI_4,
        std::f32::consts::FRAC_PI_2,
    ] {
        let volume = LightShaft {
            world: Matrix4::multiply(
                &Matrix4::multiply(
                    &Matrix4::scaling(1.5, 0.75, 2.0),
                    &Matrix4::rotation_x(angle),
                ),
                &Matrix4::translation(0.0, 0.0, 5.0),
            ),
            ..shaft()
        };
        assert!(center(&scene.render(&[volume], 20.0, None, [0.0; 2])) > 20);
    }
    let inside = LightShaft {
        world: Matrix4::multiply(
            &Matrix4::scaling(1.5, 0.75, 2.0),
            &Matrix4::translation(0.0, 0.0, 1.0),
        ),
        ..shaft()
    };
    assert!(center(&scene.render(&[inside], 20.0, None, [0.0; 2])) > 20);
    let plain = scene.render(&[shaft()], 20.0, None, [0.0; 2]);
    let tapered = LightShaft {
        source_width: [0.25; 2],
        ..shaft()
    };
    let narrow = scene.render(&[tapered], 20.0, None, [0.0; 2]);
    assert!(center(&narrow) > 20);
    let edge = 16 * 256 + 28 * 4;
    assert!(f32::from(narrow[edge]) < f32::from(plain[edge]) * 0.7);
    let temporal = scene.render(&[shaft()], 20.0, Some(0.0), [0.0; 2]);
    assert_eq!(&plain[..256 * 32], &temporal[..256 * 32]);
    assert_eq!(temporal[256 * 32 + 16 * 256 + 32], center(&temporal));
    let existing = scene.render(&[shaft()], 20.0, Some(0.75), [0.0; 2]);
    assert_eq!(existing[256 * 32 + 16 * 256 + 32], 191);
    let clear = scene.render(&[], 20.0, Some(0.75), [0.0; 2]);
    assert_eq!(&existing[256 * 32..], &clear[256 * 32..]);
    assert!(block_on(scope.pop()).is_none());
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn ray_reconstruction_follows_camera_jitter_and_depth_target_replacement() {
    let mut scene = Scene::new(wgpu::TextureFormat::Depth32Float);
    let scope = scene
        .gpu
        .device
        .push_error_scope(wgpu::ErrorFilter::Validation);
    let centroid = |bytes: &[u8]| {
        let pixels = &bytes[16 * 256..17 * 256];
        let total: f32 = pixels.chunks_exact(4).map(|p| f32::from(p[0])).sum();
        pixels
            .chunks_exact(4)
            .enumerate()
            .map(|(i, p)| i as f32 * f32::from(p[0]))
            .sum::<f32>()
            / total
    };
    let plain = scene.render(&[shaft()], 20.0, None, [0.0; 2]);
    let jittered = scene.render(&[shaft()], 20.0, None, [0.25, 0.0]);
    assert!((centroid(&jittered) - centroid(&plain) - 8.0).abs() < 0.15);
    let offset = Vec3::new(2048.0, 1024.0, 2048.0);
    scene.frame.view = Matrix4::look_at_lh(offset, offset + Vec3::Z, Vec3::Y).shader_constants();
    let translated = LightShaft {
        world: Matrix4::multiply(
            &shaft().world,
            &Matrix4::translation(offset.x, offset.y, offset.z),
        ),
        ..shaft()
    };
    let distant_origin = scene.render(&[translated], 20.0, None, [0.0; 2]);
    assert!(
        plain[..256 * 32]
            .iter()
            .zip(&distant_origin[..256 * 32])
            .all(|(a, b)| a.abs_diff(*b) <= 2)
    );
    scene.frame.view = Matrix4::IDENTITY.shader_constants();
    scene.depth =
        crate::DepthBuffer::with_format(&scene.gpu, 64, 32, wgpu::TextureFormat::Depth32Float);
    let replaced = scene.render(&[shaft()], 20.0, None, [0.0; 2]);
    assert_eq!(&plain[..256 * 32], &replaced[..256 * 32]);
    let invalid = LightShaft {
        world: Matrix4::scaling(0.0, 1.0, 1.0),
        ..shaft()
    };
    assert_eq!(center(&scene.render(&[invalid], 20.0, None, [0.0; 2])), 0);
    let behind = LightShaft {
        world: Matrix4::translation(0.0, 0.0, -5.0),
        ..shaft()
    };
    assert_eq!(center(&scene.render(&[behind], 20.0, None, [0.0; 2])), 0);
    assert_eq!(scene.renderer.draw_count(), 0);
    assert!(block_on(scope.pop()).is_none());
}
