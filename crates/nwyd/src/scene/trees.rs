use formats::mesh::appearance::PartRef;
use formats::world::objects::{ObjRecord, ObjectClass};
use formats::world::trn::Terrain;
use glam::Vec3;
use render::skin::{SkinFog, SkinParams};

pub const TREE_SUB_FRAME_MS: u32 = 0x50;

pub const TREE_SLOT: usize = 0;

pub const KIND_RANGE_A: std::ops::RangeInclusive<u32> = 0x14B..=0x156;

pub const MODEL_BASE_A: u32 = 0x3F;

pub const KIND_RANGE_B: std::ops::RangeInclusive<u32> = 0x15F..=0x190;

pub const MODEL_BASE_B: u32 = 0x47;

pub const PALE_MODEL: u32 = 0x57;

pub const PALE_EMISSIVE: [f32; 4] = [1.0, 0.9, 0.9, 0.0];

pub fn model_index(kind: u32) -> Option<u32> {
    if KIND_RANGE_A.contains(&kind) {
        return Some((kind - *KIND_RANGE_A.start()) / 2 + MODEL_BASE_A);
    }

    if KIND_RANGE_B.contains(&kind) {
        return Some((kind - *KIND_RANGE_B.start()) / 2 + MODEL_BASE_B);
    }

    None
}

pub fn part_zero(kind: u32) -> PartRef {
    match kind {
        0x156 => PartRef { mesh: 1, tex: 0 },
        0x162 => PartRef { mesh: 0, tex: 1 },
        0x16A => PartRef { mesh: 1, tex: 0 },
        0x169 | 0x175 | 0x177 | 0x179 => PartRef { mesh: 0, tex: 1 },
        _ => PartRef::EMPTY,
    }
}

pub fn kingdom_kind(kind: u32, kingdom: u8) -> u32 {
    if !matches!(kind, 0x162 | 0x179 | 0x17A) {
        return kind;
    }

    match kingdom {
        7 => 0x17A,
        8 => 0x179,
        _ => 0x162,
    }
}

pub const TERRAIN_MIX: f32 = 0.4;

pub const LIGHT0_MIX: f32 = 0.6;

pub const EMISSIVE_MIX: f32 = 0.4;

#[derive(Debug, Clone, PartialEq)]

pub struct Tree {
    pub record: usize,
    pub kind: u32,
    pub model_id: u32,
    pub model: Option<usize>,
    pub position: Vec3,
    pub yaw: f32,
    pub phase_ms: u32,
    pub firefly_stamp_ms: u32,
    pub diffuse: [f32; 4],
    pub emissive: [f32; 4],
}

impl Tree {
    pub fn firefly(
        &mut self,
        now_ms: u32,
        frame: usize,
        enabled: bool,
        rng: &mut super::rng::Lcg,
    ) -> Option<super::effects::Billboard> {
        if !enabled
            || !matches!(self.kind, 0x16B..=0x16F)
            || !(7..12).contains(&frame)
            || now_ms.wrapping_sub(self.firefly_stamp_ms) <= 1000
        {
            return None;
        }

        self.firefly_stamp_ms = now_ms;

        let group = ((self.kind - 0x16B) / 2) as usize;

        let x = (rng.next_mod(10) as i32 - 5) as f32 * 0.05;

        let z = (rng.next_mod(10) as i32 - 5) as f32 * 0.05;

        let mut b =
            super::effects::Billboard::new(0, 1500, Vec3::new(0.01, 0.1, 0.01), 0.001, 1, 80);

        b.position = self.position + Vec3::new(x, [1.6, 1.4, 1.4][group], z);

        b.origin = b.position;

        b.birth_ms = Some(now_ms);

        b.pass = render::billboard::BillboardPass::Additive;

        b.motion = super::effects::Motion::Rise;

        b.rise = 2.0;

        b.set_color([0x00008822, 0x00884400, 0x00770000][group]);

        Some(b)
    }

    pub fn params(&self, fog: SkinFog) -> SkinParams {
        SkinParams {
            diffuse: self.diffuse,
            ambient: [0.0; 4],
            emissive: self.emissive,
            unlit: false,
            fog_start: fog.start,
            fog_end: fog.end,
            fog_enabled: fog.enabled,
            force_opaque: false,
            modulate_alpha: true,
            glow: None,
        }
    }
}

pub fn material(terrain: [f32; 4], light0: [f32; 4], light1: [f32; 4]) -> ([f32; 4], [f32; 4]) {
    let mut diffuse = [0.0f32; 4];

    let mut emissive = [0.0f32; 4];

    for i in 0..3 {
        diffuse[i] = light0[i] * LIGHT0_MIX + terrain[i] * TERRAIN_MIX;

        emissive[i] = diffuse[i] * EMISSIVE_MIX + light1[i] * EMISSIVE_MIX;
    }

    diffuse[3] = terrain[3];

    (diffuse, emissive)
}

#[derive(Debug, Clone, Default, PartialEq)]

pub struct Grove {
    pub trees: Vec<Tree>,
    pub unresolved: Vec<(usize, u32)>,
}

impl Grove {
    pub fn build(
        records: &[ObjRecord],
        terrain: &Terrain,
        relief: &[i8],
        light0: [f32; 4],
        light1: [f32; 4],
    ) -> Grove {
        let (ox, oz) =
            formats::world::zone_origin(u32::from(terrain.zone_x), u32::from(terrain.zone_y));

        let mut trees = Vec::new();

        let mut unresolved = Vec::new();

        for (record, r) in records.iter().enumerate() {
            if r.class() != ObjectClass::Tree {
                continue;
            }

            let Some(model_id) = model_index(r.kind) else {
                unresolved.push((record, r.kind));

                continue;
            };

            let [x, _, z] = r.world_position(u32::from(terrain.zone_x), u32::from(terrain.zone_y));

            let y = super::actors::ground_height(relief, terrain, x, z);

            let (diffuse, emissive) = material(terrain.color_at(x - ox, z - oz), light0, light1);

            let ordinal = trees.len() as u32;

            trees.push(Tree {
                record,
                kind: r.kind,
                model_id,
                model: None,
                position: Vec3::new(x, y, z),
                yaw: r.yaw,
                phase_ms: (ordinal % super::grass::PHASE_STEPS) * super::grass::PHASE_STEP_MS,
                firefly_stamp_ms: 0,
                diffuse,
                emissive: if model_id == PALE_MODEL {
                    PALE_EMISSIVE
                } else {
                    emissive
                },
            });
        }

        Grove { trees, unresolved }
    }

    pub fn specs(&self) -> Vec<(u32, PartRef)> {
        let mut v: Vec<(u32, PartRef)> = self
            .trees
            .iter()
            .map(|t| (t.model_id, part_zero(t.kind)))
            .collect();

        v.sort_unstable_by_key(|(m, p)| (*m, p.mesh, p.tex));

        v.dedup();

        v
    }
}
