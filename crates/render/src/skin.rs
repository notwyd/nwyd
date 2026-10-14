use crate::camera::Matrix4;
use crate::device::Gpu;
use crate::pack;
use crate::preset::d3d;
use crate::preset::{Preset, RenderMode};
use crate::quirk::VendorClass;
use bytemuck::{Pod, Zeroable};
use formats::mesh::SkinVertex;
use formats::mesh::glow;
use formats::mesh::msh::{MAX_BONE_PALETTE, SkinMesh};

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct SkinVertexGpu {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub bone_indices: [u8; 4],
    pub bone_weights: [f32; 4],
}

impl SkinVertexGpu {
    pub const STRIDE: u64 = 52;

    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: SkinVertexGpu::STRIDE,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 0,
                shader_location: 0,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x3,
                offset: 12,
                shader_location: 1,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 24,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Uint8x4,
                offset: 32,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x4,
                offset: 36,
                shader_location: 4,
            },
        ],
    };

    pub fn from_skin(v: &SkinVertex) -> SkinVertexGpu {
        SkinVertexGpu {
            position: v.position,
            normal: v.normal,
            uv: v.uv,
            bone_indices: v.bone_indices,
            bone_weights: v.bone_weights,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkinAlloc {
    pub base_vertex: i32,
    pub start_index: u32,
    pub index_count: u32,
    pub vertex_count: u32,
    pub palette_len: u32,
}

impl SkinAlloc {
    pub fn range(&self) -> std::ops::Range<u32> {
        self.start_index..self.start_index + self.index_count
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct SkinArena {
    pub vertices: Vec<SkinVertexGpu>,
    pub indices: Vec<u16>,
    pub allocs: Vec<SkinAlloc>,
}

impl SkinArena {
    pub fn push(&mut self, mesh: &SkinMesh) -> usize {
        let base_vertex = self.vertices.len() as i32;

        let start_index = self.indices.len() as u32;

        self.vertices
            .extend(mesh.vertices.iter().map(SkinVertexGpu::from_skin));

        self.indices.extend_from_slice(&mesh.indices);

        self.allocs.push(SkinAlloc {
            base_vertex,
            start_index,
            index_count: (mesh.indices.len() as u32 / 3) * 3,
            vertex_count: mesh.vertices.len() as u32,
            palette_len: mesh.header.num_matrices,
        });

        self.allocs.len() - 1
    }

    pub fn len(&self) -> usize {
        self.allocs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.allocs.is_empty()
    }

    pub fn triangle_count(&self) -> u32 {
        self.allocs.iter().map(|a| a.index_count / 3).sum()
    }

    pub fn max_addressed_slot(&self) -> Option<(usize, u8)> {
        self.allocs
            .iter()
            .enumerate()
            .filter_map(|(i, a)| {
                let start = a.base_vertex as usize;

                self.vertices[start..start + a.vertex_count as usize]
                    .iter()
                    .flat_map(|v| {
                        v.bone_indices
                            .iter()
                            .zip(v.bone_weights.iter())
                            .filter(|&(_, &w)| w != 0.0)
                            .map(|(&b, _)| b)
                    })
                    .max()
                    .map(|b| (i, b))
            })
            .max_by_key(|&(_, b)| b)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct BonePalette {
    pub bones: [[f32; 16]; MAX_BONE_PALETTE],
}

pub type MotionKey = (u64, u64);

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TemporalPalette {
    current: BonePalette,
    previous: BonePalette,
    previous_projection: [f32; 16],
    history: [f32; 4],
}

#[derive(Default)]
struct PaletteHistory {
    previous: std::collections::HashMap<MotionKey, (BonePalette, [f32; 16])>,
}

impl PaletteHistory {
    fn pack(
        &mut self,
        palettes: &[BonePalette],
        keys: &[Option<MotionKey>],
        projection: [f32; 16],
    ) -> Vec<TemporalPalette> {
        let mut next = std::collections::HashMap::with_capacity(palettes.len());

        let result = palettes
            .iter()
            .enumerate()
            .map(|(index, current)| {
                let key = keys.get(index).copied().flatten();

                let previous = key.and_then(|key| self.previous.get(&key));

                let (old, old_projection) = previous.copied().unwrap_or((*current, projection));

                if let Some(key) = key {
                    next.insert(key, (*current, projection));
                }

                TemporalPalette {
                    current: *current,
                    previous: old,
                    previous_projection: old_projection,
                    history: [if previous.is_some() { 0.0 } else { 1.0 }, 0.0, 0.0, 0.0],
                }
            })
            .collect();

        self.previous = next;

        result
    }
}

impl Default for BonePalette {
    fn default() -> BonePalette {
        BonePalette {
            bones: [Matrix4::IDENTITY.shader_constants(); MAX_BONE_PALETTE],
        }
    }
}

impl BonePalette {
    pub fn build(
        mesh: &SkinMesh,
        view: &Matrix4,
        combined: impl Fn(u32) -> Matrix4,
    ) -> BonePalette {
        let mut palette = BonePalette::default();

        for (slot, (offset, &bone)) in mesh
            .bone_offsets
            .iter()
            .zip(mesh.bone_indices.iter())
            .take(MAX_BONE_PALETTE)
            .enumerate()
        {
            let skin = Matrix4::multiply(&Matrix4(*offset), &combined(bone));

            palette.bones[slot] = Matrix4::multiply(&skin, view).shader_constants();
        }

        palette
    }

    pub fn outline(
        mesh: &SkinMesh,
        view: &Matrix4,
        eye: glam::Vec3,
        combined: impl Fn(u32) -> Matrix4,
    ) -> BonePalette {
        let origin = mesh
            .bone_indices
            .first()
            .map(|&bone| combined(bone).transform_coord(glam::Vec3::ZERO))
            .unwrap_or(eye);

        let delta = origin - eye;

        let scale = 1.0 + (delta.length() * 0.00002).max(0.02);

        let expand = Matrix4::scaling(scale, scale, scale);

        let shift = delta.normalize_or_zero() * 0.1;

        let shifted_view =
            Matrix4::multiply(&Matrix4::translation(shift.x, shift.y, shift.z), view);

        let mut palette = BonePalette::default();

        for (slot, (offset, &bone)) in mesh
            .bone_offsets
            .iter()
            .zip(&mesh.bone_indices)
            .take(MAX_BONE_PALETTE)
            .enumerate()
        {
            let expanded = Matrix4::multiply(&Matrix4(*offset), &expand);

            let world = Matrix4::multiply(&expanded, &combined(bone));

            palette.bones[slot] = Matrix4::multiply(&world, &shifted_view).shader_constants();
        }

        palette
    }
}

pub fn grass_enabled(vendor: VendorClass) -> bool {
    vendor != VendorClass::NvidiaMx
}

pub fn skin_light_direction() -> [f32; 4] {
    let n = 1.0 / 3.0f32.sqrt();

    [-n, n, n, 0.0]
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct SkinFrame {
    pub projection: [f32; 16],
    pub view_inverse: [f32; 16],
    pub light_direction: [f32; 4],
    pub fog_color: [f32; 4],
    pub scroll: [f32; 4],
}

impl SkinFrame {
    pub fn new(projection: &Matrix4, view: &Matrix4, fog_color: [f32; 4]) -> SkinFrame {
        let inverse = crate::mesh::normal_matrix(view).transpose();

        SkinFrame {
            projection: projection.shader_constants(),
            view_inverse: inverse.shader_constants(),
            light_direction: skin_light_direction(),
            fog_color,
            scroll: [0.0; 4],
        }
    }

    pub fn with_scroll(mut self, now_ms: u32) -> SkinFrame {
        self.scroll[0] = (now_ms % glow::SCROLL_PERIOD_MS) as f32 / glow::SCROLL_PERIOD_MS as f32;

        self
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct SkinObject {
    pub material_diffuse: [f32; 4],
    pub material_ambient_emissive: [f32; 4],
    pub fog_params: [f32; 4],
    pub flags: [f32; 4],
    pub stage: [f32; 4],
    pub glow: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinFog {
    pub start: f32,
    pub end: f32,
    pub enabled: bool,
}

impl SkinFog {
    pub fn on(start: f32, end: f32) -> SkinFog {
        SkinFog {
            start,
            end,
            enabled: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinParams {
    pub diffuse: [f32; 4],
    pub ambient: [f32; 4],
    pub emissive: [f32; 4],
    pub unlit: bool,
    pub fog_start: f32,
    pub fog_end: f32,
    pub fog_enabled: bool,
    pub force_opaque: bool,
    pub modulate_alpha: bool,
    pub glow: Option<glow::Glow>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GlowMode {
    Off = 0,
    Modulate = 1,
    AddSmooth = 2,
    ModulateAlphaAddColor = 3,
    DotProduct3 = 4,
    AddSigned = 5,
}

impl GlowMode {
    pub fn of(blend: glow::Blend) -> GlowMode {
        match blend {
            glow::Blend::Modulate => GlowMode::Modulate,
            glow::Blend::AddSmooth => GlowMode::AddSmooth,
            glow::Blend::ModulateAlphaAddColor => GlowMode::ModulateAlphaAddColor,
            glow::Blend::DotProduct3 => GlowMode::DotProduct3,
            glow::Blend::AddSigned => GlowMode::AddSigned,
        }
    }
}

pub fn emissive_material(params: &SkinParams) -> SkinParams {
    let (diffuse_ceiling, emissive_floor, diffuse_alpha) = glow::EMISSIVE_MATERIAL;

    let mut out = *params;

    for channel in 0..3 {
        out.diffuse[channel] = params.diffuse[channel].min(diffuse_ceiling);

        out.emissive[channel] = params.emissive[channel].max(emissive_floor);
    }

    out.diffuse[3] = diffuse_alpha;

    out.emissive[3] = diffuse_alpha;

    out
}

impl SkinParams {
    pub fn effective_fog_end(&self) -> f32 {
        if self.unlit {
            self.fog_end * 1.4
        } else {
            self.fog_end
        }
    }
}

impl SkinObject {
    pub fn new(params: &SkinParams, palette_len: u32, alpha_reference: f32) -> SkinObject {
        let end = params.effective_fog_end();

        let range = end - params.fog_start;

        SkinObject {
            material_diffuse: params.diffuse,
            material_ambient_emissive: [
                params.ambient[0] * 0.25 + params.emissive[0],
                params.ambient[1] * 0.25 + params.emissive[1],
                params.ambient[2] * 0.25 + params.emissive[2],
                params.ambient[3] + params.emissive[3],
            ],
            fog_params: [
                1.0,
                end,
                if range != 0.0 { 1.0 / range } else { 0.0 },
                f32::from(u8::from(!params.fog_enabled)),
            ],
            flags: [
                f32::from(u8::from(params.unlit)),
                alpha_reference,
                palette_len.clamp(1, MAX_BONE_PALETTE as u32) as f32,
                f32::from(u8::from(params.force_opaque)),
            ],
            stage: [f32::from(u8::from(params.modulate_alpha)), 0.0, 0.0, 0.0],
            glow: match params.glow {
                None => [GlowMode::Off as u32 as f32, 0.0, 0.0, 0.0],
                Some(glow) => [
                    GlowMode::of(glow.blend) as u32 as f32,
                    f32::from(u8::from(glow.double)),
                    f32::from(u8::from(glow.uv == glow::Uv::Mesh)),
                    0.0,
                ],
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum SkinPass {
    Opaque,
    Highlight,
    Invisible,
    Blended,
    EffectAlpha,
    EffectAdditive,
}

impl SkinPass {
    pub const ALL: [SkinPass; 6] = [
        SkinPass::Opaque,
        SkinPass::Highlight,
        SkinPass::Invisible,
        SkinPass::Blended,
        SkinPass::EffectAlpha,
        SkinPass::EffectAdditive,
    ];

    pub fn blend_state(self) -> Option<wgpu::BlendState> {
        let (src, dst) = match self {
            SkinPass::Opaque => return None,
            SkinPass::Highlight | SkinPass::Blended | SkinPass::EffectAlpha => {
                (d3d::BLEND_SRCALPHA, d3d::BLEND_INVSRCALPHA)
            }
            SkinPass::Invisible | SkinPass::EffectAdditive => (d3d::BLEND_SRCALPHA, d3d::BLEND_ONE),
        };

        let component = wgpu::BlendComponent {
            src_factor: crate::preset::blend_factor(src),
            dst_factor: crate::preset::blend_factor(dst),
            operation: wgpu::BlendOperation::Add,
        };

        Some(wgpu::BlendState {
            color: component,
            alpha: component,
        })
    }

    pub fn depth_write(self) -> bool {
        match self {
            SkinPass::Opaque | SkinPass::Invisible | SkinPass::EffectAlpha => true,
            SkinPass::Highlight | SkinPass::Blended | SkinPass::EffectAdditive => false,
        }
    }

    pub fn write_mask(self) -> wgpu::ColorWrites {
        match self {
            SkinPass::Opaque
            | SkinPass::Highlight
            | SkinPass::Invisible
            | SkinPass::EffectAlpha
            | SkinPass::EffectAdditive => wgpu::ColorWrites::ALL,
            SkinPass::Blended => wgpu::ColorWrites::COLOR,
        }
    }

    pub fn alpha_reference(self, vendor: VendorClass) -> f32 {
        match self {
            SkinPass::Opaque | SkinPass::Highlight => {
                Preset::lit_3d(vendor).alpha_reference_normalised()
            }
            SkinPass::Invisible
            | SkinPass::Blended
            | SkinPass::EffectAlpha
            | SkinPass::EffectAdditive => 0.0,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SkinPass::Opaque => "skinned mesh",
            SkinPass::Highlight => "skinned mesh (owned hover outline)",
            SkinPass::Invisible => "skinned mesh (invisible self)",
            SkinPass::Blended => "blended skinned mesh",
            SkinPass::EffectAlpha => "skinned effect mesh (blend mode 0)",
            SkinPass::EffectAdditive => "skinned effect mesh (blend modes 1 and 2)",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SkinDraw {
    pub pass: SkinPass,
    pub part: usize,
    pub object: u32,
    pub palette: u32,
    pub texture: Option<usize>,
}

pub struct SkinTexture {
    pub bind_group: wgpu::BindGroup,
}

pub const PALETTE_CAPACITY: u32 = 2048;

pub struct SkinRenderer {
    pipelines: Vec<(SkinPass, bool, wgpu::RenderPipeline)>,
    null_texture: SkinTexture,
    frame_layout: wgpu::BindGroupLayout,
    object_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    glow_sampler: wgpu::Sampler,
    vendor: VendorClass,
    frame_buffer: wgpu::Buffer,
    frame_bind_group: wgpu::BindGroup,
    object_buffer: wgpu::Buffer,
    object_bind_group: wgpu::BindGroup,
    object_stride: u32,
    object_capacity: u32,
    palette_buffer: wgpu::Buffer,
    palette_bind_group: wgpu::BindGroup,
    palette_stride: u32,
    geometry: crate::geometry::GeometryBuffers,
    allocs: Vec<SkinAlloc>,
    scratch: Vec<u8>,
    palette_scratch: Vec<u8>,
    draws: Vec<SkinDraw>,
    drops: SkinDrops,
    motion_history: Option<PaletteHistory>,
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct SkinDrops {
    pub palette_demand: u32,
    pub over_capacity: u32,
    pub unbound_part: u32,
    pub unbound_texture: u32,
}

impl SkinDrops {
    pub fn any(&self) -> bool {
        self.over_capacity | self.unbound_part | self.unbound_texture != 0
    }

    pub fn worse_than(&self, worst: &SkinDrops) -> bool {
        self.over_capacity > worst.over_capacity
            || self.unbound_part > worst.unbound_part
            || self.unbound_texture > worst.unbound_texture
    }
}

impl SkinRenderer {
    pub fn new(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        vendor: VendorClass,
    ) -> SkinRenderer {
        Self::with_temporal(gpu, target_format, depth_format, vendor, None)
    }

    pub fn with_temporal(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        vendor: VendorClass,
        temporal: Option<&crate::temporal::TemporalCamera>,
    ) -> SkinRenderer {
        let palette_size = if temporal.is_some() {
            std::mem::size_of::<TemporalPalette>()
        } else {
            std::mem::size_of::<BonePalette>()
        };

        let preset = Preset::for_mode(RenderMode::Lit3d).expect("mode 1 has a preset");

        let shader = crate::shaders::module(
            &gpu.device,
            if temporal.is_some() {
                crate::shaders::Shader::TemporalSkin
            } else {
                crate::shaders::Shader::Skin
            },
        );

        let frame_layout = uniform_layout(
            gpu,
            "skin frame",
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            std::mem::size_of::<SkinFrame>() as u64,
            false,
        );

        let object_layout = uniform_layout(
            gpu,
            "skin object",
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            std::mem::size_of::<SkinObject>() as u64,
            true,
        );

        let palette_layout = uniform_layout(
            gpu,
            "skin palette",
            wgpu::ShaderStages::VERTEX,
            palette_size as u64,
            true,
        );

        let texture_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: crate::gpu_label("skin texture"),
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
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2,
                                multisampled: false,
                            },
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                    ],
                });

        let layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: crate::gpu_label("skinned mesh"),
                bind_group_layouts: &crate::temporal::layouts(
                    &[
                        Some(&frame_layout),
                        Some(&object_layout),
                        Some(&texture_layout),
                        Some(&palette_layout),
                    ],
                    temporal,
                ),
                immediate_size: 0,
            });

        let pipelines = SkinPass::ALL
            .into_iter()
            .flat_map(|pass| {
                [true, false]
                    .into_iter()
                    .map(move |textured| (pass, textured))
            })
            .filter(|(pass, textured)| *textured || *pass != SkinPass::Highlight)
            .map(|(pass, textured)| {
                let depth = preset
                    .depth_state(depth_format)
                    .map(|d| wgpu::DepthStencilState {
                        depth_write_enabled: Some(pass.depth_write()),
                        ..d
                    });

                let pipeline = gpu
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: crate::gpu_label(pass.label()),
                        layout: Some(&layout),
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: Some("vs"),
                            compilation_options: Default::default(),
                            buffers: &[Some(SkinVertexGpu::LAYOUT)],
                        },
                        primitive: wgpu::PrimitiveState {
                            cull_mode: None,
                            ..preset.primitive_state()
                        },
                        depth_stencil: depth,
                        multisample: wgpu::MultisampleState::default(),
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: Some(if !textured {
                                "fs_untextured"
                            } else if pass == SkinPass::Highlight {
                                "fs_highlight"
                            } else {
                                "fs"
                            }),
                            compilation_options: crate::temporal::fragment_options(
                                temporal.is_some(),
                                pass.blend_state(),
                                pass.depth_write(),
                            ),
                            targets: &crate::temporal::targets(
                                wgpu::ColorTargetState {
                                    format: target_format,
                                    blend: pass.blend_state(),
                                    write_mask: pass.write_mask(),
                                },
                                temporal.is_some(),
                                pass.depth_write(),
                            ),
                        }),
                        multiview_mask: None,
                        cache: None,
                    });

                (pass, textured, pipeline)
            })
            .collect();

        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: crate::gpu_label("skinned mesh (SAMP0)"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..preset.sampler_descriptor()
        });

        let glow_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: crate::gpu_label("skinned mesh (SAMP1)"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            ..preset.sampler_descriptor()
        });

        let frame_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("skin frame"),
            size: std::mem::size_of::<SkinFrame>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: crate::gpu_label("skin frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });

        let alignment = gpu.device.limits().min_uniform_buffer_offset_alignment;

        let object_stride = dynamic_stride(std::mem::size_of::<SkinObject>() as u32, alignment);

        let object_capacity = PALETTE_CAPACITY;

        let (object_buffer, object_bind_group) = dynamic_uniform(
            gpu,
            "skin object",
            &object_layout,
            object_stride,
            object_capacity,
            std::mem::size_of::<SkinObject>() as u64,
        );

        let palette_stride = dynamic_stride(palette_size as u32, alignment);

        let (palette_buffer, palette_bind_group) = dynamic_uniform(
            gpu,
            "skin palette",
            &palette_layout,
            palette_stride,
            PALETTE_CAPACITY,
            palette_size as u64,
        );

        let geometry = crate::geometry::GeometryBuffers::new(
            gpu,
            "skin",
            8 * 1024 * SkinVertexGpu::STRIDE,
            32 * 1024,
        );

        let empty = gpu
            .device
            .create_texture(&wgpu::TextureDescriptor {
                label: crate::gpu_label("disabled skin texture stages"),
                size: wgpu::Extent3d {
                    width: 1,
                    height: 1,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&wgpu::TextureViewDescriptor::default());

        let null_texture = SkinTexture {
            bind_group: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: crate::gpu_label("disabled skin texture stages"),
                layout: &texture_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&empty),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(&empty),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&glow_sampler),
                    },
                ],
            }),
        };

        SkinRenderer {
            pipelines,
            null_texture,
            frame_layout,
            object_layout,
            texture_layout,
            sampler,
            glow_sampler,
            vendor,
            frame_buffer,
            frame_bind_group,
            object_buffer,
            object_bind_group,
            object_stride,
            object_capacity,
            palette_buffer,
            palette_bind_group,
            palette_stride,
            geometry,
            allocs: Vec::new(),
            draws: Vec::new(),
            scratch: Vec::new(),
            palette_scratch: Vec::new(),
            drops: SkinDrops::default(),
            motion_history: temporal.map(|_| PaletteHistory::default()),
        }
    }

    pub fn bind_texture(&self, gpu: &Gpu, view: &wgpu::TextureView) -> SkinTexture {
        self.bind_glow(gpu, view, view)
    }

    pub fn bind_glow(
        &self,
        gpu: &Gpu,
        base: &wgpu::TextureView,
        glow: &wgpu::TextureView,
    ) -> SkinTexture {
        SkinTexture {
            bind_group: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: crate::gpu_label("skin texture"),
                layout: &self.texture_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(base),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&self.sampler),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: wgpu::BindingResource::TextureView(glow),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: wgpu::BindingResource::Sampler(&self.glow_sampler),
                    },
                ],
            }),
        }
    }

    pub fn upload_arena(&mut self, gpu: &Gpu, arena: &SkinArena) {
        self.allocs.clone_from(&arena.allocs);

        self.geometry.upload(
            gpu,
            bytemuck::cast_slice(&arena.vertices),
            &arena.indices,
            false,
        );
    }

    pub fn append_arena(&mut self, gpu: &Gpu, arena: &SkinArena) {
        self.append_staged_arena(gpu, arena, &[]);
    }

    pub fn append_staged_arena(
        &mut self,
        gpu: &Gpu,
        arena: &SkinArena,
        copies: &[crate::geometry::VertexCopy<'_>],
    ) {
        if arena.allocs.len() < self.allocs.len() {
            self.allocs.clear();
        }

        self.allocs
            .extend_from_slice(&arena.allocs[self.allocs.len()..]);

        self.geometry.upload_copies(
            gpu,
            bytemuck::cast_slice(&arena.vertices),
            &arena.indices,
            true,
            copies,
        );
    }

    pub fn geometry_upload_stats(&self) -> crate::geometry::GeometryUploadStats {
        self.geometry.stats()
    }

    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        frame: &SkinFrame,
        objects: &[SkinObject],
        palettes: &[BonePalette],
        draws: &[SkinDraw],
        bound_textures: usize,
    ) {
        self.prepare_with_history(gpu, frame, objects, palettes, draws, bound_textures, &[]);
    }

    pub fn reset_motion_history(&mut self) {
        if let Some(history) = &mut self.motion_history {
            history.previous.clear();
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_with_history(
        &mut self,
        gpu: &Gpu,
        frame: &SkinFrame,
        objects: &[SkinObject],
        palettes: &[BonePalette],
        draws: &[SkinDraw],
        bound_textures: usize,
        keys: &[Option<MotionKey>],
    ) {
        gpu.queue
            .write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(frame));

        self.grow_objects(gpu, objects.len() as u32);

        pack::upload(
            gpu,
            &self.object_buffer,
            &mut self.scratch,
            self.object_stride,
            objects,
        );

        let capped = &palettes[..palettes.len().min(PALETTE_CAPACITY as usize)];

        if let Some(history) = &mut self.motion_history {
            let blocks = { history.pack(capped, keys, frame.projection) };

            pack::upload(
                gpu,
                &self.palette_buffer,
                &mut self.palette_scratch,
                self.palette_stride,
                &blocks,
            );
        } else {
            pack::upload(
                gpu,
                &self.palette_buffer,
                &mut self.palette_scratch,
                self.palette_stride,
                capped,
            );
        }

        self.draws.clear();

        self.draws.extend(
            draws
                .iter()
                .copied()
                .filter(|d| d.palette < PALETTE_CAPACITY && (d.object) < self.object_capacity),
        );

        let count =
            |f: &dyn Fn(&SkinDraw) -> bool| self.draws.iter().filter(|d| f(d)).count() as u32;

        self.drops = SkinDrops {
            palette_demand: palettes.len() as u32,
            over_capacity: (draws.len() - self.draws.len()) as u32,
            unbound_part: count(&|d| d.part >= self.allocs.len()),
            unbound_texture: count(&|d| d.texture.is_some_and(|index| index >= bound_textures)),
        };

        self.draws.sort_by_key(|d| d.pass);
    }

    pub fn drops(&self) -> SkinDrops {
        self.drops
    }

    pub fn draw(&self, render: &mut wgpu::RenderPass<'_>, textures: &[SkinTexture]) {
        self.draw_passes(render, textures, &SkinPass::ALL);
    }

    pub fn draw_passes(
        &self,
        render: &mut wgpu::RenderPass<'_>,
        textures: &[SkinTexture],
        kinds: &[SkinPass],
    ) {
        if self.draws.is_empty() {
            return;
        }

        render.set_vertex_buffer(0, self.geometry.vertices.buffer.slice(..));

        render.set_index_buffer(
            self.geometry.indices.buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );

        render.set_bind_group(0, &self.frame_bind_group, &[]);

        let mut bound = None;

        let mut bindings = DrawBindings::default();

        for draw in self.draws.iter().filter(|draw| kinds.contains(&draw.pass)) {
            let texture = match draw.texture {
                Some(index) => textures.get(index),
                None => Some(&self.null_texture),
            };

            let (Some(alloc), Some(texture)) = (self.allocs.get(draw.part), texture) else {
                continue;
            };

            if alloc.index_count == 0 {
                continue;
            }

            let key = (draw.pass, draw.texture.is_some());

            if bound != Some(key) {
                let Some((_, _, pipeline)) =
                    self.pipelines.iter().find(|(p, t, _)| (*p, *t) == key)
                else {
                    continue;
                };

                render.set_pipeline(pipeline);

                bound = Some(key);
            }

            let [object_changed, texture_changed, palette_changed] = bindings.update(draw);

            if object_changed {
                render.set_bind_group(
                    1,
                    &self.object_bind_group,
                    &[draw.object * self.object_stride],
                );
            }

            if texture_changed {
                render.set_bind_group(2, &texture.bind_group, &[]);
            }

            if palette_changed {
                render.set_bind_group(
                    3,
                    &self.palette_bind_group,
                    &[draw.palette * self.palette_stride],
                );
            }

            render.draw_indexed(alloc.range(), alloc.base_vertex, 0..1);
        }
    }

    pub fn draw_count(&self) -> usize {
        self.draws.len()
    }

    pub fn vendor(&self) -> VendorClass {
        self.vendor
    }

    pub fn alpha_reference(&self) -> f32 {
        SkinPass::Opaque.alpha_reference(self.vendor)
    }

    pub fn grass_enabled(&self) -> bool {
        grass_enabled(self.vendor)
    }

    pub fn palette_bytes(&self) -> u64 {
        u64::from(self.palette_stride) * u64::from(PALETTE_CAPACITY)
    }

    pub fn frame_layout(&self) -> &wgpu::BindGroupLayout {
        &self.frame_layout
    }

    fn grow_objects(&mut self, gpu: &Gpu, count: u32) {
        if count <= self.object_capacity {
            return;
        }

        self.object_capacity = count.next_power_of_two();

        let (buffer, bind_group) = dynamic_uniform(
            gpu,
            "skin object",
            &self.object_layout,
            self.object_stride,
            self.object_capacity,
            std::mem::size_of::<SkinObject>() as u64,
        );

        self.object_buffer = buffer;

        self.object_bind_group = bind_group;
    }
}

#[derive(Default)]
struct DrawBindings {
    object: Option<u32>,
    texture: Option<Option<usize>>,
    palette: Option<u32>,
}

impl DrawBindings {
    fn update(&mut self, draw: &SkinDraw) -> [bool; 3] {
        let changed = [
            self.object != Some(draw.object),
            self.texture != Some(draw.texture),
            self.palette != Some(draw.palette),
        ];

        self.object = Some(draw.object);

        self.texture = Some(draw.texture);

        self.palette = Some(draw.palette);

        changed
    }
}

fn uniform_layout(
    gpu: &Gpu,
    label: &str,
    visibility: wgpu::ShaderStages,
    size: u64,
    dynamic: bool,
) -> wgpu::BindGroupLayout {
    gpu.device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: crate::gpu_label(label),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: dynamic,
                    min_binding_size: wgpu::BufferSize::new(size),
                },
                count: None,
            }],
        })
}

fn dynamic_uniform(
    gpu: &Gpu,
    label: &str,
    layout: &wgpu::BindGroupLayout,
    stride: u32,
    capacity: u32,
    binding_size: u64,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: crate::gpu_label(label),
        size: u64::from(stride) * u64::from(capacity),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: crate::gpu_label(label),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(binding_size),
            }),
        }],
    });

    (buffer, bind_group)
}

fn dynamic_stride(size: u32, alignment: u32) -> u32 {
    size.max(alignment).next_multiple_of(alignment.max(1))
}

pub const SKIN_WGSL: &str = include_str!("shaders/skin.wgsl");
