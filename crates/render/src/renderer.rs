use crate::device::Gpu;
use crate::pack;
use crate::preset::{Preset, RenderMode};
use crate::sprite::{SpriteBatcher, TlVertex};
use crate::texture::{AlphaPolicy, GpuTexture};
use bytemuck::{Pod, Zeroable};
use std::collections::HashMap;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]

pub struct BatchUniforms {
    pub viewport: [f32; 2],
    pub alpha_reference: f32,
    pub opaque: f32,
    pub select_texture_rgb: f32,
    pub _pad: [f32; 3],
}

pub struct TextureSet<K = u32> {
    textures: HashMap<K, BoundTexture>,
}

impl<K> Default for TextureSet<K> {
    fn default() -> TextureSet<K> {
        TextureSet {
            textures: HashMap::new(),
        }
    }
}

impl<K: Eq + std::hash::Hash> TextureSet<K> {
    pub fn new() -> TextureSet<K> {
        TextureSet::default()
    }

    pub fn insert(&mut self, id: K, texture: BoundTexture) {
        self.textures.insert(id, texture);
    }

    pub fn get(&self, id: &K) -> Option<&BoundTexture> {
        self.textures.get(id)
    }

    pub fn remove(&mut self, id: &K) -> bool {
        self.textures.remove(id).is_some()
    }

    pub fn len(&self) -> usize {
        self.textures.len()
    }

    pub fn is_empty(&self) -> bool {
        self.textures.is_empty()
    }

    pub fn keys(&self) -> impl Iterator<Item = &K> {
        self.textures.keys()
    }
}

pub struct BoundTexture {
    pub bind_group: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
    pub alpha: AlphaPolicy,
    cube: bool,
}

pub struct SpriteRenderer {
    viewport: (u32, u32),
    pipelines: HashMap<(RenderMode, bool), wgpu::RenderPipeline>,
    uniform_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    cube_layout: wgpu::BindGroupLayout,
    samplers: HashMap<RenderMode, wgpu::Sampler>,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    uniform_stride: u32,
    uniform_capacity: u32,
    vertex_buffer: wgpu::Buffer,
    vertex_capacity: u64,
    index_buffer: wgpu::Buffer,
    index_capacity: u32,
    scratch: Vec<u8>,
}

const MODES: [RenderMode; 6] = [
    RenderMode::Ui2dPoint,
    RenderMode::Ui2dLinear,
    RenderMode::SControl2d,
    RenderMode::NuiPanel2d,
    RenderMode::NuiHighlight2d,
    RenderMode::LensFlare2d,
];

impl SpriteRenderer {
    pub fn new(gpu: &Gpu, target_format: wgpu::TextureFormat) -> SpriteRenderer {
        let cube_shader = crate::shaders::module(&gpu.device, crate::shaders::Shader::CubeSprite);

        let shader = crate::shaders::module(&gpu.device, crate::shaders::Shader::Sprite);

        let uniform_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: crate::gpu_label("sprite uniforms"),
                    entries: &[wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: true,
                            min_binding_size: wgpu::BufferSize::new(
                                std::mem::size_of::<BatchUniforms>() as u64,
                            ),
                        },
                        count: None,
                    }],
                });

        let texture_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: crate::gpu_label("sprite texture"),
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

        let cube_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: crate::gpu_label("cube sprite texture"),
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::Cube,
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

        let cube_pipeline_layout =
            gpu.device
                .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                    label: crate::gpu_label("cube sprite"),
                    bind_group_layouts: &[Some(&uniform_layout), Some(&cube_layout)],
                    immediate_size: 0,
                });

        let layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: crate::gpu_label("sprite"),
                bind_group_layouts: &[Some(&uniform_layout), Some(&texture_layout)],
                immediate_size: 0,
            });

        let mut pipelines = HashMap::new();

        let mut samplers = HashMap::new();

        for mode in MODES {
            let preset = Preset::for_mode(mode).expect("every 2D mode has a preset");

            for cube in [false, true] {
                let shader = if cube { &cube_shader } else { &shader };

                let layout = if cube { &cube_pipeline_layout } else { &layout };

                pipelines.insert(
                    (mode, cube),
                    gpu.device
                        .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                            label: crate::gpu_label(match mode {
                                RenderMode::Ui2dPoint => "mode 2 (point, blended)",
                                RenderMode::Ui2dLinear => "mode 3 (linear, alpha-tested)",
                                RenderMode::NuiPanel2d => "kernel UI pass",
                                RenderMode::NuiHighlight2d => "kernel UI highlight pass",
                                _ => "S* 2D (mode 0 + overrides)",
                            }),
                            layout: Some(layout),
                            vertex: wgpu::VertexState {
                                module: shader,
                                entry_point: Some("vs"),
                                compilation_options: Default::default(),
                                buffers: &[Some(TlVertex::LAYOUT)],
                            },
                            primitive: preset.primitive_state(),
                            depth_stencil: None,
                            multisample: wgpu::MultisampleState::default(),
                            fragment: Some(wgpu::FragmentState {
                                module: shader,
                                entry_point: Some("fs"),
                                compilation_options: Default::default(),
                                targets: &[Some(wgpu::ColorTargetState {
                                    format: target_format,
                                    blend: preset.blend_state(),
                                    write_mask: wgpu::ColorWrites::ALL,
                                })],
                            }),
                            multiview_mask: None,
                            cache: None,
                        }),
                );
            }

            samplers.insert(
                mode,
                gpu.device.create_sampler(&preset.sampler_descriptor()),
            );
        }

        let uniform_stride =
            uniform_stride(gpu.device.limits().min_uniform_buffer_offset_alignment);

        let uniform_capacity = 64;

        let uniform_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("sprite uniforms"),
            size: u64::from(uniform_stride) * u64::from(uniform_capacity),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let uniform_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: crate::gpu_label("sprite uniforms"),
            layout: &uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &uniform_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<BatchUniforms>() as u64),
                }),
            }],
        });

        let vertex_capacity = 4 * 1024 * TlVertex::STRIDE;

        let vertex_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("sprite vertices"),
            size: vertex_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let index_capacity = 1024;

        let index_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: crate::gpu_label("sprite indices"),
                contents: bytemuck::cast_slice(&crate::sprite::indices(index_capacity)),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            });

        SpriteRenderer {
            viewport: (0, 0),
            pipelines,
            uniform_layout,
            texture_layout,
            cube_layout,
            samplers,
            uniform_buffer,
            uniform_bind_group,
            uniform_stride,
            uniform_capacity,
            vertex_buffer,
            vertex_capacity,
            index_buffer,
            index_capacity,
            scratch: Vec::new(),
        }
    }

    pub fn bind(&self, gpu: &Gpu, texture: &GpuTexture, mode: RenderMode) -> BoundTexture {
        self.bind_resource(gpu, texture, mode, false)
    }

    pub fn bind_cube(&self, gpu: &Gpu, texture: &GpuTexture, mode: RenderMode) -> BoundTexture {
        self.bind_resource(gpu, texture, mode, true)
    }

    fn bind_resource(
        &self,
        gpu: &Gpu,
        texture: &GpuTexture,
        mode: RenderMode,
        cube: bool,
    ) -> BoundTexture {
        let sampler = self.samplers.get(&mode).expect("a 2D mode");

        BoundTexture {
            bind_group: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: crate::gpu_label("sprite texture"),
                layout: if cube {
                    &self.cube_layout
                } else {
                    &self.texture_layout
                },
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&texture.view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(sampler),
                    },
                ],
            }),
            width: texture.width,
            height: texture.height,
            alpha: texture.alpha,
            cube,
        }
    }

    pub fn prepare<K: Copy + Eq + std::hash::Hash>(
        &mut self,
        gpu: &Gpu,
        batcher: &SpriteBatcher<K>,
        viewport: (u32, u32),
        textures: &TextureSet<K>,
    ) {
        self.viewport = viewport;

        if batcher.is_empty() {
            return;
        }

        self.grow_vertices(gpu, batcher.vertices().len() as u64);

        self.grow_indices(gpu, batcher.quad_count());

        self.grow_uniforms(gpu, batcher.batches().len() as u32);

        gpu.queue.write_buffer(
            &self.vertex_buffer,
            0,
            bytemuck::cast_slice(batcher.vertices()),
        );

        let blocks = batcher.batches().iter().map(|batch| {
            let preset = Preset::for_mode(batch.mode).expect("a 2D mode");

            let opaque = textures
                .get(&batch.texture)
                .map(|t| t.alpha)
                .unwrap_or(AlphaPolicy::Stored)
                .as_uniform();

            BatchUniforms {
                viewport: [viewport.0 as f32, viewport.1 as f32],
                alpha_reference: preset.alpha_reference_normalised(),
                opaque,
                select_texture_rgb: preset.color_op_uniform(),
                _pad: [0.0; 3],
            }
        });

        let packed = pack::strided(&mut self.scratch, self.uniform_stride, blocks);

        gpu.queue.write_buffer(&self.uniform_buffer, 0, packed);
    }

    pub fn draw<K: Copy + Eq + std::hash::Hash>(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        batcher: &SpriteBatcher<K>,
        textures: &TextureSet<K>,
    ) {
        self.draw_range(pass, batcher, textures, 0..batcher.quad_count());
    }

    pub fn draw_range<K: Copy + Eq + std::hash::Hash>(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        batcher: &SpriteBatcher<K>,
        textures: &TextureSet<K>,
        range: std::ops::Range<u32>,
    ) {
        if batcher.is_empty() {
            return;
        }

        pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));

        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

        let mut bound = None;

        for (i, batch) in batcher.batches().iter().enumerate() {
            let first = range.start.max(batch.first_quad);

            let end = range.end.min(batch.first_quad + batch.quad_count);

            if first >= end {
                continue;
            }

            let [x, y, w, h] = sprite_scissor(batch.clip, self.viewport);

            if w == 0 || h == 0 {
                continue;
            }

            pass.set_scissor_rect(x, y, w, h);

            let Some(texture) = textures.get(&batch.texture) else {
                continue;
            };

            let key = (batch.mode, texture.cube);

            if bound != Some(key) {
                let pipeline = self
                    .pipelines
                    .get(&key)
                    .expect("a sprite mode/resource pair");

                pass.set_pipeline(pipeline);

                bound = Some(key);
            }

            pass.set_bind_group(
                0,
                &self.uniform_bind_group,
                &[i as u32 * self.uniform_stride],
            );

            pass.set_bind_group(1, &texture.bind_group, &[]);

            pass.draw_indexed(first * 6..end * 6, 0, 0..1);
        }

        if self.viewport.0 > 0 && self.viewport.1 > 0 {
            pass.set_scissor_rect(0, 0, self.viewport.0, self.viewport.1);
        }
    }

    fn grow_vertices(&mut self, gpu: &Gpu, count: u64) {
        let needed = count * TlVertex::STRIDE;

        if needed <= self.vertex_capacity {
            return;
        }

        self.vertex_capacity = needed.next_power_of_two();

        self.vertex_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("sprite vertices"),
            size: self.vertex_capacity,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
    }

    fn grow_indices(&mut self, gpu: &Gpu, quads: u32) {
        if quads <= self.index_capacity {
            return;
        }

        self.index_capacity = quads.next_power_of_two();

        self.index_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: crate::gpu_label("sprite indices"),
                contents: bytemuck::cast_slice(&crate::sprite::indices(self.index_capacity)),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            });
    }

    fn grow_uniforms(&mut self, gpu: &Gpu, batches: u32) {
        if batches <= self.uniform_capacity {
            return;
        }

        self.uniform_capacity = batches.next_power_of_two();

        self.uniform_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("sprite uniforms"),
            size: u64::from(self.uniform_stride) * u64::from(self.uniform_capacity),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        self.uniform_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: crate::gpu_label("sprite uniforms"),
            layout: &self.uniform_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                    buffer: &self.uniform_buffer,
                    offset: 0,
                    size: wgpu::BufferSize::new(std::mem::size_of::<BatchUniforms>() as u64),
                }),
            }],
        });
    }
}

fn sprite_scissor(clip: Option<[f32; 4]>, viewport: (u32, u32)) -> [u32; 4] {
    let Some([left, top, right, bottom]) = clip else {
        return [0, 0, viewport.0, viewport.1];
    };

    if ![left, top, right, bottom].iter().all(|v| v.is_finite()) {
        return [0; 4];
    }

    let x = left.ceil().clamp(0.0, viewport.0 as f32) as u32;

    let y = top.ceil().clamp(0.0, viewport.1 as f32) as u32;

    let right = right.ceil().clamp(0.0, viewport.0 as f32) as u32;

    let bottom = bottom.ceil().clamp(0.0, viewport.1 as f32) as u32;

    [x, y, right.saturating_sub(x), bottom.saturating_sub(y)]
}

fn uniform_stride(alignment: u32) -> u32 {
    let size = std::mem::size_of::<BatchUniforms>() as u32;

    size.max(alignment).next_multiple_of(alignment.max(1))
}
