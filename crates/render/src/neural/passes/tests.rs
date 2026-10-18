use super::*;
use crate::{DepthBuffer, Offscreen};
use std::future::Future;
use std::sync::Arc;
use std::task::{Context, Poll, Wake, Waker};

struct ThreadWake(std::thread::Thread);

impl Wake for ThreadWake {
    fn wake(self: Arc<Self>) {
        self.0.unpark();
    }
}

pub(crate) fn block_on<T>(future: impl Future<Output = T>) -> T {
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

pub(crate) fn read(gpu: &Gpu, texture: &wgpu::Texture) -> Vec<u8> {
    let pixel_size = texture.format().block_copy_size(None).unwrap();
    let stride = (texture.width() * pixel_size).div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: u64::from(stride * texture.height()),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        texture.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(stride),
                rows_per_image: Some(texture.height()),
            },
        },
        texture.size(),
    );
    let submission = gpu.queue.submit([encoder.finish()]);
    let (sender, receiver) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |result| {
            sender.send(result).unwrap()
        });
    gpu.device
        .poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(30)),
        })
        .unwrap();
    receiver.recv().unwrap().unwrap();
    let data = buffer.slice(..).get_mapped_range().unwrap();
    data.chunks(stride as usize)
        .flat_map(|row| &row[..(texture.width() * pixel_size) as usize])
        .copied()
        .collect()
}

pub(crate) fn write(gpu: &Gpu, texture: &wgpu::Texture, bytes: &[u8], bytes_per_pixel: u32) {
    gpu.queue.write_texture(
        texture.as_image_copy(),
        bytes,
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(texture.width() * bytes_per_pixel),
            rows_per_image: Some(texture.height()),
        },
        texture.size(),
    );
}

fn input(gpu: &Gpu, width: u32, format: wgpu::TextureFormat) -> wgpu::TextureView {
    gpu.device
        .create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::COPY_DST
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
        .create_view(&Default::default())
}

fn gpu_with_normalized_16_bit_textures() -> Gpu {
    let mut gpu = block_on(Gpu::open(Gpu::instance(), None)).unwrap();
    let (device, queue) = block_on(gpu.adapter.request_device(&wgpu::DeviceDescriptor {
        required_features: gpu.device.features() | DEVICE_FEATURES,
        required_limits: gpu.device.limits(),
        ..Default::default()
    }))
    .unwrap();
    gpu.device = Arc::new(device);
    gpu.queue = Arc::new(queue);
    gpu
}

fn write_model_output(gpu: &Gpu, texture: &wgpu::Texture, bytes: &[u8]) {
    let data = bytes
        .iter()
        .flat_map(|&byte| (u16::from(byte) * 257).to_le_bytes())
        .collect::<Vec<_>>();
    write(gpu, texture, &data, 8);
}

#[test]
fn neural_conversion_shaders_validate() {
    for source in [
        include_str!("../depth.wgsl"),
        include_str!("../compose.wgsl"),
    ] {
        let module = wgpu::naga::front::wgsl::parse_str(source).unwrap();
        wgpu::naga::valid::Validator::new(
            wgpu::naga::valid::ValidationFlags::all(),
            wgpu::naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
    }
}

#[test]
#[ignore = "requires a Vulkan graphics device"]
fn depth_and_reactive_composition_preserve_engine_guides_and_alpha() {
    let gpu = gpu_with_normalized_16_bit_textures();
    let mut passes = Passes::new(&gpu, [3, 1]).unwrap();
    assert_eq!(
        passes.output.texture().format(),
        wgpu::TextureFormat::Rgba16Unorm
    );
    let color = input(&gpu, 3, wgpu::TextureFormat::Rgba8Unorm);
    let motion = input(&gpu, 3, wgpu::TextureFormat::Rg16Float);
    let bias = input(&gpu, 3, wgpu::TextureFormat::R8Unorm);
    let depth = DepthBuffer::with_format(&gpu, 3, 1, wgpu::TextureFormat::Depth32Float);
    let target = Offscreen::new(&gpu, 3, 1);
    let native = [220, 40, 20, 37, 220, 40, 20, 127, 220, 40, 20, 231];
    write(&gpu, color.texture(), &native, 4);
    write(&gpu, bias.texture(), &[0, 128, 255], 1);
    write(&gpu, motion.texture(), &[0; 12], 4);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth.view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(0.375),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            }),
            ..Default::default()
        });
    }
    gpu.queue.submit([encoder.finish()]);
    passes
        .validate_inputs(&color, &depth.view, &motion, &bias, &target.view)
        .unwrap();
    let wrong_size = Offscreen::new(&gpu, 4, 1);
    assert!(
        passes
            .validate_inputs(&color, &depth.view, &motion, &bias, &wrong_size.view)
            .is_err()
    );
    gpu.queue
        .submit([passes.prepare(&gpu, &color, &depth.view, &motion, &bias)]);
    for depth in read(&gpu, passes.depth.texture()).chunks_exact(4) {
        assert_eq!(f32::from_le_bytes(depth.try_into().unwrap()), 0.375);
    }
    write_model_output(
        &gpu,
        passes.output.texture(),
        &[20, 140, 240, 255, 20, 140, 240, 255, 20, 140, 240, 255],
    );
    gpu.queue.submit([passes.compose(&gpu, &target.view)]);
    let result = read(&gpu, &target.texture);
    assert_eq!(&result[..4], &[20, 140, 240, 37]);
    for (actual, expected) in result[4..8].iter().zip([120, 90, 130, 127]) {
        assert!(actual.abs_diff(expected) <= 1);
    }
    assert_eq!(&result[8..], &native[8..]);
    gpu.queue
        .submit([passes.prepare(&gpu, &color, &depth.view, &motion, &bias)]);
    write_model_output(
        &gpu,
        passes.output.texture(),
        &[0, 255, 0, 0, 0, 255, 0, 0, 0, 255, 0, 0],
    );
    gpu.queue.submit([passes.compose(&gpu, &target.view)]);
    let result = read(&gpu, &target.texture);
    assert_eq!(&result[..4], &[0, 255, 0, 37]);
    assert_eq!(&result[8..], &native[8..]);
}

#[test]
#[ignore = "requires a Vulkan graphics device with 16-bit normalized texture support"]
fn two_pass_chain_preserves_precision_until_final_composition() {
    let gpu = gpu_with_normalized_16_bit_textures();
    let mut passes = Passes::new(&gpu, [1, 1]).unwrap();
    for texture in [passes.intermediate.texture(), passes.output.texture()] {
        assert_eq!(texture.format(), wgpu::TextureFormat::Rgba16Unorm);
    }
    assert_ne!(passes.intermediate.texture(), passes.output.texture());
    let color = input(&gpu, 1, wgpu::TextureFormat::Rgba8Unorm);
    let motion = input(&gpu, 1, wgpu::TextureFormat::Rg16Float);
    let bias = input(&gpu, 1, wgpu::TextureFormat::R8Unorm);
    let depth = DepthBuffer::with_format(&gpu, 1, 1, wgpu::TextureFormat::Depth32Float);
    let target = Offscreen::new(&gpu, 1, 1);
    write(&gpu, color.texture(), &[250, 60, 10, 93], 4);
    write(&gpu, motion.texture(), &[0; 4], 4);
    write(&gpu, bias.texture(), &[0], 1);
    let vertex = r#"
@vertex fn vs(@builtin(vertex_index) index: u32) -> @builtin(position) vec4<f32> {
    let positions = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    return vec4(positions[index], 0.0, 1.0);
}
"#;
    let first_layout = layout(&gpu, &[]);
    let first_binding = bind(&gpu, &first_layout, &[]);
    let first_pipeline = pipeline(
        &gpu,
        &first_layout,
        wgpu::TextureFormat::Rgba16Unorm,
        &format!(
            "{vertex}\n@fragment fn fs() -> @location(0) vec4<f32> {{ return vec4(128.0 / 65535.0, 0.0, 0.0, 1.0); }}"
        ),
    );
    let second_layout = layout(
        &gpu,
        &[wgpu::TextureSampleType::Float { filterable: false }],
    );
    let second_binding = bind(&gpu, &second_layout, &[&passes.intermediate]);
    let second_pipeline = pipeline(
        &gpu,
        &second_layout,
        wgpu::TextureFormat::Rgba16Unorm,
        &format!(
            "{vertex}\n@group(0) @binding(0) var source: texture_2d<f32>;\n@fragment fn fs(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {{ return textureLoad(source, vec2<i32>(position.xy), 0) * vec4(128.0, 1.0, 1.0, 1.0); }}"
        ),
    );
    let prepare = passes.prepare(&gpu, &color, &depth.view, &motion, &bias);
    let mut first_pass = gpu.device.create_command_encoder(&Default::default());
    draw(
        &mut first_pass,
        &passes.intermediate,
        &first_pipeline,
        &first_binding,
    );
    let barrier = passes.between_passes(&gpu);
    let mut second_pass = gpu.device.create_command_encoder(&Default::default());
    draw(
        &mut second_pass,
        &passes.output,
        &second_pipeline,
        &second_binding,
    );
    let compose = passes.compose(&gpu, &target.view);
    gpu.queue.submit([
        prepare,
        first_pass.finish(),
        barrier,
        second_pass.finish(),
        compose,
    ]);
    let intermediate = read(&gpu, passes.intermediate.texture());
    let output = read(&gpu, passes.output.texture());
    assert_eq!(
        u16::from_le_bytes(intermediate[..2].try_into().unwrap()),
        128
    );
    assert_eq!(u16::from_le_bytes(output[..2].try_into().unwrap()), 16384);
    let composed = read(&gpu, &target.texture);
    assert!(composed[0].abs_diff(64) <= 1);
    assert_eq!(&composed[1..], &[0, 0, 93]);
}
