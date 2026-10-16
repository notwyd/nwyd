use super::*;
use crate::target::{COLOR_FORMAT, DepthBuffer, Offscreen};
use crate::texture::AlphaPolicy;
use formats::mesh::msh::MshHeader;
use formats::texture::{MipLevel, PixelFormat, Texture};
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

fn foliage_card() -> SkinMesh {
    SkinMesh {
        header: MshHeader {
            magic: 0,
            attach_bone: 0,
            fvf: 0x1116,
            vertex_stride: 36,
            num_influences: 1,
            num_matrices: 1,
            num_vertices: 4,
            num_indices: 6,
        },
        bone_offsets: vec![Matrix4::IDENTITY.0],
        bone_indices: vec![0],
        vertices: [
            ([-1.0, -1.0, 0.25], [0.0, 1.0]),
            ([1.0, -1.0, 0.25], [1.0, 1.0]),
            ([-1.0, 1.0, 0.25], [0.0, 0.0]),
            ([1.0, 1.0, 0.25], [1.0, 0.0]),
        ]
        .into_iter()
        .map(|(position, uv)| SkinVertex {
            position,
            normal: [0.0, 0.0, -1.0],
            uv,
            bone_indices: [0; 4],
            bone_weights: [1.0, 0.0, 0.0, 0.0],
        })
        .collect(),
        indices: vec![0, 1, 2, 2, 1, 3],
    }
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn foliage_cutout_preserves_color_and_depth_below_its_alpha_threshold() {
    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    let target = Offscreen::new(&gpu, 4, 1);
    let depth = DepthBuffer::with_format(&gpu, 4, 1, wgpu::TextureFormat::Depth32Float);
    let mut renderer = SkinRenderer::new(
        &gpu,
        COLOR_FORMAT,
        wgpu::TextureFormat::Depth32Float,
        VendorClass::Nvidia,
    );
    let texture = Texture {
        width: 4,
        height: 1,
        format: PixelFormat::Rgba8Unorm,
        source_len: 16,
        data: [0, 169, 170, 255]
            .into_iter()
            .flat_map(|alpha| [32, 192, 48, alpha])
            .collect(),
        mips: vec![MipLevel {
            width: 4,
            height: 1,
            offset: 0,
            len: 16,
        }],
    };
    let uploaded = crate::texture::upload(&gpu, &texture, AlphaPolicy::Stored, "foliage cutout");
    let binding = renderer.bind_texture(&gpu, &uploaded.view);
    let mesh = foliage_card();
    let mut arena = SkinArena::default();
    let part = arena.push(&mesh);

    renderer.upload_arena(&gpu, &arena);

    let object = SkinObject::new(
        &SkinParams {
            diffuse: [1.0; 4],
            ambient: [0.0; 4],
            emissive: [1.0, 1.0, 1.0, 0.0],
            unlit: false,
            fog_start: 100.0,
            fog_end: 200.0,
            fog_enabled: false,
            force_opaque: false,
            modulate_alpha: false,
            glow: None,
        },
        1,
        170.0 / 255.0,
    );
    let palette = BonePalette::build(&mesh, &Matrix4::IDENTITY, |_| Matrix4::IDENTITY);

    renderer.prepare(
        &gpu,
        &SkinFrame::new(&Matrix4::IDENTITY, &Matrix4::IDENTITY, [0.0; 4]),
        &[object],
        &[palette],
        &[SkinDraw {
            pass: SkinPass::Opaque,
            part,
            object: 0,
            palette: 0,
            texture: Some(0),
        }],
        1,
    );

    assert_eq!(renderer.draw_count(), 1);
    assert!(!renderer.drops().any());

    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("foliage color and depth readback"),
        size: 512,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("foliage cutout regression"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &target.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 17.0 / 255.0,
                        g: 34.0 / 255.0,
                        b: 51.0 / 255.0,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.75),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        renderer.draw(&mut pass, &[binding]);
    }

    for (texture, aspect, offset) in [
        (&target.texture, wgpu::TextureAspect::All, 0),
        (&depth.texture, wgpu::TextureAspect::DepthOnly, 256),
    ] {
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset,
                    bytes_per_row: Some(256),
                    rows_per_image: Some(1),
                },
            },
            wgpu::Extent3d {
                width: 4,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    let submission = gpu.queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
    let (sender, receiver) = std::sync::mpsc::sync_channel(1);

    slice.map_async(wgpu::MapMode::Read, move |result| {
        sender.send(result).expect("send readback result");
    });

    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(Duration::from_secs(30)),
        })
        .expect("wait for foliage draw");

    receiver
        .recv_timeout(Duration::from_secs(30))
        .expect("receive readback result")
        .expect("map foliage readback");

    let mapped = slice.get_mapped_range().expect("access foliage readback");

    for (column, expected) in [
        [17, 34, 51, 255],
        [17, 34, 51, 255],
        [32, 192, 48, 170],
        [32, 192, 48, 255],
    ]
    .into_iter()
    .enumerate()
    {
        assert_eq!(
            &mapped[column * 4..column * 4 + 4],
            &expected,
            "color at column {column}"
        );

        let at = 256 + column * 4;
        let actual_depth = f32::from_le_bytes(mapped[at..at + 4].try_into().unwrap());
        let expected_depth = if column < 2 { 0.75 } else { 0.25 };

        assert_eq!(actual_depth, expected_depth, "depth at column {column}");
    }

    drop(mapped);
    readback.unmap();
}
