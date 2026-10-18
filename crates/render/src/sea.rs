use crate::camera::Matrix4;
use crate::device::Gpu;
use crate::pack;
use crate::preset::Preset;
use crate::quirk::VendorClass;
use crate::world::{FrameUniforms, Material};
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct SeaVertex {
    pub position: [f32; 3],
    pub diffuse: u32,
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
}

impl SeaVertex {
    pub const STRIDE: u64 = 0x20;

    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: SeaVertex::STRIDE,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Unorm8x4,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 16,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 24,
                shader_location: 3,
            },
        ],
    };
}

pub struct SeaMaterial;

impl SeaMaterial {
    pub const MATERIAL: Material = Material {
        diffuse: [0.5, 0.5, 0.5, 0.9],
        ambient: [0.5, 0.5, 0.5, 0.9],
        specular: [0.5, 0.5, 0.5, 0.9],
        emissive: [0.1, 0.1, 0.1, 0.1],
        power: 0.0,
    };
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct SeaUniforms {
    pub world: [f32; 16],
    pub material: [f32; 4],
}

impl SeaUniforms {
    pub fn new(world: Matrix4) -> SeaUniforms {
        SeaUniforms {
            world: world.shader_constants(),
            material: [0.5, 0.1, 2.0, 1.0],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeaPass {
    Outdoor,
    OutdoorOpaque,
    Lava,
    Indoor,
    IndoorOpaque,
}

impl SeaPass {
    pub const ALL: [Self; 5] = [
        Self::Outdoor,
        Self::OutdoorOpaque,
        Self::Lava,
        Self::Indoor,
        Self::IndoorOpaque,
    ];

    pub fn depth_write(self) -> bool {
        !matches!(self, Self::Indoor | Self::IndoorOpaque)
    }

    pub fn blend(self) -> Option<wgpu::BlendState> {
        let (src, dst) = match self {
            Self::Outdoor => return Some(BLEND),
            Self::OutdoorOpaque | Self::IndoorOpaque => return None,
            Self::Lava => (wgpu::BlendFactor::OneMinusSrc, wgpu::BlendFactor::Src),
            Self::Indoor => (
                wgpu::BlendFactor::SrcAlpha,
                wgpu::BlendFactor::OneMinusSrcAlpha,
            ),
        };

        let c = wgpu::BlendComponent {
            src_factor: src,
            dst_factor: dst,
            operation: wgpu::BlendOperation::Add,
        };

        Some(wgpu::BlendState { color: c, alpha: c })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SeaDraw {
    pub pass: SeaPass,
    pub object: u32,
    pub texture: usize,
    pub base_vertex: u32,
    pub indices: std::ops::Range<u32>,
}

pub struct SeaTexture {
    pub bind_group: wgpu::BindGroup,
}

pub struct SeaRenderer {
    pipelines: Vec<(SeaPass, wgpu::RenderPipeline)>,
    frame_layout: wgpu::BindGroupLayout,
    object_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    base_sampler: wgpu::Sampler,
    detail_sampler: wgpu::Sampler,
    frame_buffer: wgpu::Buffer,
    frame_bind_group: wgpu::BindGroup,
    object_buffer: wgpu::Buffer,
    object_bind_group: wgpu::BindGroup,
    object_stride: u32,
    object_capacity: u32,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    index_buffer: wgpu::Buffer,
    index_capacity: u64,
    scratch: Vec<u8>,
    index_scratch: Vec<u16>,
    draws: Vec<SeaDraw>,
}

impl SeaRenderer {
    pub fn new(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> SeaRenderer {
        Self::with_temporal(gpu, target_format, depth_format, None)
    }

    pub fn with_temporal(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        temporal: Option<&crate::temporal::TemporalCamera>,
    ) -> SeaRenderer {
        let shader = crate::shaders::module(
            &gpu.device,
            if temporal.is_some() {
                crate::shaders::Shader::TemporalSea
            } else {
                crate::shaders::Shader::Sea
            },
        );

        let frame_layout = uniform_layout(
            gpu,
            "sea frame",
            std::mem::size_of::<FrameUniforms>() as u64,
            false,
        );

        let object_layout = uniform_layout(
            gpu,
            "sea object",
            std::mem::size_of::<SeaUniforms>() as u64,
            true,
        );

        let texture_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: crate::gpu_label("sea textures"),
                    entries: &[
                        texture_entry(0),
                        sampler_entry(1),
                        texture_entry(2),
                        sampler_entry(3),
                    ],
                });

        let layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: crate::gpu_label("sea"),
                bind_group_layouts: &crate::temporal::layouts(
                    &[
                        Some(&frame_layout),
                        Some(&object_layout),
                        Some(&texture_layout),
                    ],
                    temporal,
                ),
                immediate_size: 0,
            });

        let pipelines = SeaPass::ALL
            .into_iter()
            .map(|kind| {
                let pipeline = gpu
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: crate::gpu_label("sea"),
                        layout: Some(&layout),
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: Some("vs"),
                            compilation_options: Default::default(),
                            buffers: &[Some(SeaVertex::LAYOUT)],
                        },
                        primitive: wgpu::PrimitiveState {
                            cull_mode: if kind == SeaPass::Lava {
                                Some(wgpu::Face::Back)
                            } else {
                                None
                            },
                            ..PRIMITIVE
                        },
                        depth_stencil: Some(wgpu::DepthStencilState {
                            depth_write_enabled: Some(kind.depth_write()),
                            ..depth_state(depth_format)
                        }),
                        multisample: wgpu::MultisampleState::default(),
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: Some("fs"),
                            compilation_options: crate::temporal::fragment_options(
                                temporal.is_some(),
                                kind.blend(),
                                kind.depth_write(),
                            ),
                            targets: &crate::temporal::targets(
                                wgpu::ColorTargetState {
                                    format: target_format,
                                    blend: kind.blend(),
                                    write_mask: wgpu::ColorWrites::COLOR,
                                },
                                temporal.is_some(),
                                kind.depth_write(),
                            ),
                        }),
                        multiview_mask: None,
                        cache: None,
                    });

                (kind, pipeline)
            })
            .collect();

        let preset = Preset::lit_3d(VendorClass::DEFAULT);

        let base_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: crate::gpu_label("sea base (SAMP0)"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..preset.material_sampler_descriptor(gpu.adapter.get_downlevel_capabilities().flags)
        });

        let detail_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: crate::gpu_label("sea detail (SAMP1)"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..preset.material_sampler_descriptor(gpu.adapter.get_downlevel_capabilities().flags)
        });

        let frame_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("sea frame"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: crate::gpu_label("sea frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });

        let object_stride = object_stride(gpu.device.limits().min_uniform_buffer_offset_alignment);

        let object_capacity = 8;

        let (object_buffer, object_bind_group) =
            object_storage(gpu, &object_layout, object_stride, object_capacity);

        let vertex_capacity = 1024 * SeaVertex::STRIDE;

        let vertex_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("sea vertices"),
            size: vertex_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_capacity = 4096 * std::mem::size_of::<u16>() as u64;

        let index_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("sea indices"),
            size: index_capacity,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        SeaRenderer {
            pipelines,
            frame_layout,
            object_layout,
            texture_layout,
            base_sampler,
            detail_sampler,
            frame_buffer,
            frame_bind_group,
            object_buffer,
            object_bind_group,
            object_stride,
            object_capacity,
            vertex_buffer,
            vertex_capacity,
            index_buffer,
            index_capacity,
            draws: Vec::new(),
            scratch: Vec::new(),
            index_scratch: Vec::new(),
        }
    }

    pub fn bind_textures(
        &self,
        gpu: &Gpu,
        base: &wgpu::TextureView,
        detail: &wgpu::TextureView,
    ) -> SeaTexture {
        SeaTexture {
            bind_group: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: crate::gpu_label("sea textures"),
                layout: &self.texture_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(base),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.base_sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(detail),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.detail_sampler),
                    },
                ],
            }),
        }
    }

    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        uniforms: &FrameUniforms,
        objects: &[SeaUniforms],
        vertices: &[SeaVertex],
        indices: &[u16],
        draws: &[SeaDraw],
    ) {
        gpu.queue
            .write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(uniforms));

        self.grow_objects(gpu, objects.len() as u32);

        pack::upload(
            gpu,
            &self.object_buffer,
            &mut self.scratch,
            self.object_stride,
            objects,
        );

        if !vertices.is_empty() {
            self.grow_vertices(gpu, vertices.len() as u64 * SeaVertex::STRIDE);

            gpu.queue
                .write_buffer(&self.vertex_buffer, 0, bytemuck::cast_slice(vertices));
        }

        if !indices.is_empty() {
            let bytes = std::mem::size_of_val(pack::indices(&mut self.index_scratch, indices));

            self.grow_indices(gpu, bytes as u64);

            let padded = self.index_scratch.as_slice();

            gpu.queue
                .write_buffer(&self.index_buffer, 0, bytemuck::cast_slice(padded));
        }

        self.draws.clear();

        self.draws.extend_from_slice(draws);
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, textures: &[SeaTexture]) {
        if self.draws.is_empty() {
            return;
        }

        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        pass.set_bind_group(0, &self.frame_bind_group, &[]);

        for draw in &self.draws {
            let Some((_, pipeline)) = self.pipelines.iter().find(|(kind, _)| *kind == draw.pass)
            else {
                continue;
            };

            pass.set_pipeline(pipeline);

            let Some(texture) = textures.get(draw.texture) else {
                continue;
            };

            pass.set_bind_group(
                1,
                &self.object_bind_group,
                &[draw.object * self.object_stride],
            );

            pass.set_bind_group(2, &texture.bind_group, &[]);

            pass.draw_indexed(draw.indices.clone(), draw.base_vertex as i32, 0..1);
        }
    }

    pub fn draw_count(&self) -> usize {
        self.draws.len()
    }

    pub fn frame_layout(&self) -> &wgpu::BindGroupLayout {
        &self.frame_layout
    }

    fn grow_vertices(&mut self, gpu: &Gpu, bytes: u64) {
        if bytes <= self.vertex_capacity {
            return;
        }

        self.vertex_capacity = bytes.next_power_of_two();

        self.vertex_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("sea vertices"),
            size: self.vertex_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }

    fn grow_indices(&mut self, gpu: &Gpu, bytes: u64) {
        if bytes <= self.index_capacity {
            return;
        }

        self.index_capacity = bytes.next_power_of_two();

        self.index_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("sea indices"),
            size: self.index_capacity,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }

    fn grow_objects(&mut self, gpu: &Gpu, count: u32) {
        if count <= self.object_capacity {
            return;
        }

        self.object_capacity = count.next_power_of_two();

        let (buffer, bind_group) = object_storage(
            gpu,
            &self.object_layout,
            self.object_stride,
            self.object_capacity,
        );

        self.object_buffer = buffer;

        self.object_bind_group = bind_group;
    }
}

const BLEND: wgpu::BlendState = {
    let component = wgpu::BlendComponent {
        src_factor: wgpu::BlendFactor::Src,
        dst_factor: wgpu::BlendFactor::One,
        operation: wgpu::BlendOperation::Add,
    };

    wgpu::BlendState {
        color: component,
        alpha: component,
    }
};

const PRIMITIVE: wgpu::PrimitiveState = wgpu::PrimitiveState {
    topology: wgpu::PrimitiveTopology::TriangleList,
    strip_index_format: None,
    front_face: wgpu::FrontFace::Cw,
    cull_mode: None,
    unclipped_depth: false,
    polygon_mode: wgpu::PolygonMode::Fill,
    conservative: false,
};

fn depth_state(format: wgpu::TextureFormat) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format,
        depth_write_enabled: Some(true),
        depth_compare: Some(wgpu::CompareFunction::LessEqual),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

fn sampler_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
        count: None,
    }
}

fn uniform_layout(gpu: &Gpu, label: &str, size: u64, dynamic: bool) -> wgpu::BindGroupLayout {
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: crate::gpu_label(label),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: dynamic,
                    min_binding_size: wgpu::BufferSize::new(size),
                },
                count: None,
            }],
        })
}

fn object_storage(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    stride: u32,
    capacity: u32,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: crate::gpu_label("sea object"),
        size: u64::from(stride) * u64::from(capacity),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: crate::gpu_label("sea object"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(std::mem::size_of::<SeaUniforms>() as u64),
            }),
        }],
    });

    (buffer, bind_group)
}

fn object_stride(alignment: u32) -> u32 {
    let size = std::mem::size_of::<SeaUniforms>() as u32;

    size.max(alignment).next_multiple_of(alignment.max(1))
}

pub const SEA_WGSL: &str = include_str!("shaders/sea.wgsl");
