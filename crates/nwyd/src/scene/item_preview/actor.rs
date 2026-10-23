use super::super::actors::{self, ClipChoice, ModelSpec};
use super::super::objects::{BoundTexture, Frame, ObjectScene};
use assets::GameDir;
use formats::mesh::appearance::{self, Look, Mount, PartRef};
use formats::mesh::glow;
use formats::mesh::msa::{StaticMesh, Subset};
use formats::mesh::msh::SkinMesh;
use formats::mesh::pose::{FrameTree, Smoothing};
use formats::mesh::{SkinVertex, StaticVertex};
use formats::texture::texture_list::FormatFlag;
use glam::{Vec3, Vec4};
use render::Matrix4;
use render::cull::Aabb;
use render::mesh::{Cull, DrawParams, MeshDraw, MeshPass, ObjectUniforms, SubsetRange};
use render::preset::ColorOp;
use render::texture::AlphaPolicy;
use render::world::Material;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Appearance {
    pub spec: ModelSpec,
    pub visible_parts: u16,
    pub refine: [u8; appearance::PARTS],
    pub tier: [u8; appearance::PARTS],
    pub tint: [u8; appearance::PARTS],
    pub costume: bool,
    pub costume_part_one: bool,
    pub mmulti: bool,
    pub mount_kind: u8,
}

impl Appearance {
    pub fn body(look: &Look) -> Option<Self> {
        let mut tint = look.part_tint;

        if look.substitutes_part_names() {
            tint[..6].fill(0);
        }

        Some(Self {
            spec: ModelSpec {
                model_id: look.model?,
                clip: ClipChoice::Action(actors::IDLE_ACTION),
                parts: look.parts,
                variant: look.variant,
                costume_body: look.costume_body,
            },
            visible_parts: u16::MAX,
            refine: look.part_color,
            tier: look.part_tier,
            tint,
            costume: look.costume_body != 0,
            costume_part_one: look.parts[1].mesh == actors::COSTUME_FLAG_MESH,
            mmulti: appearance::MMULTI_CLASSES.contains(&look.class),
            mount_kind: 0,
        })
    }

    pub fn mount(mount: &Mount) -> Option<Self> {
        let mut parts = [PartRef::EMPTY; appearance::PARTS];
        parts[..appearance::MOUNT_PARTS].copy_from_slice(&mount.parts);
        let mut refine = [0; appearance::PARTS];
        refine[..appearance::MOUNT_PARTS].copy_from_slice(&mount.part_color);

        Some(Self {
            spec: ModelSpec {
                model_id: mount.model?,
                clip: ClipChoice::Action(actors::IDLE_ACTION),
                parts,
                variant: 0,
                costume_body: 0,
            },
            visible_parts: u16::MAX,
            refine,
            tier: [0; appearance::PARTS],
            tint: [0; appearance::PARTS],
            costume: false,
            costume_part_one: false,
            mmulti: true,
            mount_kind: mount.visual_kind,
        })
    }

    pub fn cape(look: &Look) -> Option<Self> {
        let cape = look.cape?;
        let mut parts = [PartRef::EMPTY; appearance::PARTS];
        parts[0] = cape.part_for_body(look.costume_body);
        let mut refine = [0; appearance::PARTS];
        refine[0] = cape.rank;
        let mut tier = [0; appearance::PARTS];
        tier[0] = if look.costume_body == 100 { 1 } else { 12 };

        Some(Self {
            spec: ModelSpec {
                model_id: appearance::CAPE_MODEL,
                clip: ClipChoice::Fixed {
                    slot: look.cape_clip(actors::IDLE_ACTION)?,
                    sub_frame_ms: appearance::CAPE_SUB_FRAME_MS,
                },
                parts,
                variant: 0,
                costume_body: look.costume_body,
            },
            visible_parts: u16::MAX,
            refine,
            tier,
            tint: [0; appearance::PARTS],
            costume: look.costume_body == 100,
            costume_part_one: false,
            mmulti: false,
            mount_kind: 0,
        })
    }
}

pub struct Model {
    pub bounds: Aabb,
    parts: Vec<Part>,
}

struct Part {
    base_vertex: i32,
    subset: SubsetRange,
    texture: usize,
    opaque: bool,
    color_key: bool,
    unlit: bool,
    refinement: Option<(glow::Glow, usize)>,
}

pub fn load(game: &GameDir, resources: &mut ObjectScene, appearance: Appearance) -> Option<Model> {
    let bundle =
        actors::resources::Bundle::load(game, &resources.registries, &appearance.spec).ok()??;
    let model = bundle.models.first()?;
    let mut tree = FrameTree::build(&model.animation.skeleton);
    let mut anim = actors::Anim::new(model.slot, model.sub_frame_ms);
    let world = actors::skin_world(
        model.model_id,
        [0.0; 3],
        1.0,
        Vec3::ZERO,
        actors::Chirality::Mirrored,
    );
    let world = Matrix4::multiply(
        &world,
        &Matrix4::rotation_yaw_pitch_roll(0.0, std::f32::consts::FRAC_PI_2, 0.0),
    );
    actors::pose_anim(&mut tree, model, &mut anim, 0, &world, Smoothing::Off);

    let mut result = Model {
        bounds: Aabb::EMPTY,
        parts: Vec::new(),
    };

    for part in &model.parts {
        if part.index >= appearance::PARTS || appearance.visible_parts & (1 << part.index) == 0 {
            continue;
        }

        let Some(texture) = part.texture.and_then(|i| bundle.textures.get(i)) else {
            continue;
        };
        let baked = bake(&part.mesh, &tree)?;

        if baked.indices.is_empty() {
            continue;
        }

        let refinement = glow::for_part(
            appearance.refine[part.index].min(glow::PART_CLAMP),
            glow::effect_class(appearance.tier[part.index], appearance.tint[part.index]),
            glow::Part {
                texture: texture.slot as i32,
                color_key: texture.flag == FormatFlag::ColorKey,
                costume: appearance.costume || (part.index == 1 && appearance.costume_part_one),
                mmulti: appearance.mmulti,
                mount_kind: if part.index < 2 {
                    appearance.mount_kind
                } else {
                    0
                },
            },
        )
        .and_then(|refinement| {
            let slot = refinement.texture as usize;
            resources.load_effect_textures(game, &[slot]);
            resources
                .textures
                .iter()
                .position(|texture| texture.effect_table && texture.slot == slot)
                .map(|index| (refinement, index))
        });

        let texture_index = bind_texture(resources, texture);

        for vertex in &baked.vertices {
            result.bounds.grow(vertex.position);
        }

        let alloc = resources.arena.push(&baked);
        let alloc = &resources.arena.allocs[alloc];
        result.parts.push(Part {
            base_vertex: alloc.base_vertex,
            subset: alloc.subsets[0],
            texture: texture_index,
            opaque: texture.alpha_policy() == AlphaPolicy::Opaque,
            color_key: texture.flag == FormatFlag::ColorKey,
            unlit: model.model_id == 0x3D,
            refinement,
        });
    }

    (!result.parts.is_empty()).then_some(result)
}

impl Model {
    pub fn append(&self, out: &mut Frame, now_ms: u32) -> Range<usize> {
        let start = out.draws.len();
        let scroll = (now_ms % glow::SCROLL_PERIOD_MS) as f32 / glow::SCROLL_PERIOD_MS as f32;

        for part in &self.parts {
            let mut material = Material::SKIN_PART;
            material.diffuse[3] = 1.0;
            material.emissive[3] = 1.0;
            let params = DrawParams {
                material,
                lighting: !part.unlit,
                fog: false,
                color_op: ColorOp::Modulate,
                vertex_color: false,
                force_opaque: part.opaque,
                stage1: false,
            };
            let alpha_reference = if part.color_key {
                0.0
            } else {
                actors::MODEL_ALPHA_REFERENCE
            };
            let mut uniform = ObjectUniforms::new(Matrix4::IDENTITY, &params, alpha_reference);
            uniform.set_skin_material();

            if let Some((glow, _)) = part.refinement {
                uniform.set_skin_refine(glow, scroll);
            }

            let object = out.objects.len() as u32;
            out.objects.push(uniform);
            out.draws.push(MeshDraw {
                pass: MeshPass::Opaque,
                cull: Cull::None,
                object,
                texture: part.texture,
                stage1: part.refinement.map(|(_, index)| index),
                base_vertex: part.base_vertex,
                subset: part.subset,
            });
        }

        start..out.draws.len()
    }
}

fn bind_texture(resources: &mut ObjectScene, texture: &BoundTexture) -> usize {
    let policy = texture.mip_policy(true);

    if let Some(index) = resources.textures.iter().position(|existing| {
        existing.slot == texture.slot
            && existing.effect_table == texture.effect_table
            && existing.mip_policy(false) == policy
    }) {
        return index;
    }

    let mut texture = texture.clone();
    texture.mip_policy_override = Some(policy);
    resources.textures.push(texture);
    resources.textures.len() - 1
}

fn bake(mesh: &SkinMesh, tree: &FrameTree) -> Option<StaticMesh> {
    let matrices: Vec<_> = mesh
        .bone_offsets
        .iter()
        .zip(&mesh.bone_indices)
        .map(|(offset, &bone)| {
            Matrix4::multiply(&Matrix4(*offset), &Matrix4(tree.combined(bone as usize)))
        })
        .collect();
    let vertices: Vec<_> = mesh
        .vertices
        .iter()
        .map(|vertex| bake_vertex(vertex, &matrices))
        .collect::<Option<_>>()?;

    Some(StaticMesh {
        fvf: 0x112,
        subsets: vec![Subset {
            material_id: 0,
            start_triangle: 0,
            prim_count: mesh.indices.len() as u32 / 3,
            min_vertex_index: 0,
            num_vertices: vertices.len() as u32,
        }],
        texture_fields: Vec::new(),
        indices: mesh.indices.clone(),
        vertices,
        has_normals: true,
        has_uvs: true,
        has_colors: false,
    })
}

fn bake_vertex(vertex: &SkinVertex, matrices: &[Matrix4]) -> Option<StaticVertex> {
    let last = matrices.len().checked_sub(1)?;
    let mut position = Vec3::ZERO;
    let mut normal = Vec3::ZERO;

    for (&bone, &weight) in vertex.bone_indices.iter().zip(&vertex.bone_weights) {
        let matrix = glam::Mat4::from_cols_array(&matrices[usize::from(bone).min(last)].as_d3dx());
        position += (matrix * Vec4::from((Vec3::from(vertex.position), 1.0))).truncate() * weight;
        normal += (matrix * Vec4::from((Vec3::from(vertex.normal), 0.0))).truncate() * weight;
    }

    (position.is_finite() && normal.is_finite()).then_some(StaticVertex {
        position: position.to_array(),
        normal: normal.normalize_or_zero().to_array(),
        uv: vertex.uv,
        color: 0xFFFF_FFFF,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_bake_transforms_positions_and_normals_independently() {
        let vertex = SkinVertex {
            position: [2.0, 3.0, 4.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.25, 0.75],
            bone_indices: [0, 1, 0, 0],
            bone_weights: [0.25, 0.75, 0.0, 0.0],
        };
        let result = bake_vertex(
            &vertex,
            &[Matrix4::IDENTITY, Matrix4::translation(4.0, 8.0, 12.0)],
        )
        .unwrap();
        assert_eq!(result.position, [5.0, 9.0, 13.0]);
        assert_eq!(result.normal, vertex.normal);
        assert_eq!(result.uv, vertex.uv);
    }

    #[test]
    fn bake_rejects_nonfinite_vertices_and_empty_palettes() {
        let vertex = SkinVertex {
            position: [f32::NAN, 0.0, 0.0],
            normal: [0.0, 0.0, 1.0],
            uv: [0.0; 2],
            bone_indices: [0; 4],
            bone_weights: [1.0, 0.0, 0.0, 0.0],
        };
        assert!(bake_vertex(&vertex, &[Matrix4::IDENTITY]).is_none());
        assert!(bake_vertex(&vertex, &[]).is_none());
    }
}
