use super::*;
use crate::{DepthBuffer, Offscreen};
use passes::tests::{block_on, read, write};

struct Inputs {
    color: Offscreen,
    depth: DepthBuffer,
    motion: wgpu::TextureView,
    bias: wgpu::TextureView,
    target: Offscreen,
    native: Vec<u8>,
    mask: Vec<u8>,
}

impl Inputs {
    fn new(gpu: &Gpu, size: [u32; 2]) -> Self {
        let [width, height] = size;
        let color = Offscreen::new(gpu, width, height);
        let depth = DepthBuffer::with_format(gpu, width, height, wgpu::TextureFormat::Depth32Float);
        let target = Offscreen::new(gpu, width, height);
        let resource = |format| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: None,
                    size: wgpu::Extent3d {
                        width,
                        height,
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_DST
                        | wgpu::TextureUsages::COPY_SRC,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        let motion = resource(wgpu::TextureFormat::Rg16Float);
        let bias = resource(wgpu::TextureFormat::R8Unorm);
        let mut native = Vec::new();
        let mut mask = Vec::new();
        for y in 0..height {
            for x in 0..width {
                let column = x % 128 < 24;
                let checker = ((x / 32 + y / 24) % 2) as u8;
                let glow = (255.0
                    * (-((x as f32 - width as f32 * 0.7).powi(2)
                        + (y as f32 - height as f32 * 0.4).powi(2))
                        / 5000.0)
                        .exp()) as u8;
                let pixel = if y > height / 2 {
                    [40 + checker * 60, 35 + checker * 45, 30 + checker * 30]
                } else if column {
                    [90, 80, 65]
                } else {
                    [
                        35_u8.saturating_add(glow),
                        45_u8.saturating_add(glow / 2),
                        55,
                    ]
                };
                native.extend_from_slice(&[
                    pixel[0],
                    pixel[1],
                    pixel[2],
                    ((x + y) % 192 + 32) as u8,
                ]);
                mask.push(if x.abs_diff(width / 2) < 3 || y.abs_diff(height / 2) < 3 {
                    255
                } else {
                    0
                });
            }
        }
        write(gpu, &color.texture, &native, 4);
        write(
            gpu,
            motion.texture(),
            &vec![0; width as usize * height as usize * 4],
            4,
        );
        write(gpu, bias.texture(), &mask, 1);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: None,
                color_attachments: &[],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &depth.view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(0.5),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
        }
        gpu.queue.submit([encoder.finish()]);
        Self {
            color,
            depth,
            motion,
            bias,
            target,
            native,
            mask,
        }
    }

    fn evaluate(&self, gpu: &Gpu, renderer: &mut NeuralRenderer, reset: bool) {
        renderer
            .evaluate(
                gpu,
                &self.color.view,
                &self.depth.view,
                &self.motion,
                &self.bias,
                &self.target.view,
                reset,
            )
            .unwrap();
        let passes = &renderer.state.as_ref().unwrap().1;
        assert_eq!(
            passes.intermediate.texture().format(),
            wgpu::TextureFormat::Rgba16Unorm
        );
        assert_eq!(
            passes.output.texture().format(),
            wgpu::TextureFormat::Rgba16Unorm
        );
        let intermediate = read(gpu, passes.intermediate.texture());
        let model_output = read(gpu, passes.output.texture());
        let refined = intermediate
            .chunks_exact(8)
            .zip(model_output.chunks_exact(8))
            .filter(|(first, second)| first[..6] != second[..6])
            .count();
        assert!(
            refined > self.mask.len() / 100,
            "The second model pass did not refine the first pass"
        );
        assert!(
            intermediate
                .chunks_exact(2)
                .any(|channel| { u16::from_le_bytes(channel.try_into().unwrap()) % 257 != 0 }),
            "The first pass lost its 16-bit precision"
        );
        let output = read(gpu, &self.target.texture);
        let mut changed = 0;
        let mut nonblack = 0;
        for ((native, output), &mask) in self
            .native
            .chunks_exact(4)
            .zip(output.chunks_exact(4))
            .zip(&self.mask)
        {
            assert_eq!(native[3], output[3]);
            if mask == 255 {
                assert_eq!(native, output);
            }
            if native[..3]
                .iter()
                .zip(&output[..3])
                .any(|(left, right)| left.abs_diff(*right) > 2)
            {
                changed += 1;
            }
            if output[..3].iter().any(|&channel| channel > 10) {
                nonblack += 1;
            }
        }
        assert!(
            changed > self.mask.len() / 100,
            "Neural output did not visibly change the synthetic scene"
        );
        assert!(
            nonblack > self.mask.len() / 2,
            "Neural output was mostly blank"
        );
        println!(
            "NR evaluated {}x{}: {changed} changed pixels; {refined} pixels refined by pass 2; native alpha and reactive cross preserved",
            self.target.width, self.target.height
        );
    }
}

#[test]
#[ignore = "requires Windows NVIDIA Vulkan and an explicitly supplied Neural Rendering runtime"]
fn direct_neural_rendering_evaluates_resizes_and_recreates_without_a_bridge() {
    let gpu = block_on(Gpu::open_with_backend(
        Gpu::instance(),
        None,
        Some(crate::device::RendererBackend::Vulkan),
    ))
    .unwrap();
    assert!(
        gpu.supports_neural_rendering(),
        "Neural runtime probe failed"
    );
    let mut renderer = NeuralRenderer::new(&gpu, [1024, 512]).unwrap();
    let inputs = Inputs::new(&gpu, [1024, 512]);
    for frame in 0..3 {
        inputs.evaluate(&gpu, &mut renderer, frame == 0);
    }
    renderer.resize(&gpu, [768, 512]).unwrap();
    let resized = Inputs::new(&gpu, [768, 512]);
    resized.evaluate(&gpu, &mut renderer, true);
    resized.evaluate(&gpu, &mut renderer, false);
    drop(renderer);
    let mut renderer = NeuralRenderer::new(&gpu, [1024, 512]).unwrap();
    inputs.evaluate(&gpu, &mut renderer, true);
}
