use crate::camera::Matrix4;
use crate::device::Gpu;
use crate::pack;
use crate::preset::{ColorOp, Preset, RenderMode, d3d};
use crate::quirk::VendorClass;
use crate::world::{FrameUniforms, Material};
use bytemuck::{Pod, Zeroable};
use formats::mesh::StaticVertex;
use formats::mesh::msa::StaticMesh;

#[cfg(test)]
mod neural_tests;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod sky_tests;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct MeshVertex {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv: [f32; 2],
    pub diffuse: u32,
}

impl MeshVertex {
    pub const STRIDE: u64 = 36;

    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: MeshVertex::STRIDE,
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
                format: wgpu::VertexFormat::Unorm8x4,
                offset: 32,
                shader_location: 3,
            },
        ],
    };

    pub fn from_static(v: &StaticVertex) -> MeshVertex {
        MeshVertex {
            position: v.position,
            normal: v.normal,
            uv: v.uv,
            diffuse: v.color,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SubsetRange {
    pub start_index: u32,
    pub index_count: u32,
}

impl SubsetRange {
    pub fn range(&self) -> std::ops::Range<u32> {
        self.start_index..self.start_index + self.index_count
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshAlloc {
    pub base_vertex: i32,
    pub subsets: Vec<SubsetRange>,
    pub vertex_count: u32,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct MeshArena {
    pub vertices: Vec<MeshVertex>,
    pub indices: Vec<u16>,
    pub allocs: Vec<MeshAlloc>,
}

impl MeshArena {
    pub fn push(&mut self, mesh: &StaticMesh) -> usize {
        let base_vertex = self.vertices.len() as i32;

        let base_index = self.indices.len() as u32;

        self.vertices
            .extend(mesh.vertices.iter().map(MeshVertex::from_static));

        self.indices.extend_from_slice(&mesh.indices);

        let available = mesh.indices.len() as u32;

        let subsets = mesh
            .subsets
            .iter()
            .filter_map(|s| {
                let start = s.start_index();

                let count = s.prim_count.checked_mul(3)?;

                (start.checked_add(count)? <= available).then_some(SubsetRange {
                    start_index: base_index + start,
                    index_count: count,
                })
            })
            .collect();

        self.allocs.push(MeshAlloc {
            base_vertex,
            subsets,
            vertex_count: mesh.vertices.len() as u32,
        });

        self.allocs.len() - 1
    }

    pub fn len(&self) -> usize {
        self.allocs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.allocs.is_empty()
    }

    pub fn set_diffuse(&mut self, alloc: usize, color: u32) {
        let Some(a) = self.allocs.get(alloc) else {
            return;
        };

        let start = a.base_vertex as usize;

        for v in &mut self.vertices[start..start + a.vertex_count as usize] {
            v.diffuse = color;
        }
    }

    pub fn triangle_count(&self) -> u32 {
        self.allocs
            .iter()
            .flat_map(|a| a.subsets.iter())
            .map(|s| s.index_count / 3)
            .sum()
    }
}

impl Material {
    pub const SKIN_PART: Material = Material {
        diffuse: [1.0, 1.0, 1.0, 0.0],
        ambient: [0.0, 0.0, 0.0, 0.0],
        specular: [1.0, 1.0, 1.0, 0.0],
        emissive: [0.3, 0.3, 0.3, 0.0],
        power: 0.0,
    };

    pub const STATIC_OBJECT: Material = Material {
        diffuse: [0.7, 0.7, 0.7, 1.0],
        ambient: [0.0, 0.0, 0.0, 0.0],
        specular: [0.7, 0.7, 0.7, 1.0],
        emissive: [0.3, 0.3, 0.3, 1.0],
        power: 0.0,
    };

    pub const SKY: Material = Material {
        diffuse: [0.7, 0.7, 0.7, 0.0],
        ambient: [0.0, 0.0, 0.0, 0.0],
        specular: [0.7, 0.7, 0.7, 0.0],
        emissive: [0.3, 0.3, 0.3, 0.0],
        power: 0.0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum MeshPass {
    Opaque,
    Cutout,
    Sky,
    Blended,
    Additive,
    SolidEffect,
    Silhouette,
    AdditiveSolid,
    Reflective,
}

pub const ALPHAREF_MESH_ALPHA_32BPP: u32 = 0xAA;

impl MeshPass {
    pub fn blend_state(self) -> Option<wgpu::BlendState> {
        let (src, dst) = match self {
            MeshPass::Opaque => return None,
            MeshPass::Cutout => (d3d::BLEND_SRCALPHA, d3d::BLEND_INVSRCALPHA),
            MeshPass::Sky | MeshPass::SolidEffect => (d3d::BLEND_ONE, d3d::BLEND_INVSRCALPHA),
            MeshPass::Blended => (d3d::BLEND_SRCALPHA, d3d::BLEND_INVSRCALPHA),
            MeshPass::Additive | MeshPass::AdditiveSolid => (d3d::BLEND_SRCALPHA, d3d::BLEND_ONE),
            MeshPass::Silhouette => (d3d::BLEND_SRCCOLOR, d3d::BLEND_DESTCOLOR),
            MeshPass::Reflective => (d3d::BLEND_SRCCOLOR, d3d::BLEND_DESTALPHA),
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

    pub fn alpha_reference(self, vendor: VendorClass) -> f32 {
        match self {
            MeshPass::Cutout => ALPHAREF_MESH_ALPHA_32BPP as f32 / 255.0,
            MeshPass::Blended | MeshPass::Additive | MeshPass::Reflective => 0.0,
            MeshPass::Opaque
            | MeshPass::Sky
            | MeshPass::SolidEffect
            | MeshPass::Silhouette
            | MeshPass::AdditiveSolid => Preset::lit_3d(vendor).alpha_reference_normalised(),
        }
    }

    fn label(self) -> &'static str {
        match self {
            MeshPass::Opaque => "static mesh (opaque)",
            MeshPass::Cutout => "static mesh (alpha)",
            MeshPass::Sky => "sky dome",
            MeshPass::Blended => "mesh effect (blended)",
            MeshPass::Additive => "mesh effect (additive)",
            MeshPass::SolidEffect => "mesh effect (depth-writing)",
            MeshPass::Silhouette => "held silhouette",
            MeshPass::AdditiveSolid => "ground flag (additive depth-writing)",
            MeshPass::Reflective => "house reflective surface",
        }
    }

    pub fn depth_state(
        self,
        preset: &Preset,
        format: wgpu::TextureFormat,
    ) -> Option<wgpu::DepthStencilState> {
        let mut state = preset.depth_state(format)?;

        if matches!(self, MeshPass::Sky | MeshPass::Blended | MeshPass::Additive) {
            state.depth_write_enabled = Some(false);
        }

        if self == MeshPass::Sky {
            state.depth_compare = Some(wgpu::CompareFunction::LessEqual);
        }

        Some(state)
    }

    pub const ALL: [MeshPass; 9] = [
        MeshPass::Opaque,
        MeshPass::Sky,
        MeshPass::Cutout,
        MeshPass::SolidEffect,
        MeshPass::Silhouette,
        MeshPass::AdditiveSolid,
        MeshPass::Reflective,
        MeshPass::Blended,
        MeshPass::Additive,
    ];
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct ObjectUniforms {
    pub world: [f32; 16],
    pub normal_matrix: [f32; 16],
    pub material_diffuse: [f32; 4],
    pub material_ambient: [f32; 4],
    pub material_emissive: [f32; 4],
    pub flags: [f32; 4],
    pub alpha: [f32; 4],
    pub uv0: [f32; 4],
    pub uv1: [f32; 4],
    pub refine: [f32; 4],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DrawParams {
    pub material: Material,
    pub lighting: bool,
    pub fog: bool,
    pub color_op: ColorOp,
    pub vertex_color: bool,
    pub force_opaque: bool,
    pub stage1: bool,
}

impl DrawParams {
    pub fn static_object(vertex_color: bool, force_opaque: bool) -> DrawParams {
        DrawParams {
            material: Material::STATIC_OBJECT,
            lighting: true,
            fog: true,
            color_op: ColorOp::Modulate,
            vertex_color,
            force_opaque,
            stage1: false,
        }
    }

    pub fn ground_drop(material: Material, vertex_color: bool, force_opaque: bool) -> DrawParams {
        let color_op = ColorOp::Modulate;

        DrawParams {
            material,
            lighting: true,
            fog: true,
            color_op,
            vertex_color,
            force_opaque,
            stage1: false,
        }
    }

    pub fn held_weapon(vertex_color: bool, force_opaque: bool) -> DrawParams {
        DrawParams {
            material: Material::SKIN_PART,
            lighting: true,
            fog: true,
            color_op: ColorOp::Modulate,
            vertex_color,
            force_opaque,
            stage1: false,
        }
    }

    pub fn held_silhouette(vertex_color: bool, force_opaque: bool) -> DrawParams {
        DrawParams {
            material: Material {
                diffuse: [0.5; 4],
                ambient: [0.5; 4],
                specular: [0.5; 4],
                emissive: [0.5; 4],
                power: 0.0,
            },
            ..Self::held_weapon(vertex_color, force_opaque)
        }
    }

    pub fn glow(tint: [f32; 4]) -> DrawParams {
        DrawParams {
            material: Material {
                diffuse: tint,
                ..Material::STATIC_OBJECT
            },
            lighting: false,
            fog: false,
            color_op: ColorOp::Modulate,
            vertex_color: false,
            force_opaque: false,
            stage1: false,
        }
    }

    pub const EFFECT_TEXTURE_ONLY: DrawParams = DrawParams {
        material: Material::STATIC_OBJECT,
        lighting: false,
        fog: false,
        color_op: ColorOp::SelectTextureRgb,
        vertex_color: false,
        force_opaque: false,
        stage1: false,
    };

    pub fn effect_texture_only() -> DrawParams {
        DrawParams::EFFECT_TEXTURE_ONLY
    }

    pub const SKY: DrawParams = DrawParams {
        material: Material::SKY,
        lighting: false,
        fog: false,
        color_op: ColorOp::AddSigned,
        vertex_color: true,
        force_opaque: true,
        stage1: false,
    };
}

impl ObjectUniforms {
    pub fn set_null_texture(&mut self) {
        self.flags[2] = -1.0;

        self.alpha[1] = 0.0;

        self.alpha[2] = 0.0;
    }

    pub fn set_environment_reflection(&mut self) {
        self.set_stage1(Stage1Blend::AddSmooth, 0.0);

        self.refine[3] = 1.0;
    }

    pub fn set_sky_blend(&mut self, fraction: f32) {
        self.alpha[2] = 7.0;

        self.refine[1] = (fraction.clamp(0.0, 1.0) * 255.0).trunc() / 255.0;
    }

    pub fn set_ui_tint_refinement(&mut self) {
        self.alpha[2] = 8.0;
    }

    pub fn set_stage1(&mut self, blend: Stage1Blend, scroll: f32) {
        self.alpha[2] = match blend {
            Stage1Blend::Modulate => 1.0,
            Stage1Blend::AddSmooth => 2.0,
            Stage1Blend::Modulate2x => 3.0,
            Stage1Blend::ModulateAlphaAddColor => 4.0,
            Stage1Blend::DotProduct3 => 5.0,
            Stage1Blend::AddSigned => 6.0,
        };

        self.alpha[3] = scroll;
    }

    pub fn set_refine(&mut self, glow: formats::mesh::glow::Glow, scroll: f32) {
        use formats::mesh::glow::Blend;

        let blend = match glow.blend {
            Blend::Modulate => Stage1Blend::Modulate,
            Blend::AddSmooth => Stage1Blend::AddSmooth,
            Blend::ModulateAlphaAddColor => Stage1Blend::ModulateAlphaAddColor,
            Blend::DotProduct3 => Stage1Blend::DotProduct3,
            Blend::AddSigned => Stage1Blend::AddSigned,
        };

        self.set_stage1(blend, scroll);

        self.refine[0] = if glow.double { 2.0 } else { 1.0 };

        if glow.emissive {
            for i in 0..3 {
                self.material_diffuse[i] = self.material_diffuse[i].min(0.1);

                self.material_emissive[i] = self.material_emissive[i].max(0.55);
            }

            self.material_diffuse[3] = 0.9;
        }
    }

    pub fn new(world: Matrix4, params: &DrawParams, alpha_reference: f32) -> ObjectUniforms {
        ObjectUniforms {
            world: world.shader_constants(),
            normal_matrix: normal_matrix(&world).shader_constants(),
            material_diffuse: params.material.diffuse,
            material_ambient: params.material.ambient,
            material_emissive: params.material.emissive,
            flags: [
                f32::from(u8::from(params.lighting)),
                f32::from(u8::from(params.fog)),
                params.color_op.uniform(),
                f32::from(u8::from(params.vertex_color)),
            ],
            alpha: [
                alpha_reference,
                f32::from(u8::from(params.force_opaque)),
                f32::from(u8::from(params.stage1)),
                0.0,
            ],
            uv0: [1.0, 1.0, 0.0, 0.0],
            uv1: [1.0, 1.0, 0.0, 0.0],
            refine: [1.0, 0.0, 0.0, 0.0],
        }
    }

    pub fn set_opacity(&mut self, opacity: f32) {
        self.refine[2] = 1.0 - opacity.clamp(0.0, 1.0);

        self.alpha[0] = 0.0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage1Blend {
    Modulate,
    AddSmooth,
    Modulate2x,
    ModulateAlphaAddColor,
    DotProduct3,
    AddSigned,
}

pub fn normal_matrix(world: &Matrix4) -> Matrix4 {
    let m = &world.0;

    let a = [[m[0], m[1], m[2]], [m[4], m[5], m[6]], [m[8], m[9], m[10]]];

    let det = a[0][0] * (a[1][1] * a[2][2] - a[1][2] * a[2][1])
        - a[0][1] * (a[1][0] * a[2][2] - a[1][2] * a[2][0])
        + a[0][2] * (a[1][0] * a[2][1] - a[1][1] * a[2][0]);

    if det.abs() < 1e-12 {
        return Matrix4::IDENTITY;
    }

    let inv_det = 1.0 / det;

    let c = |r: usize, k: usize| {
        let (r0, r1) = ((r + 1) % 3, (r + 2) % 3);

        let (k0, k1) = ((k + 1) % 3, (k + 2) % 3);

        (a[r0][k0] * a[r1][k1] - a[r0][k1] * a[r1][k0]) * inv_det
    };

    Matrix4([
        c(0, 0),
        c(0, 1),
        c(0, 2),
        0.0,
        c(1, 0),
        c(1, 1),
        c(1, 2),
        0.0,
        c(2, 0),
        c(2, 1),
        c(2, 2),
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ])
}

pub const NULL_TEXTURE: usize = usize::MAX;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MeshDraw {
    pub pass: MeshPass,
    pub cull: Cull,
    pub object: u32,
    pub texture: usize,
    pub stage1: Option<usize>,
    pub base_vertex: i32,
    pub subset: SubsetRange,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub enum Cull {
    #[default]
    Ccw,
    Cw,
    None,
}

impl Cull {
    pub fn front_face(self) -> wgpu::FrontFace {
        match self {
            Cull::Ccw | Cull::None => wgpu::FrontFace::Cw,
            Cull::Cw => wgpu::FrontFace::Ccw,
        }
    }

    pub fn cull_mode(self) -> Option<wgpu::Face> {
        match self {
            Cull::Ccw | Cull::Cw => Some(wgpu::Face::Back),
            Cull::None => None,
        }
    }
}

pub const EFFECT_ADDITIVE: u32 = 1;

pub const EFFECT_BLENDED: u32 = 0;

pub const EFFECT_TEXTURE_ONLY_MODE: u32 = 3;

pub fn effect_states(blend: u32, tint: [f32; 4]) -> (MeshPass, Cull, DrawParams) {
    let pass = if blend == EFFECT_ADDITIVE {
        MeshPass::Additive
    } else {
        MeshPass::Blended
    };

    match blend {
        EFFECT_TEXTURE_ONLY_MODE => (pass, Cull::Ccw, DrawParams::EFFECT_TEXTURE_ONLY),
        _ => (pass, Cull::None, DrawParams::glow(tint)),
    }
}

pub struct MeshTexture {
    pub bind_group: wgpu::BindGroup,
}

pub struct MeshRenderer {
    pipelines: Vec<((MeshPass, Cull), wgpu::RenderPipeline)>,
    frame_layout: wgpu::BindGroupLayout,
    object_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    sampler: wgpu::Sampler,
    null_texture: MeshTexture,
    vendor: VendorClass,
    frame_buffer: wgpu::Buffer,
    frame_bind_group: wgpu::BindGroup,
    object_buffer: wgpu::Buffer,
    object_bind_group: wgpu::BindGroup,
    object_stride: u32,
    object_capacity: u32,
    object_size: u64,
    motion_history: Option<std::collections::HashMap<Vec<u8>, [f32; 16]>>,
    geometry: crate::geometry::GeometryBuffers,
    scratch: Vec<u8>,
    draws: Vec<MeshDraw>,
    built_pipelines: u32,
    reported_unbuilt: bool,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct TemporalObject {
    current: ObjectUniforms,
    previous_clip: [f32; 16],
    history: [f32; 4],
}

fn motion_objects(
    history: &mut std::collections::HashMap<Vec<u8>, [f32; 16]>,
    frame: &FrameUniforms,
    objects: &[ObjectUniforms],
    draws: &[MeshDraw],
    keys: &[Option<crate::skin::MotionKey>],
) -> Vec<TemporalObject> {
    let view_projection =
        glam::Mat4::from_cols_array(&frame.view) * glam::Mat4::from_cols_array(&frame.projection);

    let mut identities: Vec<_> = objects
        .iter()
        .enumerate()
        .map(|(index, object)| {
            if let Some(key) = keys.get(index).copied().flatten() {
                let mut bytes = vec![1];

                bytes.extend_from_slice(&key.0.to_le_bytes());

                bytes.extend_from_slice(&key.1.to_le_bytes());

                bytes
            } else {
                let mut bytes = vec![0];

                bytes.extend_from_slice(bytemuck::bytes_of(object));

                bytes
            }
        })
        .collect();

    let mut preservation = vec![0.0_f32; objects.len()];

    for draw in draws {
        if let Some(identity) = identities.get_mut(draw.object as usize) {
            identity.extend_from_slice(&draw.base_vertex.to_le_bytes());

            identity.extend_from_slice(&draw.subset.start_index.to_le_bytes());

            identity.extend_from_slice(&draw.subset.index_count.to_le_bytes());

            identity.extend_from_slice(&(draw.texture as u64).to_le_bytes());

            identity.extend_from_slice(&(draw.stage1.unwrap_or(usize::MAX) as u64).to_le_bytes());

            let weight = match draw.pass {
                MeshPass::Opaque => 0.0,
                MeshPass::Cutout => 0.25,
                _ => 1.0,
            };
            preservation[draw.object as usize] = preservation[draw.object as usize].max(weight);
        }
    }

    let mut next = std::collections::HashMap::with_capacity(objects.len());

    let result = objects
        .iter()
        .zip(identities)
        .zip(preservation)
        .map(|((object, key), preservation)| {
            let clip =
                (glam::Mat4::from_cols_array(&object.world) * view_projection).to_cols_array();

            let previous = history.get(&key).copied();

            next.insert(key, clip);

            TemporalObject {
                current: *object,
                previous_clip: previous.unwrap_or(clip),
                history: [
                    if previous.is_none() {
                        1.0
                    } else {
                        preservation
                    },
                    0.0,
                    0.0,
                    0.0,
                ],
            }
        })
        .collect();

    *history = next;

    result
}

fn pipeline_bit(pass: MeshPass, cull: Cull) -> u32 {
    let p = match pass {
        MeshPass::Opaque => 0,
        MeshPass::Cutout => 1,
        MeshPass::Sky => 2,
        MeshPass::Blended => 3,
        MeshPass::Additive => 4,
        MeshPass::SolidEffect => 5,
        MeshPass::Silhouette => 6,
        MeshPass::AdditiveSolid => 7,
        MeshPass::Reflective => 8,
    };

    let c = match cull {
        Cull::Ccw => 0,
        Cull::Cw => 1,
        Cull::None => 2,
    };

    1 << (p * 3 + c)
}

impl MeshRenderer {
    pub const PIPELINES: [(MeshPass, Cull); 13] = [
        (MeshPass::Opaque, Cull::Ccw),
        (MeshPass::Opaque, Cull::Cw),
        (MeshPass::Opaque, Cull::None),
        (MeshPass::Cutout, Cull::Ccw),
        (MeshPass::Cutout, Cull::Cw),
        (MeshPass::Sky, Cull::Ccw),
        (MeshPass::Blended, Cull::None),
        (MeshPass::Blended, Cull::Ccw),
        (MeshPass::Additive, Cull::None),
        (MeshPass::SolidEffect, Cull::Ccw),
        (MeshPass::Silhouette, Cull::Cw),
        (MeshPass::AdditiveSolid, Cull::Ccw),
        (MeshPass::Reflective, Cull::None),
    ];

    pub fn new(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        vendor: VendorClass,
    ) -> MeshRenderer {
        Self::with_depth(gpu, target_format, Some(depth_format), vendor, None)
    }

    pub fn with_temporal(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        vendor: VendorClass,
        temporal: Option<&crate::temporal::TemporalCamera>,
    ) -> Self {
        Self::with_depth(gpu, target_format, Some(depth_format), vendor, temporal)
    }

    pub fn new_ui(gpu: &Gpu, target_format: wgpu::TextureFormat, vendor: VendorClass) -> Self {
        Self::with_depth(gpu, target_format, None, vendor, None)
    }

    fn with_depth(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        depth_format: Option<wgpu::TextureFormat>,
        vendor: VendorClass,
        temporal: Option<&crate::temporal::TemporalCamera>,
    ) -> Self {
        let object_size = if temporal.is_some() {
            std::mem::size_of::<TemporalObject>()
        } else {
            std::mem::size_of::<ObjectUniforms>()
        } as u64;

        let preset = Preset::for_mode(RenderMode::Lit3d).expect("mode 1 has a preset");

        let shader = crate::shaders::module(
            &gpu.device,
            if temporal.is_some() {
                crate::shaders::Shader::TemporalMesh
            } else {
                crate::shaders::Shader::Mesh
            },
        );

        let frame_layout = uniform_layout(
            gpu,
            "mesh frame",
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            std::mem::size_of::<FrameUniforms>() as u64,
            false,
        );

        let object_layout = uniform_layout(
            gpu,
            "mesh object",
            wgpu::ShaderStages::VERTEX_FRAGMENT,
            object_size,
            true,
        );

        let texture_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: crate::gpu_label("mesh texture"),
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
                label: crate::gpu_label("static mesh"),
                bind_group_layouts: &crate::temporal::layouts(
                    &[
                        Some(&frame_layout),
                        Some(&object_layout),
                        Some(&texture_layout),
                        Some(&texture_layout),
                    ],
                    temporal,
                ),
                immediate_size: 0,
            });

        let pipelines = MeshRenderer::PIPELINES
            .into_iter()
            .map(|(pass, cull)| {
                let pipeline = gpu
                    .device
                    .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                        label: crate::gpu_label(pass.label()),
                        layout: Some(&layout),
                        vertex: wgpu::VertexState {
                            module: &shader,
                            entry_point: Some("vs"),
                            compilation_options: wgpu::PipelineCompilationOptions {
                                constants: &[("sky_background", f64::from(pass == MeshPass::Sky))],
                                ..Default::default()
                            },
                            buffers: &[Some(MeshVertex::LAYOUT)],
                        },
                        primitive: wgpu::PrimitiveState {
                            front_face: cull.front_face(),
                            cull_mode: cull.cull_mode(),
                            ..preset.primitive_state()
                        },
                        depth_stencil: depth_format
                            .and_then(|format| pass.depth_state(&preset, format)),
                        multisample: wgpu::MultisampleState::default(),
                        fragment: Some(wgpu::FragmentState {
                            module: &shader,
                            entry_point: Some("fs"),
                            compilation_options: crate::temporal::fragment_options(
                                temporal.is_some(),
                                pass.blend_state(),
                                !matches!(pass, MeshPass::Blended | MeshPass::Additive),
                            ),
                            targets: &crate::temporal::targets(
                                wgpu::ColorTargetState {
                                    format: target_format,
                                    blend: pass.blend_state(),
                                    write_mask: wgpu::ColorWrites::ALL,
                                },
                                temporal.is_some(),
                                !matches!(pass, MeshPass::Blended | MeshPass::Additive),
                            ),
                        }),
                        multiview_mask: None,
                        cache: None,
                    });

                ((pass, cull), pipeline)
            })
            .collect();

        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: crate::gpu_label("static mesh (SAMP0)"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..preset.material_sampler_descriptor(gpu.adapter.get_downlevel_capabilities().flags)
        });

        let null_texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: crate::gpu_label("mesh NULL binding"),
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
        });

        let null_view = null_texture.create_view(&Default::default());

        let null_texture = MeshTexture {
            bind_group: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: crate::gpu_label("mesh NULL binding"),
                layout: &texture_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&null_view),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::Sampler(&sampler),
                    },
                ],
            }),
        };

        let frame_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("mesh frame"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: crate::gpu_label("mesh frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });

        let object_stride = (object_size as u32).next_multiple_of(
            gpu.device
                .limits()
                .min_uniform_buffer_offset_alignment
                .max(1),
        );

        let object_capacity = 512;

        let (object_buffer, object_bind_group) = object_storage(
            gpu,
            &object_layout,
            object_stride,
            object_capacity,
            object_size,
        );

        let geometry = crate::geometry::GeometryBuffers::new(
            gpu,
            "mesh",
            16 * 1024 * MeshVertex::STRIDE,
            64 * 1024,
        );

        MeshRenderer {
            pipelines,
            frame_layout,
            object_layout,
            texture_layout,
            sampler,
            null_texture,
            vendor,
            frame_buffer,
            frame_bind_group,
            object_buffer,
            object_bind_group,
            object_stride,
            object_capacity,
            object_size,
            motion_history: temporal.map(|_| std::collections::HashMap::new()),
            geometry,
            draws: Vec::new(),
            scratch: Vec::new(),
            built_pipelines: MeshRenderer::PIPELINES
                .into_iter()
                .fold(0, |m, (p, c)| m | pipeline_bit(p, c)),
            reported_unbuilt: false,
        }
    }

    pub fn bind_texture(&self, gpu: &Gpu, view: &wgpu::TextureView) -> MeshTexture {
        MeshTexture {
            bind_group: gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: crate::gpu_label("mesh texture"),
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

    pub fn upload_arena(&mut self, gpu: &Gpu, arena: &MeshArena) {
        self.geometry.upload(
            gpu,
            bytemuck::cast_slice(&arena.vertices),
            &arena.indices,
            false,
        );
    }

    pub fn append_arena(&mut self, gpu: &Gpu, arena: &MeshArena) {
        self.append_staged_arena(gpu, arena, &[]);
    }

    pub fn append_staged_arena(
        &mut self,
        gpu: &Gpu,
        arena: &MeshArena,
        copies: &[crate::geometry::VertexCopy<'_>],
    ) {
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
        uniforms: &FrameUniforms,
        objects: &[ObjectUniforms],
        draws: &[MeshDraw],
    ) {
        self.prepare_with_history(gpu, uniforms, objects, draws, &[]);
    }

    pub fn reset_motion_history(&mut self) {
        if let Some(history) = &mut self.motion_history {
            history.clear();
        }
    }

    pub fn prepare_with_history(
        &mut self,
        gpu: &Gpu,
        uniforms: &FrameUniforms,
        objects: &[ObjectUniforms],
        draws: &[MeshDraw],
        keys: &[Option<crate::skin::MotionKey>],
    ) {
        gpu.queue
            .write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(uniforms));

        self.grow_objects(gpu, objects.len() as u32);

        if let Some(history) = &mut self.motion_history {
            let blocks = motion_objects(history, uniforms, objects, draws, keys);

            pack::upload(
                gpu,
                &self.object_buffer,
                &mut self.scratch,
                self.object_stride,
                &blocks,
            );
        } else {
            pack::upload(
                gpu,
                &self.object_buffer,
                &mut self.scratch,
                self.object_stride,
                objects,
            );
        }

        self.draws.clear();

        self.draws.extend_from_slice(draws);

        self.report_unbuilt_pipelines();
    }

    fn report_unbuilt_pipelines(&mut self) {
        if self.reported_unbuilt {
            return;
        }

        let mut wanted = 0u32;

        for draw in &self.draws {
            wanted |= pipeline_bit(draw.pass, draw.cull);
        }

        let missing = wanted & !self.built_pipelines;

        if missing == 0 {
            return;
        }

        for (pass, cull) in MeshPass::ALL
            .into_iter()
            .flat_map(|p| [Cull::Ccw, Cull::Cw, Cull::None].map(|c| (p, c)))
        {
            if missing & pipeline_bit(pass, cull) != 0 {
                tracing::info!(
                    "NWYD: mesh {pass:?} with {cull:?} has no pipeline and its draws are dropped \
                     — add the pair to MeshRenderer::PIPELINES",
                );
            }
        }

        self.reported_unbuilt = true;
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, textures: &[MeshTexture]) {
        self.draw_passes(pass, textures, &MeshPass::ALL);
    }

    pub fn draw_passes(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        textures: &[MeshTexture],
        kinds: &[MeshPass],
    ) {
        self.draw_iter(
            pass,
            textures,
            kinds
                .iter()
                .flat_map(|kind| self.draws.iter().filter(move |draw| draw.pass == *kind)),
        );
    }

    pub fn draw_range(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        textures: &[MeshTexture],
        range: std::ops::Range<usize>,
    ) {
        self.draw_iter(pass, textures, self.draws.get(range).unwrap_or(&[]).iter());
    }

    fn draw_iter<'a>(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        textures: &[MeshTexture],
        draws: impl Iterator<Item = &'a MeshDraw>,
    ) {
        pass.set_vertex_buffer(0, self.geometry.vertices.buffer.slice(..));

        pass.set_index_buffer(
            self.geometry.indices.buffer.slice(..),
            wgpu::IndexFormat::Uint16,
        );

        pass.set_bind_group(0, &self.frame_bind_group, &[]);

        let mut bound: Option<(MeshPass, Cull)> = None;

        for draw in draws {
            let kind = draw.pass;

            let Some(texture) = (if draw.texture == NULL_TEXTURE {
                Some(&self.null_texture)
            } else {
                textures.get(draw.texture)
            }) else {
                continue;
            };

            if bound != Some((kind, draw.cull)) {
                let Some((_, pipeline)) = self
                    .pipelines
                    .iter()
                    .find(|(key, _)| *key == (kind, draw.cull))
                else {
                    continue;
                };

                pass.set_pipeline(pipeline);

                bound = Some((kind, draw.cull));
            }

            pass.set_bind_group(
                1,
                &self.object_bind_group,
                &[draw.object * self.object_stride],
            );

            pass.set_bind_group(2, &texture.bind_group, &[]);

            let stage1 = match draw.stage1 {
                Some(index) => match textures.get(index) {
                    Some(texture) => texture,
                    None => continue,
                },
                None => texture,
            };

            pass.set_bind_group(3, &stage1.bind_group, &[]);

            pass.draw_indexed(draw.subset.range(), draw.base_vertex, 0..1);
        }
    }

    pub fn draw_count(&self) -> usize {
        self.draws.len()
    }

    pub fn vendor(&self) -> VendorClass {
        self.vendor
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
            self.object_size,
        );

        self.object_buffer = buffer;

        self.object_bind_group = bind_group;
    }

    pub fn frame_layout(&self) -> &wgpu::BindGroupLayout {
        &self.frame_layout
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

fn object_storage(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    stride: u32,
    capacity: u32,
    object_size: u64,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: crate::gpu_label("mesh object"),
        size: u64::from(stride) * u64::from(capacity),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: crate::gpu_label("mesh object"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(object_size),
            }),
        }],
    });

    (buffer, bind_group)
}

pub const MESH_WGSL: &str = include_str!("shaders/mesh.wgsl");
