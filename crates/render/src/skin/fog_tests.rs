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

fn render_unlit(
    gpu: &Gpu,
    z: f32,
    limit: f32,
    enabled: bool,
    textured: bool,
    temporal: bool,
) -> [u8; 4] {
    let target = Offscreen::new(gpu, 1, 1);
    let depth = DepthBuffer::with_format(gpu, 1, 1, wgpu::TextureFormat::Depth32Float);
    let camera = temporal.then(|| crate::temporal::TemporalCamera::new(gpu));
    let outputs = temporal.then(|| {
        (
            Offscreen::with_format(gpu, 1, 1, crate::temporal::MOTION_FORMAT),
            Offscreen::with_format(gpu, 1, 1, crate::temporal::BIAS_FORMAT),
        )
    });
    let mut renderer = SkinRenderer::with_temporal(
        gpu,
        COLOR_FORMAT,
        wgpu::TextureFormat::Depth32Float,
        VendorClass::Nvidia,
        camera.as_ref(),
    );
    let texture = Texture {
        width: 1,
        height: 1,
        format: PixelFormat::Rgba8Unorm,
        source_len: 4,
        data: vec![255; 4],
        mips: vec![MipLevel {
            width: 1,
            height: 1,
            offset: 0,
            len: 4,
        }],
    };
    let texture = crate::texture::upload(gpu, &texture, AlphaPolicy::Opaque, "unlit fog texture");
    let binding = renderer.bind_texture(gpu, &texture.view);
    let mesh = SkinMesh {
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
            [-1.0, -1.0, z],
            [1.0, -1.0, z],
            [-1.0, 1.0, z],
            [1.0, 1.0, z],
        ]
        .into_iter()
        .map(|position| SkinVertex {
            position,
            normal: [0.0, 0.0, -1.0],
            uv: [0.0; 2],
            bone_indices: [0; 4],
            bone_weights: [1.0, 0.0, 0.0, 0.0],
        })
        .collect(),
        indices: vec![0, 1, 2, 2, 1, 3],
    };
    let mut arena = SkinArena::default();
    let part = arena.push(&mesh);
    renderer.upload_arena(gpu, &arena);
    let object = SkinObject::new(
        &SkinParams {
            diffuse: [1.0; 4],
            ambient: [0.0; 4],
            emissive: [0.0; 4],
            unlit: true,
            fog_start: 24.0,
            fog_end: 63.0,
            fog_enabled: enabled,
            force_opaque: true,
            modulate_alpha: false,
            glow: None,
        },
        1,
        0.0,
    );
    assert_eq!(object.fog_params[1], 63.0 * 1.4);
    let palette = BonePalette::build(&mesh, &Matrix4::IDENTITY, |_| Matrix4::IDENTITY);
    let frame = SkinFrame::new(
        &Matrix4::scaling(1.0, 1.0, 0.01),
        &Matrix4::IDENTITY,
        [0.0; 4],
    )
    .with_fog_limit((limit > 0.0).then_some(limit));
    renderer.prepare(
        gpu,
        &frame,
        &[object],
        &[palette],
        &[SkinDraw {
            pass: SkinPass::Opaque,
            part,
            object: 0,
            palette: 0,
            texture: textured.then_some(0),
        }],
        1,
    );

    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("unlit fog readback"),
        size: 256,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    let mut attachments = vec![Some(wgpu::RenderPassColorAttachment {
        view: &target.view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::RED),
            store: wgpu::StoreOp::Store,
        },
    })];

    if let Some((motion, bias)) = &outputs {
        for output in [motion, bias] {
            attachments.push(Some(wgpu::RenderPassColorAttachment {
                view: &output.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            }));
        }
    }

    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("unlit fog regression"),
            color_attachments: &attachments,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        if let Some(camera) = &camera {
            pass.set_bind_group(4, &camera.binding, &[]);
        }
        renderer.draw(&mut pass, &[binding]);
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
                bytes_per_row: Some(256),
                rows_per_image: Some(1),
            },
        },
        wgpu::Extent3d {
            width: 1,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    let submission = gpu.queue.submit([encoder.finish()]);
    let slice = readback.slice(..);
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
    let mapped = slice.get_mapped_range().unwrap();
    let color = mapped[..4].try_into().unwrap();
    drop(mapped);
    readback.unmap();
    color
}

#[test]
#[ignore = "requires a compatible GPU adapter"]
fn unlit_models_fade_before_the_outdoor_limit_without_changing_uncapped_or_disabled_fog() {
    let gpu = block_on(Gpu::open(Gpu::instance(), None)).expect("open a GPU adapter");
    assert!(crate::temporal::supported(&gpu, COLOR_FORMAT, (1, 1)));
    for temporal in [false, true] {
        for textured in [false, true] {
            let uncapped = render_unlit(&gpu, 63.0, 0.0, true, textured, temporal);
            assert!((175..=178).contains(&uncapped[0]));
            assert_eq!(
                render_unlit(&gpu, 63.0, 63.0, true, textured, temporal),
                [0, 0, 0, 255]
            );
            assert_eq!(
                render_unlit(&gpu, 70.0, 63.0, true, textured, temporal),
                [0, 0, 0, 255]
            );
            assert_eq!(
                render_unlit(&gpu, 24.0, 63.0, true, textured, temporal),
                [255; 4]
            );
            assert_eq!(
                render_unlit(&gpu, 70.0, 63.0, false, textured, temporal),
                [255; 4]
            );
        }
    }
}

#[test]
fn fog_limit_does_not_change_the_uniform_layout_or_texture_scroll() {
    let original =
        SkinFrame::new(&Matrix4::IDENTITY, &Matrix4::IDENTITY, [0.0; 4]).with_scroll(1000);
    let limited = original.with_fog_limit(Some(63.0));
    assert_eq!(std::mem::size_of::<SkinFrame>(), 176);
    assert_eq!(limited.scroll, [original.scroll[0], 63.0, 0.0, 0.0]);
    assert_eq!(limited.with_fog_limit(None), original);
    for invalid in [f32::NAN, f32::INFINITY, -1.0, 0.0] {
        assert_eq!(original.with_fog_limit(Some(invalid)), original);
    }
}
