use crate::camera::Matrix4;
use crate::device::Gpu;
use crate::pack;
use crate::preset::{Preset, RenderMode};
use bytemuck::{Pod, Zeroable};
use formats::world::trn::Terrain;
use formats::world::uv::{BASE_UV, DETAIL_UV};
use formats::world::{HEIGHT_SCALE, QUAD_SIZE, TRN_DIM};
use glam::Vec3;
use wgpu::util::DeviceExt;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod render_tests;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct WorldVertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub diffuse: u32,
    pub uv0: [f32; 2],
    pub uv1: [f32; 2],
}

impl WorldVertex {
    pub const STRIDE: u64 = 44;

    pub const LAYOUT: wgpu::VertexBufferLayout<'static> = wgpu::VertexBufferLayout {
        array_stride: WorldVertex::STRIDE,
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
                format: wgpu::VertexFormat::Unorm8x4,
                offset: 24,
                shader_location: 2,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 28,
                shader_location: 3,
            },
            wgpu::VertexAttribute {
                format: wgpu::VertexFormat::Float32x2,
                offset: 36,
                shader_location: 4,
            },
        ],
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TerrainBatch {
    pub base_texture_id: i32,
    pub detail_texture_id: i32,
    pub material: TerrainMaterial,
    pub first_quad: u32,
    pub quad_count: u32,
    pub bounds: crate::cull::Aabb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum TerrainMaterial {
    Base,
    Detail,
    Lava,
    Water,
    Shore,
}

impl TerrainMaterial {
    pub fn needs_detail(self) -> bool {
        matches!(self, Self::Detail | Self::Water | Self::Shore)
    }

    fn multiplier(self) -> f32 {
        match self {
            Self::Detail => 1.0,
            Self::Water | Self::Shore => 2.0,
            _ => 0.0,
        }
    }

    fn scroll(self, now_ms: u32) -> f32 {
        let phase = (now_ms % 10_000) as f32 / 10_000.0;

        match self {
            Self::Lava => 1.0 + phase,
            Self::Water => -1.0 - phase,
            _ => 0.0,
        }
    }
}

pub fn terrain_environment(zone_x: u8, zone_y: u8) -> i32 {
    let (zx, zy) = (i32::from(zone_x), i32::from(zone_y));

    if zy >= 26 {
        return if (9..16).contains(&zx) { 2 } else { 1 };
    }

    if (8..13).contains(&zx) && (11..15).contains(&zy) {
        return 3;
    }

    if (2..11).contains(&zx) && zy < 5 {
        return 4;
    }

    if (26..31).contains(&zx) && (8..13).contains(&zy) {
        5
    } else {
        0
    }
}

impl TerrainBatch {
    pub fn index_range(&self) -> std::ops::Range<u32> {
        let first = self.first_quad * 6;

        first..first + self.quad_count * 6
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TerrainMesh {
    pub vertices: Vec<WorldVertex>,
    pub batches: Vec<TerrainBatch>,
    pub world: Matrix4,
    pub normal_matrix: Matrix4,
    pub zone: (u8, u8),
}

impl TerrainMesh {
    pub fn quad_count(&self) -> u32 {
        self.vertices.len() as u32 / 4
    }

    pub fn triangle_count(&self) -> u32 {
        self.quad_count() * 2
    }

    pub fn base_texture_ids(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self.batches.iter().map(|b| b.base_texture_id).collect();

        v.sort_unstable();

        v.dedup();

        v
    }

    pub fn append_neighbour(&mut self, other: &TerrainMesh, cells_x: i32, cells_z: i32) {
        let first = self.quad_count();

        self.vertices.reserve(other.vertices.len());

        for v in &other.vertices {
            let mut v = *v;

            v.pos[0] += cells_x as f32;

            v.pos[2] += cells_z as f32;

            self.vertices.push(v);
        }

        self.batches
            .extend(other.batches.iter().map(|b| TerrainBatch {
                first_quad: b.first_quad + first,
                bounds: b.bounds.translated(cells_x as f32, 0.0, cells_z as f32),
                ..*b
            }));
    }

    pub fn stitch_boundaries(&mut self) -> Vec<std::ops::Range<usize>> {
        let dimension = TRN_DIM as i32;
        let boundary_key = |vertex: &WorldVertex| {
            let x = vertex.pos[0] as i32;
            let z = vertex.pos[2] as i32;

            (x.rem_euclid(dimension) == 0 || z.rem_euclid(dimension) == 0).then_some((x, z))
        };
        let mut samples: std::collections::BTreeMap<(i32, i32), ((i32, i32), WorldVertex)> =
            std::collections::BTreeMap::new();

        for quad in self.vertices.chunks_exact(4) {
            let owner = (
                (quad[0].pos[0] / TRN_DIM as f32).floor() as i32,
                (quad[0].pos[2] / TRN_DIM as f32).floor() as i32,
            );

            for vertex in quad {
                let Some(key) = boundary_key(vertex) else {
                    continue;
                };

                samples
                    .entry(key)
                    .and_modify(|sample| {
                        if owner > sample.0 {
                            *sample = (owner, *vertex);
                        }
                    })
                    .or_insert((owner, *vertex));
            }
        }

        let mut changed: Vec<std::ops::Range<usize>> = Vec::new();

        for (index, vertex) in self.vertices.iter_mut().enumerate() {
            let Some((_, source)) = boundary_key(vertex).and_then(|key| samples.get(&key)) else {
                continue;
            };

            if vertex.pos[1] == source.pos[1]
                && vertex.normal == source.normal
                && vertex.diffuse == source.diffuse
            {
                continue;
            }

            vertex.pos[1] = source.pos[1];
            vertex.normal = source.normal;
            vertex.diffuse = source.diffuse;

            match changed.last_mut() {
                Some(range) if range.end == index => range.end += 1,
                _ => changed.push(index..index + 1),
            }
        }

        if !changed.is_empty() {
            for batch in &mut self.batches {
                let start = batch.first_quad as usize * 4;
                let end = start + batch.quad_count as usize * 4;

                batch.bounds = crate::cull::Aabb::EMPTY;

                for vertex in &self.vertices[start..end] {
                    batch.bounds.grow(vertex.pos);
                }
            }
        }

        changed
    }

    pub fn detail_texture_ids(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self
            .batches
            .iter()
            .filter(|b| b.material.needs_detail())
            .map(|b| b.detail_texture_id)
            .collect();

        v.sort_unstable();

        v.dedup();

        v
    }
}

pub fn build_terrain(terrain: &Terrain) -> TerrainMesh {
    let environment = terrain_environment(terrain.zone_x, terrain.zone_y);

    let animated = matches!(environment, 1 | 2);

    let heights = terrain.patched_heights();

    let normals = terrain.vertex_normals();

    let last = TRN_DIM - 1;

    let mut buckets: std::collections::BTreeMap<
        (i32, i32, TerrainMaterial),
        Vec<[WorldVertex; 4]>,
    > = std::collections::BTreeMap::new();

    for row in 0..TRN_DIM {
        for col in 0..TRN_DIM {
            let cell = &terrain.cells[row * TRN_DIM + col];

            let next_row = (row + 1).min(last);

            let next_col = (col + 1).min(last);

            let base_uv = BASE_UV[usize::from(cell.uv_mode) % BASE_UV.len()];

            let mut detail_uv = DETAIL_UV[usize::from(cell.uv_mode2) % DETAIL_UV.len()];

            let mut base = cell.base_texture_id();

            let mut detail = cell.detail_texture_id();

            let material = if animated && matches!(base, 170 | 171) {
                TerrainMaterial::Lava
            } else if animated && matches!(base, 38 | 39) {
                base = i32::from(cell.tile0) + 102;

                detail = 344;

                detail_uv = [[0.0, 0.0], [0.0, 1.0], [1.0, 0.0], [1.0, 1.0]];

                TerrainMaterial::Water
            } else if animated && (52..=55).contains(&cell.tile0) {
                base = (base & 1) + 130;

                detail = i32::from(cell.tile0) + 296;

                detail_uv = base_uv;

                TerrainMaterial::Shore
            } else if matches!(environment, 0 | 3 | 4) {
                TerrainMaterial::Detail
            } else {
                TerrainMaterial::Base
            };

            let corners = [
                (col, row),
                (col, next_row),
                (next_col, row),
                (next_col, next_row),
            ];

            let position_x = [col, col, col + 1, col + 1];

            let position_z = [row, next_row.max(row + 1), row, next_row.max(row + 1)];

            let quad = std::array::from_fn(|i| {
                let (cx, cz) = corners[i];

                let source = &terrain.cells[cz * TRN_DIM + cx];

                WorldVertex {
                    pos: [
                        position_x[i] as f32,
                        f32::from(heights[cz * TRN_DIM + cx]),
                        position_z[i] as f32,
                    ],
                    normal: normals[cz * TRN_DIM + cx],
                    diffuse: u32::from_le_bytes(source.color),
                    uv0: base_uv[i],
                    uv1: detail_uv[i],
                }
            });

            buckets
                .entry((base, detail, material))
                .or_default()
                .push(quad);
        }
    }

    let mut vertices = Vec::with_capacity(TRN_DIM * TRN_DIM * 4);

    let mut batches = Vec::with_capacity(buckets.len());

    for ((base_texture_id, detail_texture_id, material), quads) in buckets {
        let first_quad = vertices.len() as u32 / 4;

        let mut bounds = crate::cull::Aabb::EMPTY;

        for quad in &quads {
            vertices.extend_from_slice(quad);

            for v in quad {
                bounds.grow(v.pos);
            }
        }

        batches.push(TerrainBatch {
            base_texture_id,
            detail_texture_id,
            material,
            first_quad,
            quad_count: quads.len() as u32,
            bounds,
        });
    }

    let (origin_x, origin_z) = zone_origin(terrain.zone_x, terrain.zone_y);

    let scale = Matrix4::scaling(QUAD_SIZE, HEIGHT_SCALE, QUAD_SIZE);

    let world = Matrix4::multiply(&scale, &Matrix4::translation(origin_x, 0.0, origin_z));

    let normal_matrix = Matrix4::scaling(1.0 / QUAD_SIZE, 1.0 / HEIGHT_SCALE, 1.0 / QUAD_SIZE);

    TerrainMesh {
        vertices,
        batches,
        world,
        normal_matrix,
        zone: (terrain.zone_x, terrain.zone_y),
    }
}

pub fn zone_origin(zone_x: u8, zone_y: u8) -> (f32, f32) {
    (
        f32::from(zone_x) * QUAD_SIZE * TRN_DIM as f32,
        f32::from(zone_y) * QUAD_SIZE * TRN_DIM as f32,
    )
}

pub fn terrain_indices(quads: u32) -> Vec<u32> {
    let mut out = Vec::with_capacity(quads as usize * 6);

    for q in 0..quads {
        let b = q * 4;

        out.extend_from_slice(&[b, b + 1, b + 2, b + 2, b + 1, b + 3]);
    }

    out
}

pub const LIGHT0_DIRECTION: [f32; 3] = [1.0, -1.0, -1.0];

pub const LIGHT1_DIRECTION: [f32; 3] = [0.0, 0.0, 1.0];

pub const GLOBAL_AMBIENT: u32 = 0x33FF_FFFF;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Material {
    pub diffuse: [f32; 4],
    pub ambient: [f32; 4],
    pub specular: [f32; 4],
    pub emissive: [f32; 4],
    pub power: f32,
}

impl Material {
    pub const TERRAIN: Material = Material {
        diffuse: [1.0, 1.0, 1.0, 0.0],
        ambient: [0.0, 0.0, 0.0, 0.0],
        specular: [1.0, 1.0, 1.0, 0.0],
        emissive: [0.3, 0.3, 0.3, 0.0],
        power: 0.0,
    };
}

pub const MAX_POINT_LIGHTS: usize = 8;

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct PointLight {
    pub position: [f32; 4],
    pub diffuse: [f32; 4],
    pub attenuation: [f32; 4],
}

impl PointLight {
    pub const OFF: PointLight = PointLight {
        position: [0.0; 4],
        diffuse: [0.0; 4],
        attenuation: [0.0; 4],
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PointLights {
    pub slots: [PointLight; MAX_POINT_LIGHTS],
    pub dropped: usize,
}

impl Default for PointLights {
    fn default() -> PointLights {
        PointLights {
            slots: [PointLight::OFF; MAX_POINT_LIGHTS],
            dropped: 0,
        }
    }
}

impl PointLights {
    pub fn set(&mut self, index: u32, light: PointLight) {
        match (index as usize).checked_sub(FIRST_POINT_LIGHT_INDEX) {
            Some(slot) if slot < MAX_POINT_LIGHTS => self.slots[slot] = light,
            _ => self.dropped += 1,
        }
    }

    pub fn enabled(&self) -> usize {
        self.slots
            .iter()
            .filter(|l| l.attenuation[3] != 0.0)
            .count()
    }
}

pub const FIRST_POINT_LIGHT_INDEX: usize = 2;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lighting {
    pub light0: [f32; 4],
    pub light1: [f32; 4],
    pub material: Material,
    pub global_ambient: u32,
    pub point_lights: PointLights,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Fog {
    pub enabled: bool,
    pub color: u32,
    pub start: f32,
    pub end: f32,
}

impl Fog {
    pub fn from_sky(color: u32, start: f32, end: f32, camera_distance: f32) -> Fog {
        Fog {
            enabled: true,
            color,
            start: start + camera_distance - 8.0,
            end: end + camera_distance - 15.0,
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct FrameUniforms {
    pub world: [f32; 16],
    pub view: [f32; 16],
    pub projection: [f32; 16],
    pub normal_matrix: [f32; 16],
    pub light0_color: [f32; 4],
    pub light1_color: [f32; 4],
    pub light0_direction: [f32; 4],
    pub light1_direction: [f32; 4],
    pub material_emissive: [f32; 4],
    pub material_ambient: [f32; 4],
    pub global_ambient: [f32; 4],
    pub fog_color: [f32; 4],
    pub fog_params: [f32; 4],
    pub point_lights: [PointLight; MAX_POINT_LIGHTS],
    pub ground_wave: [f32; 4],
}

impl FrameUniforms {
    pub fn new(
        mesh_world: Matrix4,
        normal_matrix: Matrix4,
        view: Matrix4,
        projection: Matrix4,
        lighting: &Lighting,
        fog: &Fog,
    ) -> FrameUniforms {
        let towards = |d: [f32; 3]| {
            let v = -Vec3::from(d).normalize_or_zero();

            [v.x, v.y, v.z, 0.0]
        };

        let range = fog.end - fog.start;

        FrameUniforms {
            world: mesh_world.shader_constants(),
            view: view.shader_constants(),
            projection: projection.shader_constants(),
            normal_matrix: normal_matrix.shader_constants(),
            light0_color: lighting.light0,
            light1_color: lighting.light1,
            light0_direction: towards(LIGHT0_DIRECTION),
            light1_direction: towards(LIGHT1_DIRECTION),
            material_emissive: lighting.material.emissive,
            material_ambient: lighting.material.ambient,
            global_ambient: argb_to_rgba_f32(lighting.global_ambient),
            fog_color: argb_to_rgba_f32(fog.color),
            fog_params: [
                fog.start,
                fog.end,
                1.0 / if range == 0.0 { 0.01 } else { range },
                if fog.enabled { 1.0 } else { 0.0 },
            ],
            point_lights: lighting.point_lights.slots,
            ground_wave: [0.0; 4],
        }
    }
}

pub fn argb_to_rgba_f32(argb: u32) -> [f32; 4] {
    [
        ((argb >> 16) & 0xFF) as f32 / 255.0,
        ((argb >> 8) & 0xFF) as f32 / 255.0,
        (argb & 0xFF) as f32 / 255.0,
        ((argb >> 24) & 0xFF) as f32 / 255.0,
    ]
}

#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Pod, Zeroable)]
pub struct DrawUniforms {
    pub base_layer: u32,
    pub detail_layer: u32,
    pub stage1_enabled: f32,
    pub scroll: f32,
}

fn detail_multiplier(material: TerrainMaterial, override_enabled: Option<bool>) -> f32 {
    match (material, override_enabled) {
        (TerrainMaterial::Detail, Some(false)) => 0.0,
        _ => material.multiplier(),
    }
}

pub struct TileArray {
    pub bind_group: wgpu::BindGroup,
    pub layers: std::collections::HashMap<i32, u32>,
    groups: std::collections::HashMap<i32, usize>,
    pairs: Vec<wgpu::BindGroup>,
    group_count: usize,
}

mod tile_cache;

pub use tile_cache::{TileCache, TileUploadStats};

impl TileArray {
    pub fn layer(&self, texture_id: i32) -> Option<u32> {
        self.layers.get(&texture_id).copied()
    }

    fn binding(&self, batch: &TerrainBatch) -> (usize, &wgpu::BindGroup) {
        if self.pairs.is_empty() {
            return (0, &self.bind_group);
        }

        let base = self
            .groups
            .get(&batch.base_texture_id)
            .copied()
            .unwrap_or(0);

        let detail = if batch.material.needs_detail() {
            self.groups
                .get(&batch.detail_texture_id)
                .copied()
                .unwrap_or(base)
        } else {
            base
        };

        let pair = base * self.group_count + detail;

        (pair, &self.pairs[pair])
    }
}

pub struct TerrainRenderer {
    pipeline: wgpu::RenderPipeline,
    frame_layout: wgpu::BindGroupLayout,
    texture_layout: wgpu::BindGroupLayout,
    draw_layout: wgpu::BindGroupLayout,
    base_sampler: wgpu::Sampler,
    detail_sampler: wgpu::Sampler,
    frame_buffer: wgpu::Buffer,
    frame_bind_group: wgpu::BindGroup,
    draw_buffer: wgpu::Buffer,
    draw_bind_group: wgpu::BindGroup,
    draw_stride: u32,
    draw_capacity: u32,
    vertices: crate::geometry::GeometryBuffer,
    index_buffer: wgpu::Buffer,
    index_capacity: u32,
    scratch: Vec<u8>,
    draws: Vec<(TerrainBatch, u32)>,
}

impl TerrainRenderer {
    pub fn new(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
    ) -> TerrainRenderer {
        Self::with_temporal(gpu, target_format, depth_format, None)
    }

    pub fn with_temporal(
        gpu: &Gpu,
        target_format: wgpu::TextureFormat,
        depth_format: wgpu::TextureFormat,
        temporal: Option<&crate::temporal::TemporalCamera>,
    ) -> TerrainRenderer {
        let preset = Preset::for_mode(RenderMode::Lit3d).expect("mode 1 has a preset");

        let shader = crate::shaders::module(
            &gpu.device,
            if temporal.is_some() {
                crate::shaders::Shader::TemporalTerrain
            } else {
                crate::shaders::Shader::Terrain
            },
        );

        let frame_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: crate::gpu_label("terrain frame"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<FrameUniforms>() as u64,
                        ),
                    },
                    count: None,
                }],
            });

        let texture_layout =
            gpu.device
                .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                    label: crate::gpu_label("terrain tiles"),
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
                        wgpu::BindGroupLayoutEntry {
                            binding: 2,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                            count: None,
                        },
                        wgpu::BindGroupLayoutEntry {
                            binding: 3,
                            visibility: wgpu::ShaderStages::FRAGMENT,
                            ty: wgpu::BindingType::Texture {
                                sample_type: wgpu::TextureSampleType::Float { filterable: true },
                                view_dimension: wgpu::TextureViewDimension::D2Array,
                                multisampled: false,
                            },
                            count: None,
                        },
                    ],
                });

        let draw_layout = gpu
            .device
            .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: crate::gpu_label("terrain draw"),
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: true,
                        min_binding_size: wgpu::BufferSize::new(
                            std::mem::size_of::<DrawUniforms>() as u64,
                        ),
                    },
                    count: None,
                }],
            });

        let layout = gpu
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: crate::gpu_label("terrain"),
                bind_group_layouts: &crate::temporal::layouts(
                    &[
                        Some(&frame_layout),
                        Some(&texture_layout),
                        Some(&draw_layout),
                    ],
                    temporal,
                ),
                immediate_size: 0,
            });

        let pipeline = gpu
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: crate::gpu_label("mode 1 (lit 3D terrain)"),
                layout: Some(&layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs"),
                    compilation_options: Default::default(),
                    buffers: &[Some(WorldVertex::LAYOUT)],
                },
                primitive: preset.primitive_state(),
                depth_stencil: preset.depth_state(depth_format),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs"),
                    compilation_options: Default::default(),
                    targets: &crate::temporal::targets(
                        wgpu::ColorTargetState {
                            format: target_format,
                            blend: preset.blend_state(),
                            write_mask: wgpu::ColorWrites::ALL,
                        },
                        temporal.is_some(),
                        true,
                    ),
                }),
                multiview_mask: None,
                cache: None,
            });

        let base_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: crate::gpu_label("terrain base (SAMP0)"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..preset.sampler_descriptor()
        });

        let detail_sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: crate::gpu_label("terrain detail (SAMP1)"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            lod_min_clamp: 0.0,
            lod_max_clamp: 0.0,
            ..preset.sampler_descriptor()
        });

        let frame_buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: crate::gpu_label("terrain frame"),
            size: std::mem::size_of::<FrameUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let frame_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: crate::gpu_label("terrain frame"),
            layout: &frame_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: frame_buffer.as_entire_binding(),
            }],
        });

        let draw_stride = draw_stride(gpu.device.limits().min_uniform_buffer_offset_alignment);

        let draw_capacity = 16;

        let (draw_buffer, draw_bind_group) =
            draw_storage(gpu, &draw_layout, draw_stride, draw_capacity);

        let vertices = crate::geometry::GeometryBuffer::new(
            gpu,
            "terrain vertices",
            4 * 1024 * WorldVertex::STRIDE,
            wgpu::BufferUsages::VERTEX,
        );

        let index_capacity = 1024;

        let index_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: crate::gpu_label("terrain indices"),
                contents: bytemuck::cast_slice(&terrain_indices(index_capacity)),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            });

        TerrainRenderer {
            pipeline,
            frame_layout,
            texture_layout,
            draw_layout,
            base_sampler,
            detail_sampler,
            frame_buffer,
            frame_bind_group,
            draw_buffer,
            draw_bind_group,
            draw_stride,
            draw_capacity,
            vertices,
            index_buffer,
            index_capacity,
            draws: Vec::new(),
            scratch: Vec::new(),
        }
    }

    pub fn bind_tiles(
        &self,
        gpu: &Gpu,
        view: &wgpu::TextureView,
        layers: std::collections::HashMap<i32, u32>,
    ) -> TileArray {
        TileArray {
            bind_group: self.bind_tile_pair(gpu, view, view),
            layers,
            groups: Default::default(),
            pairs: Vec::new(),
            group_count: 1,
        }
    }

    fn bind_tile_pair(
        &self,
        gpu: &Gpu,
        base: &wgpu::TextureView,
        detail: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: crate::gpu_label("terrain tiles"),
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
                    resource: wgpu::BindingResource::Sampler(&self.detail_sampler),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::TextureView(detail),
                },
            ],
        })
    }

    pub fn bind_null_tiles(&self, gpu: &Gpu) -> TileArray {
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: crate::gpu_label("terrain null stage binding"),
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

        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(wgpu::TextureViewDimension::D2Array),
            ..Default::default()
        });

        self.bind_tiles(gpu, &view, Default::default())
    }

    pub fn upload_mesh(&mut self, gpu: &Gpu, mesh: &TerrainMesh) {
        self.grow_indices(gpu, mesh.quad_count());

        self.vertices
            .upload(gpu, bytemuck::cast_slice(&mesh.vertices), false);
    }

    pub fn update_mesh(
        &mut self,
        gpu: &Gpu,
        mesh: &TerrainMesh,
        changed: impl Iterator<Item = std::ops::Range<usize>>,
    ) {
        self.grow_indices(gpu, mesh.quad_count());

        self.vertices
            .upload(gpu, bytemuck::cast_slice(&mesh.vertices), true);

        for range in changed {
            self.vertices.patch(
                gpu,
                range.start * WorldVertex::STRIDE as usize,
                bytemuck::cast_slice(&mesh.vertices[range]),
            );
        }
    }

    pub fn vertex_upload_stats(&self) -> crate::geometry::GeometryUploadStats {
        self.vertices.stats()
    }

    pub fn prepare(
        &mut self,
        gpu: &Gpu,
        batches: &[TerrainBatch],
        tiles: &TileArray,
        uniforms: &FrameUniforms,
        stage1_enabled: bool,
    ) {
        self.prepare_frame(gpu, batches, tiles, uniforms, Some(stage1_enabled), 0);
    }

    pub fn prepare_at(
        &mut self,
        gpu: &Gpu,
        batches: &[TerrainBatch],
        tiles: &TileArray,
        uniforms: &FrameUniforms,
        now_ms: u32,
    ) {
        self.prepare_frame(gpu, batches, tiles, uniforms, None, now_ms);
    }

    fn prepare_frame(
        &mut self,
        gpu: &Gpu,
        batches: &[TerrainBatch],
        tiles: &TileArray,
        uniforms: &FrameUniforms,
        detail_override: Option<bool>,
        now_ms: u32,
    ) {
        gpu.queue
            .write_buffer(&self.frame_buffer, 0, bytemuck::bytes_of(uniforms));

        let frustum = crate::cull::Frustum::from_frame(uniforms);

        self.draws.clear();

        for batch in batches {
            let base = tiles.layer(batch.base_texture_id).unwrap_or(0);

            let multiplier = detail_multiplier(batch.material, detail_override);

            let detail = if multiplier > 0.0 {
                tiles.layer(batch.detail_texture_id).unwrap_or(base)
            } else {
                base
            };

            let mut bounds = batch.bounds;

            let amplitude = uniforms.ground_wave[3].abs();

            bounds.min[1] -= amplitude;

            bounds.max[1] += amplitude;

            if !frustum.admits(&bounds) {
                continue;
            }

            self.draws.push((*batch, base | (detail << 16)));
        }

        self.grow_draws(gpu, self.draws.len() as u32);

        if !self.draws.is_empty() {
            let blocks = self.draws.iter().map(|(batch, packed)| DrawUniforms {
                base_layer: packed & 0xFFFF,
                detail_layer: packed >> 16,
                stage1_enabled: if tiles.layer(batch.base_texture_id).is_none() {
                    -1.0
                } else if tiles.layer(batch.detail_texture_id).is_some() {
                    detail_multiplier(batch.material, detail_override)
                } else {
                    0.0
                },
                scroll: batch.material.scroll(now_ms),
            });

            let packed = pack::strided(&mut self.scratch, self.draw_stride, blocks);

            gpu.queue.write_buffer(&self.draw_buffer, 0, packed);
        }
    }

    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, tiles: &TileArray) {
        if self.draws.is_empty() {
            return;
        }

        pass.set_pipeline(&self.pipeline);

        pass.set_vertex_buffer(0, self.vertices.buffer.slice(..));

        pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint32);

        pass.set_bind_group(0, &self.frame_bind_group, &[]);

        let mut previous_pair = None;

        for (i, (batch, _)) in self.draws.iter().enumerate() {
            let (pair, binding) = tiles.binding(batch);

            if previous_pair != Some(pair) {
                pass.set_bind_group(1, binding, &[]);

                previous_pair = Some(pair);
            }

            pass.set_bind_group(2, &self.draw_bind_group, &[i as u32 * self.draw_stride]);

            pass.draw_indexed(batch.index_range(), 0, 0..1);
        }
    }

    pub fn draw_count(&self) -> usize {
        self.draws.len()
    }

    fn grow_indices(&mut self, gpu: &Gpu, quads: u32) {
        if quads <= self.index_capacity {
            return;
        }

        self.index_capacity = quads.next_power_of_two();

        self.index_buffer = gpu
            .device
            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: crate::gpu_label("terrain indices"),
                contents: bytemuck::cast_slice(&terrain_indices(self.index_capacity)),
                usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
            });
    }

    fn grow_draws(&mut self, gpu: &Gpu, draws: u32) {
        if draws <= self.draw_capacity {
            return;
        }

        self.draw_capacity = draws.next_power_of_two();

        let (buffer, bind_group) =
            draw_storage(gpu, &self.draw_layout, self.draw_stride, self.draw_capacity);

        self.draw_buffer = buffer;

        self.draw_bind_group = bind_group;
    }

    pub fn frame_layout(&self) -> &wgpu::BindGroupLayout {
        &self.frame_layout
    }
}

fn draw_storage(
    gpu: &Gpu,
    layout: &wgpu::BindGroupLayout,
    stride: u32,
    capacity: u32,
) -> (wgpu::Buffer, wgpu::BindGroup) {
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: crate::gpu_label("terrain draw"),
        size: u64::from(stride) * u64::from(capacity),
        usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });

    let bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: crate::gpu_label("terrain draw"),
        layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding {
                buffer: &buffer,
                offset: 0,
                size: wgpu::BufferSize::new(std::mem::size_of::<DrawUniforms>() as u64),
            }),
        }],
    });

    (buffer, bind_group)
}

fn draw_stride(alignment: u32) -> u32 {
    let size = std::mem::size_of::<DrawUniforms>() as u32;

    size.max(alignment).next_multiple_of(alignment.max(1))
}

pub const POINT_LIGHT_WGSL: &str = include_str!("shaders/point_light.wgsl");

pub fn lit_shader(body: &str) -> String {
    format!("{POINT_LIGHT_WGSL}{body}")
}

pub const WORLD_WGSL: &str = include_str!("shaders/world.wgsl");
