use crate::Gpu;

#[cfg(test)]
mod tests;

pub fn supported(gpu: &Gpu, format: wgpu::TextureFormat) -> bool {
    let features = gpu.device.features();

    if !features.contains(format.required_features())
        || format.sample_type(None, Some(features))
            != Some(wgpu::TextureSampleType::Float { filterable: true })
    {
        return false;
    }

    let mut supported = gpu.adapter.get_texture_format_features(format);

    if !features.contains(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES)
        && gpu
            .adapter
            .get_downlevel_capabilities()
            .flags
            .contains(wgpu::DownlevelFlags::WEBGPU_TEXTURE_FORMAT_SUPPORT)
    {
        let guaranteed = format.guaranteed_format_features(features);
        supported.allowed_usages &= guaranteed.allowed_usages;
        supported.flags &= guaranteed.flags;
    }

    supported
        .allowed_usages
        .contains(wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING)
        && supported
            .flags
            .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
}

pub struct AntialiasSource {
    binding: wgpu::BindGroup,
}

pub struct SceneAntialias {
    pipeline: wgpu::RenderPipeline,
    layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    empty_mask: wgpu::TextureView,
}

impl SceneAntialias {
    pub fn new(gpu: &Gpu, target_format: wgpu::TextureFormat) -> Self {
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: crate::gpu_label("scene antialias"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/antialias.wgsl").into()),
            });
        let layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: crate::gpu_label("scene antialias source"),
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
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let pipeline_layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: crate::gpu_label("scene antialias"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: crate::gpu_label("scene antialias"),
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
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: crate::gpu_label("scene antialias"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let empty_mask = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: crate::gpu_label("scene antialias empty preservation mask"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::R8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&Default::default());

        Self {
            pipeline,
            layout,
            sampler,
            empty_mask,
        }
    }

    pub fn source(&self, gpu: &Gpu, view: &wgpu::TextureView) -> AntialiasSource {
        self.source_with_preservation(gpu, view, &self.empty_mask)
    }

    pub fn source_with_preservation(
        &self,
        gpu: &Gpu,
        view: &wgpu::TextureView,
        mask: &wgpu::TextureView,
    ) -> AntialiasSource {
        AntialiasSource {
            binding: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: crate::gpu_label("scene antialias source"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(mask),
                    },
                ],
            }),
        }
    }

    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        source: &AntialiasSource,
        target: &wgpu::TextureView,
    ) {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: crate::gpu_label("scene antialias"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });

        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &source.binding, &[]);
        pass.draw(0..3, 0..1);
    }

    pub fn render(&self, gpu: &Gpu, source: &AntialiasSource, target: &wgpu::TextureView) {
        let mut encoder = gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: crate::gpu_label("scene antialias"),
            });

        self.encode(&mut encoder, source, target);

        gpu.queue.submit([encoder.finish()]);
    }
}
