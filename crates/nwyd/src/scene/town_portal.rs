use super::effect_tree::{
    Base, Class, Cull, Effect, MeshSubmission, Pass, Sink, Submit, TexturedMesh, Tick,
};
use super::quads::{GroundQuad, Slot};
use super::timers::TimedQuad;
use glam::Vec3;
use render::billboard::BillboardPass;
use render::camera::Matrix4;

pub const SPAWN_ARGUMENT_COUNT: usize = 5;

pub const OBJECT_SIZE: u16 = 184;

pub const MESH_ID: i32 = 0x2BF;

pub const MESH_CACHE_MS: u32 = 180_000;

pub const LIFETIME_MS: u32 = 1_000;

pub const TEXTURE_BASE: i32 = 0x3A;

pub const DRAW_FLOOR: f32 = 0.05;

pub const EXPIRE_T: f32 = 1.0;

pub const CHILD_GAP_MS: u32 = 100;

pub const LAST_CHILD_INIT: u32 = 0;

pub const CHILD_TEXTURE: usize = 0x5E;

pub const CHILD_MS: u32 = 700;

pub const CHILD_SCALE: f32 = 1.5;

pub const CHILD_GROWTH: f32 = 0.0;

pub const CHILD_SPIN_PERIOD: u32 = 0;

pub const CHILD_LIFT_RATE: f32 = 2.0;

pub const DRAW_ROLL: f32 = std::f32::consts::FRAC_PI_2;

pub const DRAW_YAW: f32 = 0.0;

pub const SOUND: u32 = 0x9F;

pub const RAMP_KIND: i32 = 1;

pub const COLOUR_KIND_2: u32 = 0x0000_55FF;

pub const COLOUR_KIND_3: u32 = 0x00FF_5500;

pub const COLOUR_DEFAULT: u32 = 0xFFFF_FFFF;

pub fn colour_of(kind: i32) -> u32 {
    match kind {
        2 => COLOUR_KIND_2,
        3 => COLOUR_KIND_3,
        _ => COLOUR_DEFAULT,
    }
}

pub fn channels_of(argb: u32) -> [u32; 4] {
    [
        argb >> 24,
        (argb >> 16) & 0xFF,
        (argb >> 8) & 0xFF,
        argb & 0xFF,
    ]
}

pub fn pulse(t: f32) -> f32 {
    (std::f32::consts::PI * t).sin().abs()
}

pub fn channel(byte: u32, k: f32) -> u32 {
    ((byte as f32 * k) as u32).min(0xFF)
}

pub const CLASS: Class = Class {
    name: "SkillTownPortal",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

#[derive(Debug, Clone, PartialEq)]
pub struct TownPortal {
    pub kind: i32,
    pub position: Vec3,
    pub born_ms: u32,
    pub last_child_ms: u32,
    pub progress: f32,
    pub draw_scale_y: f32,
    pub argb: u32,
    pub channels: [u32; 4],
    pub scale_xz: f32,
    pub tint: u32,
    children: Vec<TimedQuad>,
    gone: bool,
}

impl TownPortal {
    pub fn new(position: Vec3, kind: i32, now_ms: u32) -> TownPortal {
        let argb = colour_of(kind);

        TownPortal {
            kind,
            position,
            born_ms: now_ms,
            last_child_ms: LAST_CHILD_INIT,
            progress: 0.0,
            draw_scale_y: 0.0,
            argb,
            channels: channels_of(argb),
            scale_xz: 0.0,
            tint: 0,
            children: Vec::new(),
            gone: false,
        }
    }

    pub fn texture_base(&self) -> i32 {
        TEXTURE_BASE
    }

    pub fn progress_at(&self, now_ms: u32) -> f32 {
        now_ms.wrapping_sub(self.born_ms) as f32 / LIFETIME_MS as f32
    }

    pub fn scale_for(&self, t: f32, k: f32) -> f32 {
        match self.kind == RAMP_KIND {
            true => t * 0.7 + 0.2,
            false => k * 0.5 + 0.5,
        }
    }

    pub fn child_due(&self, now_ms: u32) -> bool {
        now_ms.wrapping_sub(self.last_child_ms) > CHILD_GAP_MS
    }

    pub fn child_at(&self, t: f32) -> TimedQuad {
        let at = Vec3::new(
            self.position.x,
            self.position.y + CHILD_LIFT_RATE * t,
            self.position.z,
        );

        TimedQuad::new(
            GroundQuad {
                owner: 0,
                slot: Slot::Placed,
                texture: CHILD_TEXTURE,
                pass: BillboardPass::Additive,
                anchor: at,
                base_scale: Vec3::splat(CHILD_SCALE),
                grow: CHILD_GROWTH,
                spin_period: CHILD_SPIN_PERIOD,
                ordinal: 0,
                argb: self.argb,
            },
            CHILD_MS,
        )
    }

    pub fn tick_with(&mut self, now_ms: u32) {
        let t = self.progress_at(now_ms);

        self.progress = t;

        if t > EXPIRE_T {
            self.gone = true;

            return;
        }

        let k = pulse(t);

        let [_, r, g, b] = self.channels;

        self.tint = channel(r, k) << 16 | channel(g, k) << 8 | channel(b, k);

        if self.child_due(now_ms) {
            let child = self.child_at(t);

            self.children.push(child);

            self.last_child_ms = now_ms;
        }

        self.scale_xz = self.scale_for(t, k);

        self.draw_scale_y = t * std::f32::consts::PI;
    }

    pub fn draw(&self) -> Option<TexturedMesh> {
        if self.gone || self.progress < DRAW_FLOOR {
            return None;
        }

        let rotation = Matrix4::rotation_yaw_pitch_roll(
            0.0,
            -std::f32::consts::FRAC_PI_2,
            std::f32::consts::FRAC_PI_2,
        );

        let world = Matrix4::multiply(
            &Matrix4::multiply(
                &rotation,
                &Matrix4::scaling(self.scale_xz, self.draw_scale_y, self.scale_xz),
            ),
            &Matrix4::translation(self.position.x, self.position.y, self.position.z),
        );

        let [_, r, g, b] = self.tint.to_be_bytes();

        Some(TexturedMesh {
            mesh_id: 703,
            world,
            texture: self.texture_base() as usize,
            uv_offset: [0.0; 2],
            tint: [
                f32::from(r) / 255.0,
                f32::from(g) / 255.0,
                f32::from(b) / 255.0,
                1.0,
            ],
            blend: 1,
        })
    }
}

impl Effect for TownPortal {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        self.tick_with(tick.now_ms);
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, _cx: &Submit<'_>, out: &mut Sink<'_>) {
        if let Some(draw) = self.draw() {
            out.mesh.push(MeshSubmission::Textured(draw));
        }
    }

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        self.children
            .drain(..)
            .map(|c| Box::new(c) as Box<dyn Effect>)
            .collect()
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}
