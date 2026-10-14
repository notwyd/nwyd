use super::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use glam::Vec3;
use render::billboard::{
    BillboardDraw, BillboardPass, BillboardUniforms, BillboardVertex, QUAD_INDICES,
};
use render::camera::Matrix4;

pub const CORNERS: [([f32; 3], [f32; 2]); 4] = [
    ([-0.5, 0.0, -0.5], [0.02, 0.02]),
    ([0.5, 0.0, -0.5], [0.98, 0.02]),
    ([0.5, 0.0, 0.5], [0.98, 0.98]),
    ([-0.5, 0.0, 0.5], [0.02, 0.98]),
];

pub const DIAMOND: f32 = std::f32::consts::FRAC_PI_4;

pub const SPIN_TURN: f32 = std::f32::consts::TAU;

pub const CYCLE_MS: u32 = 1_000;

pub const GROWTH: f32 = 500.0;

pub const PHASE_STEP_MS: u32 = 300;

pub const PHASE_STEPS: u32 = 10;

pub const FROZEN_T: f32 = 0.3;

pub const COLOUR_MODE: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GroundQuad {
    pub owner: usize,
    pub slot: Slot,
    pub texture: usize,
    pub pass: BillboardPass,
    pub anchor: Vec3,
    pub base_scale: Vec3,
    pub grow: f32,
    pub spin_period: u32,
    pub ordinal: usize,
    pub argb: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Slot {
    Body,
    Glow,
    FollowingGlow,
    Placed,
}

impl GroundQuad {
    pub fn at(&self, now_ms: u32) -> Vec3 {
        match self.slot {
            Slot::Body | Slot::Placed => self.anchor,
            Slot::FollowingGlow => Vec3::new(
                self.anchor.x,
                self.anchor.y - super::floats::TRAIL_DROP,
                self.anchor.z,
            ),
            Slot::Glow if now_ms > super::floats::FOLLOW_AFTER_MS => Vec3::new(
                self.anchor.x,
                self.anchor.y - super::floats::TRAIL_DROP,
                self.anchor.z,
            ),
            Slot::Glow => Vec3::ZERO,
        }
    }

    pub fn t(&self, now_ms: u32, at: Vec3) -> f32 {
        if !self.pass.is_additive() {
            return FROZEN_T;
        }

        let round = |v: f32| v.round_ties_even() as i32 as u32;

        let phase = (self.ordinal as u32 % PHASE_STEPS) * PHASE_STEP_MS;

        let n = round(at.z)
            .wrapping_add(now_ms)
            .wrapping_add(phase)
            .wrapping_add(round(at.x))
            % CYCLE_MS;

        n as f32 / CYCLE_MS as f32
    }

    pub fn tint(&self, t: f32) -> u32 {
        let k = (std::f32::consts::PI * t).sin().abs();

        let scale = |shift: u32| {
            let byte = (self.argb >> shift) & 0xFF;

            ((byte as f32 * k).round_ties_even() as i64 as u32 & 0xFF) << shift
        };

        scale(24) | scale(16) | scale(8) | scale(0)
    }

    pub fn scale(&self, t: f32) -> Vec3 {
        if !self.pass.is_additive() {
            return self.base_scale;
        }

        self.base_scale + Vec3::splat(t * GROWTH * self.grow)
    }

    pub fn scale_lived(&self, elapsed_ms: u32) -> Vec3 {
        self.base_scale + Vec3::splat(elapsed_ms as f32 * self.grow)
    }

    pub fn yaw(&self, now_ms: u32) -> f32 {
        if self.spin_period == 0 {
            return 0.0;
        }

        (now_ms % self.spin_period) as f32 / self.spin_period as f32 * SPIN_TURN
    }

    pub fn world(&self, at: Vec3, t: f32, now_ms: u32) -> Matrix4 {
        self.world_with(at, self.scale(t), now_ms)
    }

    pub fn world_with(&self, at: Vec3, scale: Vec3, now_ms: u32) -> Matrix4 {
        let spun = Matrix4::rotation_yaw_pitch_roll(self.yaw(now_ms) + DIAMOND, 0.0, 0.0);

        let scaled = Matrix4::multiply(&spun, &Matrix4::scaling(scale.x, scale.y, scale.z));

        Matrix4::multiply(&scaled, &Matrix4::translation(at.x, at.y, at.z))
    }

    pub fn vertices(&self, t: f32) -> [BillboardVertex; 4] {
        let diffuse = self.tint(t);

        CORNERS.map(|(position, uv)| BillboardVertex {
            position,
            diffuse,
            uv,
        })
    }

    pub fn submit_with(&self, cx: &Submit<'_>, out: &mut Sink<'_>, life: Option<Life>) {
        if cx.scene.detail == 1 {
            return;
        }

        self.submit_coloured(cx, out, life, None);
    }

    pub fn submit_coloured(
        &self,
        cx: &Submit<'_>,
        out: &mut Sink<'_>,
        life: Option<Life>,
        colour: Option<u32>,
    ) {
        if life.is_some_and(|life| life.t() < 0.05) {
            return;
        }

        if self.cull(cx.eye, false) == Cull::Rejected {
            out.effect.culled += 1;

            return;
        }

        let Some((array, layer)) = cx.scene.placement(self.texture) else {
            out.effect.unbound += 1;

            return;
        };

        let at = self.at(cx.now_ms);

        let (t, scale) = match life {
            None => {
                let t = self.t(cx.now_ms, at);

                (t, self.scale(t))
            }
            Some(life) => (life.t(), self.scale_lived(life.elapsed_ms)),
        };

        let object = out.effect.objects.len() as u32;

        out.effect.objects.push(
            BillboardUniforms::with_alpha_op(
                self.world_with(at, scale, cx.now_ms),
                self.pass,
                false,
            )
            .with_layer(layer)
            .with_opaque(cx.scene.alpha_at(self.texture)),
        );

        let base_vertex = out.effect.vertices.len() as u32;

        let first = out.effect.indices.len() as u32;

        let mut vertices = self.vertices(t);

        if let Some(colour) = colour {
            for vertex in &mut vertices {
                vertex.diffuse = colour;
            }
        }

        out.effect.vertices.extend_from_slice(&vertices);

        out.effect.indices.extend(QUAD_INDICES);

        out.effect.draws.push(BillboardDraw {
            pass: self.pass,
            object,
            texture: array,
            base_vertex,
            indices: first..out.effect.indices.len() as u32,
        });

        out.effect.quads += 1;

        out.effect.submitted(&CLASS);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Life {
    pub lifetime_ms: u32,
    pub elapsed_ms: u32,
}

impl Life {
    pub fn t(self) -> f32 {
        self.elapsed_ms as f32 / self.lifetime_ms as f32
    }
}

pub const CLASS: Class = Class {
    name: "EffectBillboard2",
    method_count: 17,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

impl Effect for GroundQuad {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, _tick: &Tick) {}

    fn expired(&self) -> bool {
        false
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        self.submit_with(cx, out, None);
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}
