use super::rng::Lcg;
use formats::world::objects::{ObjRecord, ObjectClass};
use glam::Vec3;
use render::skin::{SkinFog, SkinParams};

pub const FLOAT_MODEL: u32 = 0x3E;

pub const FLOAT_SUB_FRAME_MS: u32 = 0x50;

pub const FLOAT_SLOT: usize = 0;

pub const FLOAT_SCALE: f32 = 1.5;

pub const MESH_LIFT: f32 = 0.3;

pub const QUAD_LIFT: f32 = 0.36;

pub const TRAIL_DROP: f32 = 0.1;

pub const NO_GROUND: f32 = -100.0;

pub const FOLLOW_AFTER_MS: u32 = 500;

pub const BODY_TEXTURE: usize = 0x5A;

pub const GLOW_TEXTURE: usize = 10;

pub const PHASE_STEP_MS: u32 = 300;

pub const PHASE_STEPS: u32 = 10;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Float {
    pub zone: (u8, u8),
    pub record: usize,
    pub kind: u32,
    pub position: Vec3,
    pub yaw: f32,
    pub grounded: bool,
    pub model: Option<usize>,
    pub phase_ms: u32,
    pub diffuse: [f32; 4],
}

impl Float {
    pub fn has_mesh(&self) -> bool {
        self.kind == 3
    }

    pub fn start_offset_ms(record: usize) -> u32 {
        Lcg::for_object(record, 0).next_mod(PHASE_STEPS) * PHASE_STEP_MS
    }

    pub fn snap(kind: u32, world: [f32; 3], height: Option<f32>) -> (Vec3, bool) {
        let lift = if kind == 3 { MESH_LIFT } else { QUAD_LIFT };

        match height.filter(|h| h.is_finite() && *h > -100.0) {
            Some(h) => (Vec3::new(world[0], h + lift, world[2]), true),
            None => (Vec3::new(world[0], world[1], world[2]), false),
        }
    }

    pub fn refresh(&mut self, height: Option<f32>, light: [f32; 4]) {
        let (position, grounded) = Self::snap(self.kind, self.position.to_array(), height);

        self.position = position;

        self.grounded = grounded;

        if self.has_mesh() {
            self.diffuse = diffuse(light);
        }
    }

    pub fn trail(&self, now_ms: u32) -> Vec3 {
        if now_ms <= FOLLOW_AFTER_MS {
            return Vec3::ZERO;
        }

        Vec3::new(
            self.position.x,
            self.position.y - TRAIL_DROP,
            self.position.z,
        )
    }

    pub fn quads(&self, ordinal: usize) -> Vec<super::quads::GroundQuad> {
        use super::quads::{GroundQuad, Slot};

        let mut out = Vec::new();

        if self.kind == 5 {
            out.push(GroundQuad {
                owner: self.record,
                slot: Slot::Body,
                texture: BODY_TEXTURE,
                pass: render::billboard::BillboardPass::Blended,
                anchor: self.position,
                base_scale: Vec3::splat(1.0),
                grow: 0.0005,
                spin_period: 0,
                ordinal,
                argb: u32::MAX,
            });
        }

        out.push(GroundQuad {
            owner: self.record,
            slot: Slot::Glow,
            texture: GLOW_TEXTURE,
            pass: render::billboard::BillboardPass::Additive,
            anchor: self.position,
            base_scale: Vec3::splat(0.5),
            grow: 0.001,
            spin_period: 0,
            ordinal: ordinal + usize::from(self.kind == 5),
            argb: u32::MAX,
        });

        out
    }

    pub fn behind_camera(&self, eye: Vec3, forward: Vec3) -> bool {
        forward.dot(self.position - eye) <= 0.0
    }

    pub fn params(&self, fog: SkinFog) -> SkinParams {
        SkinParams {
            diffuse: self.diffuse,
            ambient: super::actors::MATERIAL_AMBIENT,
            emissive: super::actors::MATERIAL_EMISSIVE,
            unlit: false,
            fog_start: fog.start,
            fog_end: fog.end,
            fog_enabled: fog.enabled,
            force_opaque: false,
            modulate_alpha: false,
            glow: None,
        }
    }
}

pub fn water_height(
    seas: &[super::sea::Sea],
    zone: (u8, u8),
    x: f32,
    z: f32,
    now_ms: u32,
) -> Option<f32> {
    let sea = seas
        .iter()
        .filter(|s| s.zone == zone)
        .take(10)
        .find(|s| s.contains(x, z))?;

    let left = (sea.position.x - sea.cols as f32) as i32 as f32;

    let phase = std::f32::consts::PI * ((x - left) * 0.5 + 2.0 * (now_ms % 12000) as f32 / 6000.0);

    Some(sea.position.y + phase.sin() * 0.1 - 0.1)
}

pub fn diffuse(terrain: [f32; 4]) -> [f32; 4] {
    let mut out = [0.0f32; 4];

    for i in 0..3 {
        out[i] = terrain[i] * 0.3 + 0.5;
    }

    out[3] = 0.0;

    out
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Raft {
    pub floats: Vec<Float>,
}

impl Raft {
    pub fn build(
        records: &[ObjRecord],
        zone: (u8, u8),
        height: impl Fn(f32, f32) -> Option<f32>,
        light: impl Fn(f32, f32) -> [f32; 4],
    ) -> Raft {
        let mut floats = Vec::new();

        for (record, r) in records.iter().enumerate() {
            if r.class() != ObjectClass::Prop {
                continue;
            }

            let world = r.world_position(u32::from(zone.0), u32::from(zone.1));

            let (position, grounded) = Float::snap(r.kind, world, height(world[0], world[2]));

            floats.push(Float {
                zone,
                record,
                kind: r.kind,
                position,
                yaw: r.yaw,
                grounded,
                model: None,
                phase_ms: Float::start_offset_ms(record),
                diffuse: diffuse(light(position.x, position.z)),
            });
        }

        Raft { floats }
    }

    pub fn meshed(&self) -> impl Iterator<Item = &Float> {
        self.floats.iter().filter(|f| f.has_mesh())
    }
}
