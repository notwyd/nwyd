use crate::{Gpu, Matrix4, world::FrameUniforms};
use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3, Vec4};

#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LightShaft {
    pub world: Matrix4,
    pub color: [f32; 3],
    pub density: f32,
    pub source_width: [f32; 2],
    pub texture: usize,
    pub uv_scale_offset: [f32; 4],
}

pub struct LightShaftTexture {
    binding: wgpu::BindGroup,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Frame {
    inverse_view_projection: [f32; 16],
    viewport_jitter: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Volume {
    inverse_world: [f32; 16],
    color_density: [f32; 4],
    source_width: [f32; 4],
    uv_scale_offset: [f32; 4],
}

struct Draw {
    texture: usize,
    object: u32,
    scissor: [u32; 4],
}

pub struct LightShaftRenderer {
    pipelines: [wgpu::RenderPipeline; 2],
    frame_layout: wgpu::BindGroupLayout,
    volume_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    frame_buffer: wgpu::Buffer,
    frame_binding: Option<(wgpu::Texture, wgpu::BindGroup)>,
    volume_buffer: wgpu::Buffer,
    volume_binding: wgpu::BindGroup,
    volume_stride: u32,
    volume_capacity: usize,
    scratch: Vec<u8>,
    draws: Vec<Draw>,
}

impl LightShaftRenderer {
    pub fn new(gpu: &Gpu, target_format: wgpu::TextureFormat) -> Self {
        let uniform_entry = |binding, size, dynamic| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: dynamic,
                min_binding_size: std::num::NonZeroU64::new(size),
            },
            count: None,
        };
        let frame_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: crate::gpu_label("light shaft frame"),
                entries: &[
                    uniform_entry(0, std::mem::size_of::<Frame>() as u64, false),
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Depth,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                ],
            });
        let volume_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: crate::gpu_label("light shaft volume"),
                entries: &[uniform_entry(0, std::mem::size_of::<Volume>() as u64, true)],
            });
        let texture_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: crate::gpu_label("light shaft texture"),
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
        let layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: crate::gpu_label("light shaft"),
                bind_group_layouts: &[
                    Some(&frame_layout),
                    Some(&volume_layout),
                    Some(&texture_layout),
                ],
                immediate_size: 0,
            });
        let shader = gpu
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: crate::gpu_label("light shaft"),
                source: wgpu::ShaderSource::Wgsl(include_str!("shaders/light_shafts.wgsl").into()),
            });
        let pipelines = [false, true].map(|reactive| {
            let mut targets = vec![Some(wgpu::ColorTargetState {
                format: target_format,
                blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                write_mask: wgpu::ColorWrites::ALL,
            })];
            if reactive {
                let maximum = wgpu::BlendComponent {
                    src_factor: wgpu::BlendFactor::One,
                    dst_factor: wgpu::BlendFactor::One,
                    operation: wgpu::BlendOperation::Max,
                };
                targets.push(Some(wgpu::ColorTargetState {
                    format: crate::temporal::BIAS_FORMAT,
                    blend: Some(wgpu::BlendState {
                        color: maximum,
                        alpha: maximum,
                    }),
                    write_mask: wgpu::ColorWrites::RED,
                }));
            }
            gpu.device
                .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: crate::gpu_label("light shaft"),
                    layout: Some(&layout),
                    vertex: wgpu::VertexState {
                        module: &shader,
                        entry_point: Some("vs"),
                        compilation_options: Default::default(),
                        buffers: &[],
                    },
                    primitive: wgpu::PrimitiveState::default(),
                    depth_stencil: None,
                    multisample: wgpu::MultisampleState::default(),
                    fragment: Some(wgpu::FragmentState {
                        module: &shader,
                        entry_point: Some(if reactive { "fs_reactive" } else { "fs" }),
                        compilation_options: Default::default(),
                        targets: &targets,
                    }),
                    multiview_mask: None,
                    cache: None,
                })
        });
        let frame_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("light shaft frame"),
            size: std::mem::size_of::<Frame>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let alignment = gpu.device.limits().min_uniform_buffer_offset_alignment;
        let volume_stride = (std::mem::size_of::<Volume>() as u32).div_ceil(alignment) * alignment;
        let (volume_buffer, volume_binding) = volume_storage(gpu, &volume_layout, volume_stride, 1);
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: crate::gpu_label("light shaft mask"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        Self {
            pipelines,
            frame_layout,
            volume_layout,
            texture_layout,
            sampler,
            frame_buffer,
            frame_binding: None,
            volume_buffer,
            volume_binding,
            volume_stride,
            volume_capacity: 1,
            scratch: Vec::new(),
            draws: Vec::new(),
        }
    }

    pub fn bind_texture(&self, gpu: &Gpu, view: &wgpu::TextureView) -> LightShaftTexture {
        LightShaftTexture {
            binding: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: crate::gpu_label("light shaft mask"),
                layout: &self.texture_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                ],
            }),
        }
    }

    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        frame: &FrameUniforms,
        shafts: &[LightShaft],
        depth: &wgpu::TextureView,
        jitter: [f32; 2],
    ) {
        self.draws.clear();
        let view_projection =
            Mat4::from_cols_array(&frame.view) * Mat4::from_cols_array(&frame.projection);
        let inverse = view_projection.inverse();
        if shafts.is_empty() || !inverse.is_finite() || !jitter.into_iter().all(f32::is_finite) {
            return;
        }
        let size = [depth.texture().width(), depth.texture().height()];
        let uniform = Frame {
            inverse_view_projection: inverse.to_cols_array(),
            viewport_jitter: [size[0] as f32, size[1] as f32, jitter[0], jitter[1]],
        };
        gpu.queue
            .write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(&uniform));
        if !self
            .frame_binding
            .as_ref()
            .is_some_and(|(texture, _)| texture == depth.texture())
        {
            self.frame_binding = Some((
                depth.texture().clone(),
                gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: crate::gpu_label("light shaft frame"),
                    layout: &self.frame_layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: self.frame_buffer.as_entire_binding(),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(depth),
                        },
                    ],
                }),
            ));
        }
        if shafts.len() > self.volume_capacity {
            self.volume_capacity = shafts.len().next_power_of_two();
            (self.volume_buffer, self.volume_binding) = volume_storage(
                gpu,
                &self.volume_layout,
                self.volume_stride,
                self.volume_capacity,
            );
        }
        self.scratch.clear();
        for shaft in shafts {
            let world = Mat4::from_cols_array(&shaft.world.shader_constants());
            let inverse_world = world.inverse();
            if !inverse_world.is_finite()
                || !shaft.density.is_finite()
                || shaft.density <= 0.0
                || !shaft.color.into_iter().all(|v| v.is_finite() && v >= 0.0)
                || !shaft.uv_scale_offset.into_iter().all(f32::is_finite)
                || !shaft
                    .source_width
                    .into_iter()
                    .all(|v| v.is_finite() && v > 0.0 && v <= 1.0)
            {
                continue;
            }
            let Some(scissor) = projected_bounds(world * view_projection, size, jitter) else {
                continue;
            };
            let volume = Volume {
                inverse_world: inverse_world.to_cols_array(),
                color_density: [
                    shaft.color[0],
                    shaft.color[1],
                    shaft.color[2],
                    shaft.density,
                ],
                uv_scale_offset: shaft.uv_scale_offset,
                source_width: [shaft.source_width[0], shaft.source_width[1], 0.0, 0.0],
            };
            let object = self.draws.len() as u32;
            let offset = self.scratch.len();
            self.scratch.resize(offset + self.volume_stride as usize, 0);
            self.scratch[offset..offset + std::mem::size_of::<Volume>()]
                .copy_from_slice(bytemuck::bytes_of(&volume));
            self.draws.push(Draw {
                texture: shaft.texture,
                object,
                scissor,
            });
        }
        if !self.scratch.is_empty() {
            gpu.queue
                .write_buffer(&self.volume_buffer, 0, &self.scratch);
        }
    }

    pub fn draw_count(&self) -> usize {
        self.draws.len()
    }

    pub fn encode(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        target: &wgpu::TextureView,
        textures: &[Option<LightShaftTexture>],
        reactive: Option<&wgpu::TextureView>,
    ) {
        if self.draws.is_empty() {
            return;
        }
        let Some((_, frame_binding)) = &self.frame_binding else {
            return;
        };
        let attachment = |view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })
        };
        let mut attachments = vec![attachment(target)];
        if let Some(view) = reactive {
            attachments.push(attachment(view));
        }
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: crate::gpu_label("volumetric light shafts"),
            color_attachments: &attachments,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&self.pipelines[usize::from(reactive.is_some())]);
        pass.set_bind_group(0, frame_binding, &[]);
        for draw in &self.draws {
            let Some(Some(texture)) = textures.get(draw.texture) else {
                continue;
            };
            pass.set_bind_group(1, &self.volume_binding, &[draw.object * self.volume_stride]);
            pass.set_bind_group(2, &texture.binding, &[]);
            let [x, y, width, height] = draw.scissor;
            pass.set_scissor_rect(x, y, width, height);
            pass.draw(0..3, 0..1);
        }
    }
}

fn volume_storage(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    stride: u32,
    capacity: usize,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: crate::gpu_label("light shaft volumes"),
        size: u64::from(stride) * capacity as u64,
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: crate::gpu_label("light shaft volumes"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: std::num::NonZeroU64::new(std::mem::size_of::<Volume>() as u64),
            }),
        }],
    });
    (buffer, binding)
}

fn projected_bounds(transform: Mat4, size: [u32; 2], jitter: [f32; 2]) -> Option<[u32; 4]> {
    let transform = transform.transpose();
    let mut minimum = Vec3::splat(f32::INFINITY);
    let mut maximum = Vec3::splat(f32::NEG_INFINITY);
    let mut near = false;
    let mut outside = 0b11_1111;
    for x in [-1.0, 1.0] {
        for y in [-1.0, 1.0] {
            for z in [-1.0, 1.0] {
                let clip = transform * Vec4::new(x, y, z, 1.0);
                if !clip.is_finite() {
                    return None;
                }
                let x = clip.x + jitter[0] * clip.w;
                let y = clip.y + jitter[1] * clip.w;
                outside &= u8::from(x < -clip.w)
                    | (u8::from(x > clip.w) << 1)
                    | (u8::from(y < -clip.w) << 2)
                    | (u8::from(y > clip.w) << 3)
                    | (u8::from(clip.z < 0.0) << 4)
                    | (u8::from(clip.z > clip.w) << 5);
                if clip.w <= 0.0 || clip.z <= 0.0 {
                    near = true;
                    continue;
                }
                let ndc = clip.truncate() / clip.w;
                minimum = minimum.min(ndc);
                maximum = maximum.max(ndc);
            }
        }
    }
    if outside != 0 {
        return None;
    }
    if near {
        return Some([0, 0, size[0], size[1]]);
    }
    if minimum.z > 1.0 || maximum.z < 0.0 {
        return None;
    }
    let left = ((minimum.x + jitter[0]) * 0.5 + 0.5) * size[0] as f32;
    let right = ((maximum.x + jitter[0]) * 0.5 + 0.5) * size[0] as f32;
    let top = (0.5 - (maximum.y + jitter[1]) * 0.5) * size[1] as f32;
    let bottom = (0.5 - (minimum.y + jitter[1]) * 0.5) * size[1] as f32;
    let x = left.floor().clamp(0.0, size[0] as f32) as u32;
    let y = top.floor().clamp(0.0, size[1] as f32) as u32;
    let right = right.ceil().clamp(0.0, size[0] as f32) as u32;
    let bottom = bottom.ceil().clamp(0.0, size[1] as f32) as u32;
    (right > x && bottom > y).then_some([x, y, right.saturating_sub(x), bottom.saturating_sub(y)])
}
