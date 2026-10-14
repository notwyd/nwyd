use crate::device::Gpu;
use crate::target::Offscreen;

pub const LETTERBOX: wgpu::Color = wgpu::Color::BLACK;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Fit {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Fit {
    pub fn is_one_to_one(&self, source: (u32, u32)) -> bool {
        self.width == source.0 && self.height == source.1
    }
}

pub fn fit(source: (u32, u32), surface: (u32, u32)) -> Fit {
    if surface.0 == 0 || surface.1 == 0 {
        return Fit {
            x: 0,
            y: 0,
            width: 0,
            height: 0,
        };
    }

    let (sw, sh) = (u64::from(source.0.max(1)), u64::from(source.1.max(1)));

    let (dw, dh) = (u64::from(surface.0), u64::from(surface.1));

    let (width, height) = if sw * dh >= dw * sh {
        (dw, (dw * sh / sw).max(1))
    } else {
        ((dh * sw / sh).max(1), dh)
    };

    Fit {
        x: ((dw - width.min(dw)) / 2) as u32,
        y: ((dh - height.min(dh)) / 2) as u32,
        width: width.min(dw) as u32,
        height: height.min(dh) as u32,
    }
}

pub struct BlitSource {
    nearest: wgpu::BindGroup,
    linear: wgpu::BindGroup,
    width: u32,
    height: u32,
}

pub struct Blitter {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    nearest: wgpu::Sampler,
    linear: wgpu::Sampler,
}

impl Blitter {
    pub fn new(gpu: &Gpu, target_format: wgpu::TextureFormat) -> Blitter {
        let shader = crate::shaders::module(&gpu.device, crate::shaders::Shader::Blit);

        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: crate::gpu_label("blit source"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
            });

        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: crate::gpu_label("blit"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });

        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: crate::gpu_label("blit"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState {
                    cull_mode: None,
                    ..Default::default()
                },
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: target_format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            });

        Blitter {
            pipeline,
            layout,
            nearest: gpu
                .device
                .create_sampler(&sampler_descriptor("blit nearest", false)),
            linear: gpu
                .device
                .create_sampler(&sampler_descriptor("blit linear", true)),
        }
    }

    pub fn source(&self, gpu: &Gpu, frame: &Offscreen) -> BlitSource {
        let bind = |sampler: &wgpu::Sampler, label: &str| {
            gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: crate::gpu_label(label),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&frame.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            })
        };

        BlitSource {
            nearest: bind(&self.nearest, "blit source (nearest)"),
            linear: bind(&self.linear, "blit source (linear)"),
            width: frame.width,
            height: frame.height,
        }
    }

    pub fn render(
        &self,
        gpu: &Gpu,
        source: &BlitSource,
        target: &wgpu::TextureView,
        target_size: (u32, u32),
    ) -> Fit {
        let fit = fit((source.width, source.height), target_size);

        let bind_group = if fit.is_one_to_one((source.width, source.height)) {
            &source.nearest
        } else {
            &source.linear
        };

        let mut enc = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: crate::gpu_label("blit"),
            });

        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: crate::gpu_label("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: target,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(LETTERBOX),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });

            pass.set_pipeline(&self.pipeline);

            pass.set_bind_group(0, bind_group, &[]);

            pass.set_viewport(
                fit.x as f32,
                fit.y as f32,
                fit.width as f32,
                fit.height as f32,
                0.0,
                1.0,
            );

            pass.draw(0..3, 0..1);
        }

        gpu.queue.submit([enc.finish()]);

        fit
    }
}

fn sampler_descriptor(label: &'static str, filter: bool) -> wgpu::SamplerDescriptor<'static> {
    let mode = if filter {
        wgpu::FilterMode::Linear
    } else {
        wgpu::FilterMode::Nearest
    };

    wgpu::SamplerDescriptor {
        label: crate::gpu_label(label),
        address_mode_u: wgpu::AddressMode::ClampToEdge,
        address_mode_v: wgpu::AddressMode::ClampToEdge,
        address_mode_w: wgpu::AddressMode::ClampToEdge,
        mag_filter: mode,
        min_filter: mode,
        mipmap_filter: wgpu::MipmapFilterMode::Nearest,
        ..Default::default()
    }
}

pub const BLIT_WGSL: &str = include_str!("shaders/blit.wgsl");
