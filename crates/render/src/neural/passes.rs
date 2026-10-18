use crate::Gpu;

pub(super) const DEVICE_FEATURES: wgpu::Features = wgpu::Features::TEXTURE_FORMAT_16BIT_NORM
    .union(wgpu::Features::TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES);

pub(super) struct Passes {
    color: wgpu::TextureView,
    motion: wgpu::TextureView,
    pub depth: wgpu::TextureView,
    pub intermediate: wgpu::TextureView,
    pub output: wgpu::TextureView,
    depth_layout: wgpu::BindGroupLayout,
    compose_layout: wgpu::BindGroupLayout,
    depth_pipeline: wgpu::RenderPipeline,
    compose_pipeline: wgpu::RenderPipeline,
    bindings: Option<Bindings>,
    size: [u32; 2],
}

struct Bindings {
    inputs: [wgpu::Texture; 4],
    depth: wgpu::BindGroup,
    compose: wgpu::BindGroup,
}

impl Passes {
    pub fn new(gpu: &Gpu, size: [u32; 2]) -> Result<Self, String> {
        if !gpu.device.features().contains(DEVICE_FEATURES) {
            return Err("Neural rendering requires 16-bit normalized textures".into());
        }
        if size
            .iter()
            .any(|&value| value == 0 || value > gpu.device.limits().max_texture_dimension_2d)
        {
            return Err("Invalid neural rendering dimensions".into());
        }
        let depth_layout = layout(gpu, &[wgpu::TextureSampleType::Depth]);
        let compose_layout = layout(
            gpu,
            &[wgpu::TextureSampleType::Float { filterable: false }; 3],
        );
        let depth_pipeline = pipeline(
            gpu,
            &depth_layout,
            wgpu::TextureFormat::R32Float,
            include_str!("depth.wgsl"),
        );
        let compose_pipeline = pipeline(
            gpu,
            &compose_layout,
            wgpu::TextureFormat::Rgba8Unorm,
            include_str!("compose.wgsl"),
        );
        let make_texture = |label, format, extra| {
            gpu.device
                .create_texture(&wgpu::TextureDescriptor {
                    label: crate::gpu_label(label),
                    size: wgpu::Extent3d {
                        width: size[0],
                        height: size[1],
                        depth_or_array_layers: 1,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format,
                    usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                        | wgpu::TextureUsages::TEXTURE_BINDING
                        | wgpu::TextureUsages::COPY_SRC
                        | extra,
                    view_formats: &[],
                })
                .create_view(&Default::default())
        };
        Ok(Self {
            color: make_texture(
                "neural color",
                wgpu::TextureFormat::Rgba8Unorm,
                wgpu::TextureUsages::COPY_DST,
            ),
            motion: make_texture(
                "neural motion",
                wgpu::TextureFormat::Rg16Float,
                wgpu::TextureUsages::COPY_DST,
            ),
            depth: make_texture(
                "neural depth",
                wgpu::TextureFormat::R32Float,
                wgpu::TextureUsages::empty(),
            ),
            intermediate: make_texture(
                "neural intermediate",
                wgpu::TextureFormat::Rgba16Unorm,
                wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_DST,
            ),
            output: make_texture(
                "neural output",
                wgpu::TextureFormat::Rgba16Unorm,
                wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_DST,
            ),
            depth_layout,
            compose_layout,
            depth_pipeline,
            compose_pipeline,
            bindings: None,
            size,
        })
    }

    pub fn validate_inputs(
        &self,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        motion: &wgpu::TextureView,
        bias: &wgpu::TextureView,
        target: &wgpu::TextureView,
    ) -> Result<(), String> {
        if [color, depth, motion, bias]
            .iter()
            .any(|view| view.texture() == target.texture())
        {
            return Err("Neural rendering output must not alias an input".into());
        }
        for (view, usage) in [
            (
                color,
                wgpu::TextureUsages::COPY_SRC | wgpu::TextureUsages::TEXTURE_BINDING,
            ),
            (depth, wgpu::TextureUsages::TEXTURE_BINDING),
            (motion, wgpu::TextureUsages::COPY_SRC),
            (bias, wgpu::TextureUsages::TEXTURE_BINDING),
            (target, wgpu::TextureUsages::RENDER_ATTACHMENT),
        ] {
            if !view.texture().usage().contains(usage) {
                return Err("Neural rendering texture is missing required usages".into());
            }
        }
        for (view, format) in [
            (color, wgpu::TextureFormat::Rgba8Unorm),
            (depth, wgpu::TextureFormat::Depth32Float),
            (motion, wgpu::TextureFormat::Rg16Float),
            (bias, wgpu::TextureFormat::R8Unorm),
            (target, wgpu::TextureFormat::Rgba8Unorm),
        ] {
            let texture = view.texture();
            if texture.format() != format
                || texture.width() != self.size[0]
                || texture.height() != self.size[1]
                || texture.sample_count() != 1
                || texture.depth_or_array_layers() != 1
                || texture.mip_level_count() != 1
                || texture.dimension() != wgpu::TextureDimension::D2
            {
                return Err("Neural rendering inputs must use matching full-resolution single-sample textures".into());
            }
        }
        Ok(())
    }

    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        color: &wgpu::TextureView,
        depth: &wgpu::TextureView,
        motion: &wgpu::TextureView,
        bias: &wgpu::TextureView,
    ) -> wgpu::CommandBuffer {
        let inputs = [
            color.texture().clone(),
            depth.texture().clone(),
            motion.texture().clone(),
            bias.texture().clone(),
        ];
        if !self
            .bindings
            .as_ref()
            .is_some_and(|bindings| bindings.inputs == inputs)
        {
            let views = inputs
                .each_ref()
                .map(|texture| texture.create_view(&Default::default()));
            self.bindings = Some(Bindings {
                inputs,
                depth: bind(gpu, &self.depth_layout, &[&views[1]]),
                compose: bind(
                    gpu,
                    &self.compose_layout,
                    &[&views[0], &self.output, &views[3]],
                ),
            });
        }
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        for (source, target) in [(color, &self.color), (motion, &self.motion)] {
            encoder.copy_texture_to_texture(
                source.texture().as_image_copy(),
                target.texture().as_image_copy(),
                target.texture().size(),
            );
        }
        draw(
            &mut encoder,
            &self.depth,
            &self.depth_pipeline,
            &self.bindings.as_ref().unwrap().depth,
        );
        for view in [&self.intermediate, &self.output] {
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: crate::gpu_label("initialize neural output"),
                color_attachments: &[Some(attachment(view))],
                ..Default::default()
            });
        }
        encoder.transition_resources(
            std::iter::empty(),
            [
                (self.color(), wgpu::TextureUses::RESOURCE),
                (&self.depth, wgpu::TextureUses::RESOURCE),
                (self.motion(), wgpu::TextureUses::RESOURCE),
                (&self.intermediate, wgpu::TextureUses::STORAGE_READ_WRITE),
            ]
            .into_iter()
            .map(|(view, state)| wgpu::TextureTransition {
                texture: view.texture(),
                selector: None,
                state,
            }),
        );
        encoder.finish()
    }

    pub fn between_passes(&self, gpu: &Gpu) -> wgpu::CommandBuffer {
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        encoder.transition_resources(
            std::iter::empty(),
            [
                (&self.intermediate, wgpu::TextureUses::RESOURCE),
                (&self.output, wgpu::TextureUses::STORAGE_READ_WRITE),
            ]
            .into_iter()
            .map(|(view, state)| wgpu::TextureTransition {
                texture: view.texture(),
                selector: None,
                state,
            }),
        );
        encoder.finish()
    }

    pub fn compose(&self, gpu: &Gpu, target: &wgpu::TextureView) -> wgpu::CommandBuffer {
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        draw(
            &mut encoder,
            target,
            &self.compose_pipeline,
            &self.bindings.as_ref().unwrap().compose,
        );
        encoder.finish()
    }

    pub fn color(&self) -> &wgpu::TextureView {
        &self.color
    }

    pub fn motion(&self) -> &wgpu::TextureView {
        &self.motion
    }
}

fn attachment(view: &wgpu::TextureView) -> wgpu::RenderPassColorAttachment<'_> {
    wgpu::RenderPassColorAttachment {
        view,
        depth_slice: None,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
            store: wgpu::StoreOp::Store,
        },
    }
}

fn draw(
    encoder: &mut wgpu::CommandEncoder,
    target: &wgpu::TextureView,
    pipeline: &wgpu::RenderPipeline,
    binding: &wgpu::BindGroup,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: crate::gpu_label("neural rendering conversion"),
        color_attachments: &[Some(attachment(target))],
        ..Default::default()
    });
    pass.set_pipeline(pipeline);
    pass.set_bind_group(0, binding, &[]);
    pass.draw(0..3, 0..1);
}

fn layout(gpu: &Gpu, samples: &[wgpu::TextureSampleType]) -> wgpu::BindGroupLayout {
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: crate::gpu_label("neural textures"),
            entries: &samples
                .iter()
                .enumerate()
                .map(|(index, &sample_type)| wgpu::BindGroupLayoutEntry {
                    binding: index as u32,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                })
                .collect::<Vec<_>>(),
        })
}

fn bind(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    views: &[&wgpu::TextureView],
) -> wgpu::BindGroup {
    gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: crate::gpu_label("neural textures"),
        layout,
        entries: &views
            .iter()
            .enumerate()
            .map(|(index, &view)| wgpu::BindGroupEntry {
                binding: index as u32,
                resource: wgpu::BindingResource::TextureView(view),
            })
            .collect::<Vec<_>>(),
    })
}

fn pipeline(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    format: wgpu::TextureFormat,
    source: &str,
) -> wgpu::RenderPipeline {
    let shader = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: crate::gpu_label("neural conversion shader"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
    let layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: crate::gpu_label("neural conversion layout"),
            bind_group_layouts: &[Some(layout)],
            immediate_size: 0,
        });
    gpu.device
        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: crate::gpu_label("neural conversion pipeline"),
            layout: Some(&layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: None,
            multisample: Default::default(),
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
}

#[cfg(test)]
pub(super) mod tests;
