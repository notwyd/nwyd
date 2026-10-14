use crate::camera::Matrix4;
use crate::device::Gpu;
use crate::pack;
use crate::preset::{Preset, d3d};
use crate::quirk::VendorClass;
use crate::world::FrameUniforms;
use bytemuck::{Pod, Zeroable};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]

pub struct BillboardVertex {
    pub position: [f32; 3],
    pub diffuse: u32,
    pub uv: [f32; 2],
}

impl BillboardVertex {
    pub const STRIDE: u64 = 0x18;

    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: BillboardVertex::STRIDE,
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
        ],
    };
}

pub const QUAD_INDICES: [u16; 6] = [0, 1, 2, 0, 2, 3];

pub const QUAD_VERTICES: u32 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]

pub enum BillboardPass {
    Blended,
    Additive,
}

impl BillboardPass {
    pub const ALL: [BillboardPass; 2] = [BillboardPass::Blended, BillboardPass::Additive];

    pub fn from_additive(additive: bool) -> BillboardPass {
        if additive {
            BillboardPass::Additive
        } else {
            BillboardPass::Blended
        }
    }

    pub fn is_additive(self) -> bool {
        self == BillboardPass::Additive
    }

    pub fn blend_state(self) -> wgpu::BlendState {
        let dst = match self {
            BillboardPass::Blended => d3d::BLEND_INVSRCALPHA,
            BillboardPass::Additive => d3d::BLEND_ONE,
        };

        let component = wgpu::BlendComponent {
            src_factor: crate::preset::blend_factor(d3d::BLEND_SRCALPHA),
            dst_factor: crate::preset::blend_factor(dst),
            operation: wgpu::BlendOperation::Add,
        };

        wgpu::BlendState {
            color: component,
            alpha: component,
        }
    }

    fn label(self) -> &'static str {
        match self {
            BillboardPass::Blended => "billboard (blended)",
            BillboardPass::Additive => "billboard (additive)",
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]

pub struct BillboardUniforms {
    pub world: [f32; 16],
    pub flags: [f32; 4],
}

impl BillboardUniforms {
    pub fn new(world: Matrix4, pass: BillboardPass) -> BillboardUniforms {
        BillboardUniforms::with_alpha_op(world, pass, pass.is_additive())
    }

    pub fn with_alpha_op(
        world: Matrix4,
        pass: BillboardPass,
        texture_alpha: bool,
    ) -> BillboardUniforms {
        let _ = pass;

        BillboardUniforms {
            world: world.shader_constants(),
            flags: [f32::from(u8::from(texture_alpha)), 0.0, 0.0, 0.0],
        }
    }

    pub fn with_layer(mut self, layer: u32) -> BillboardUniforms {
        self.flags[1] = layer as f32;

        self
    }

    pub fn with_opaque(mut self, policy: crate::texture::AlphaPolicy) -> BillboardUniforms {
        self.flags[2] = policy.as_uniform();

        self
    }

    pub fn forces_opaque(&self) -> bool {
        self.flags[2] != 0.0
    }

    pub fn layer(&self) -> u32 {
        self.flags[1] as u32
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct BillboardDraw {
    pub pass: BillboardPass,
    pub object: u32,
    pub texture: usize,
    pub base_vertex: u32,
    pub indices: std::ops::Range<u32>,
}

impl BillboardDraw {
    pub fn quad(pass: BillboardPass, object: u32, texture: usize) -> BillboardDraw {
        let indices = object * QUAD_INDICES.len() as u32;

        BillboardDraw {
            pass,
            object,
            texture,
            base_vertex: object * QUAD_VERTICES,
            indices: indices..indices + QUAD_INDICES.len() as u32,
        }
    }
}

pub struct BillboardArray {
    pub bind_group: wgpu::BindGroup,
}

pub struct BillboardRenderer {
    pipelines: Vec<(BillboardPass, wgpu::RenderPipeline)>,
    frame_layout: wgpu::BindGroupLayout,
    object_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
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
    draws: Vec<BillboardDraw>,
}

impl BillboardRenderer {
    pub fn new(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> BillboardRenderer {
        Self::with_temporal(gpu, target_format, depth_format, None)
    }

    pub fn with_temporal(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        temporal: Option<&crate::temporal::TemporalCamera>,
    ) -> BillboardRenderer {
        let shader = crate::shaders::module(
            &gpu.device,
            if temporal.is_some() {
                crate::shaders::Shader::TemporalBillboard
            } else {
                crate::shaders::Shader::Billboard
            },
        );

        let frame_layout = uniform_layout(
            gpu,
            "billboard frame",
            std::mem::size_of::<FrameUniforms>() as u64,
            false,
        );

        let object_layout = uniform_layout(
            gpu,
            "billboard object",
            std::mem::size_of::<BillboardUniforms>() as u64,
            true,
        );

        let texture_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: crate::gpu_label("billboard texture"),
                    entries: &[
                        wgpu::BindGroupLayoutEntry {
                            binding: 0,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2Array,
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
                label: crate::gpu_label("billboard"),
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

        let pipelines = BillboardPass::ALL
            .into_iter()
            .map(|pass| {
                let pipeline = gpu
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: crate::gpu_label(pass.label()),
                        layout: Some(&layout),
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: Some("vs"),
                            compilation_options: Default::default(),
                            buffers: &[Some(BillboardVertex::LAYOUT)],
                        },
                        primitive: PRIMITIVE,
                        depth_stencil: Some(depth_state(depth_format)),
                        multisample: wgpu::MultisampleState::default(),
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: Some("fs"),
                            compilation_options: crate::temporal::fragment_options(
                                temporal.is_some(),
                                Some(pass.blend_state()),
                                false,
                            ),
                            targets: &crate::temporal::targets(
                                wgpu::ColorTargetState {
                                    format: target_format,
                                    blend: Some(pass.blend_state()),
                                    write_mask: wgpu::ColorWrites::ALL,
                                },
                                temporal.is_some(),
                                false,
                            ),
                        }),
                        multiview_mask: None,
                        cache: None,
                    });

                (pass, pipeline)
            })
            .collect();

        let preset = Preset::lit_3d(VendorClass::DEFAULT);

        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: crate::gpu_label("billboard (SAMP0)"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..preset.sampler_descriptor()
        });

        let frame_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("billboard frame"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: crate::gpu_label("billboard frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });

        let object_stride = object_stride(gpu.device.limits().min_uniform_buffer_offset_alignment);

        let object_capacity = 256;

        let (object_buffer, object_bind_group) =
            object_storage(gpu, &object_layout, object_stride, object_capacity);

        let vertex_capacity = 256 * u64::from(QUAD_VERTICES) * BillboardVertex::STRIDE;

        let vertex_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("billboard vertices"),
            size: vertex_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_capacity = 256 * std::mem::size_of_val(&QUAD_INDICES) as u64;

        let index_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("billboard indices"),
            size: index_capacity,
            usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        BillboardRenderer {
            pipelines,
            frame_layout,
            object_layout,
            texture_layout,
            sampler,
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

    pub fn bind_array(&self, gpu: &Gpu, view: &wgpu::TextureView) -> BillboardArray {
        BillboardArray {
            bind_group: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: crate::gpu_label("billboard texture array"),
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
        uniforms: &FrameUniforms,
        objects: &[BillboardUniforms],
        vertices: &[BillboardVertex],
        indices: &[u16],
        draws: &[BillboardDraw],
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
            self.grow_vertices(gpu, vertices.len() as u64 * BillboardVertex::STRIDE);

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

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, arrays: &[BillboardArray]) {
        if self.draws.is_empty() {
            return;
        }

        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        pass.set_bind_group(0, &self.frame_bind_group, &[]);

        let mut bound: Option<BillboardPass> = None;

        let mut bound_array: Option<usize> = None;

        for draw in &self.draws {
            let Some(array) = arrays.get(draw.texture) else {
                continue;
            };

            if bound != Some(draw.pass) {
                let Some((_, pipeline)) = self.pipelines.iter().find(|(p, _)| *p == draw.pass)
                else {
                    continue;
                };

                pass.set_pipeline(pipeline);

                bound = Some(draw.pass);
            }

            pass.set_bind_group(
                1,
                &self.object_bind_group,
                &[draw.object * self.object_stride],
            );

            if bound_array != Some(draw.texture) {
                pass.set_bind_group(2, &array.bind_group, &[]);

                bound_array = Some(draw.texture);
            }

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
            label: crate::gpu_label("billboard vertices"),
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
            label: crate::gpu_label("billboard indices"),
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
        depth_write_enabled: Some(false),
        depth_compare: Some(wgpu::CompareFunction::LessEqual),
        stencil: wgpu::StencilState::default(),
        bias: wgpu::DepthBiasState::default(),
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
        label: crate::gpu_label("billboard object"),
        size: u64::from(stride) * u64::from(capacity),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: crate::gpu_label("billboard object"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(std::mem::size_of::<BillboardUniforms>() as u64),
            }),
        }],
    });

    (buffer, bind_group)
}

fn object_stride(alignment: u32) -> u32 {
    let size = std::mem::size_of::<BillboardUniforms>() as u32;

    size.max(alignment).next_multiple_of(alignment.max(1))
}

pub const BILLBOARD_WGSL: &str = include_str!("shaders/billboard.wgsl");
