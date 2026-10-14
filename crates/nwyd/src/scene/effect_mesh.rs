use super::effect_tree::{Base, Class, Cull as TreeCull, Effect, Pass, Sink, Submit, Tick};
use super::lights::Glow;
use glam::Vec3;
use render::camera::Matrix4;
use render::mesh::{Cull, DrawParams, EFFECT_ADDITIVE, MeshPass, effect_states};
use std::ops::RangeInclusive;

pub const CLASS: Class = Class {
    name: "EffectMesh",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

pub const DEFAULT_PERIOD_MS: u32 = 1000;

pub const DEFAULT_SCALE: f32 = 1.0;

pub const DEFAULT_ROLL: f32 = std::f32::consts::FRAC_PI_2;

pub const NO_TEXTURE_BIAS: i32 = -1;

pub const UNFILED_SECTOR: i32 = -1;

pub const IMMORTAL_MS: u32 = 0;

pub const UNDRAWN_MESH_IDS: RangeInclusive<i32> = 0x208..=0x212;

pub const DEAD_CULL_REASSERT_MESH_ID: i32 = 0xBF9;

pub const PULSE_BASE: f32 = 0.8;

pub const PULSE_SWING: f32 = 0.2;

pub const MODE_SPIN_SCALE: f32 = 3.0;

pub const MODE_RAMP_KNEE: f32 = 0.2;

pub const MODE_RAMP_RATE: f32 = 5.0;

pub const MODE_RAMP_BASE: f32 = 0.5;

pub const MODE_RAMP_BIAS: f32 = 1.5;

pub const UV_SCROLL_U_RATE: f32 = 0.001;

pub const UV_SCROLL_V_RATE: f32 = 0.1;

pub const UPDATE_SECTOR_KEYS: usize = 4;

pub const DRAW_SECTOR_KEYS: usize = 9;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub enum UvScroll {
    #[default]
    None,
    U,
    V,
}

impl UvScroll {
    pub fn advance(self, offset: &mut [f32; 2], phase: f32, now_ms: u32) {
        let (axis, step) = match self {
            Self::None => return,
            Self::U => (0, phase * UV_SCROLL_U_RATE),
            Self::V => (1, (now_ms % 100) as f32 / 100.0 * UV_SCROLL_V_RATE),
        };

        offset[axis] = (offset[axis] + step).rem_euclid(1.0);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Visibility {
    Kept,
    Rejected,
    Projected,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct CullView {
    pub eye: Vec3,
    pub forward: Vec3,
    pub radius: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct ModeDraw {
    pub scale_xz: f32,
    pub scale_y: f32,
    pub texture_bias: Option<i32>,
    pub yaw: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Draw {
    pub mesh_id: i32,
    pub world: Matrix4,
    pub blend: u32,
    pub tint: [f32; 4],
    pub texture_bias: Option<i32>,
    pub uv_offset: [f32; 2],
}

impl Draw {
    pub fn states(&self) -> (MeshPass, Cull, DrawParams) {
        effect_states(self.blend, self.tint)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct EffectMesh {
    pub mesh_id: i32,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub scale_xz: f32,
    pub scale_y: f32,
    pub mode: i32,
    pub blend: u32,
    pub texture_bias: i32,
    pub pulse: bool,
    pub uv_scroll: UvScroll,
    pub uv_offset: [f32; 2],
    pub lifetime_ms: u32,
    pub period_ms: u32,
    pub born_ms: u32,
    pub phase: f32,
    pub colour: u32,
    pub enabled: bool,
    pub visible: bool,
    pub cull_radius: f32,
    pub sector: i32,
    unregistered: bool,
}

impl EffectMesh {
    pub fn new(mesh_id: i32, argb: u32, yaw: f32, mode: i32, now_ms: u32) -> EffectMesh {
        EffectMesh {
            mesh_id,
            position: Vec3::ZERO,
            yaw,
            pitch: 0.0,
            roll: DEFAULT_ROLL,
            scale_xz: DEFAULT_SCALE,
            scale_y: DEFAULT_SCALE,
            mode,
            blend: EFFECT_ADDITIVE,
            texture_bias: NO_TEXTURE_BIAS,
            pulse: false,
            uv_scroll: UvScroll::None,
            uv_offset: [0.0; 2],
            lifetime_ms: IMMORTAL_MS,
            period_ms: DEFAULT_PERIOD_MS,
            born_ms: now_ms,
            phase: 0.0,
            colour: argb,
            enabled: true,
            visible: true,
            cull_radius: 1.0,
            sector: UNFILED_SECTOR,
            unregistered: false,
        }
    }

    pub fn channels(&self) -> [u32; 4] {
        [
            self.colour >> 24,
            (self.colour >> 16) & 0xFF,
            (self.colour >> 8) & 0xFF,
            self.colour & 0xFF,
        ]
    }

    pub fn tick(&mut self, now_ms: u32) {
        let age = now_ms.wrapping_sub(self.born_ms);

        if self.period_ms != 0 {
            self.phase = (age % self.period_ms) as f32 / self.period_ms as f32;
        }

        if self.lifetime_ms != IMMORTAL_MS && self.lifetime_ms < age {
            self.visible = false;

            self.unregistered = true;
        }

        if !self.unregistered && self.blend != render::mesh::EFFECT_TEXTURE_ONLY_MODE {
            self.uv_scroll
                .advance(&mut self.uv_offset, self.phase, now_ms);
        }
    }

    pub fn admitted(&self, keys: &[i32], scene_state: u32) -> bool {
        self.sector == UNFILED_SECTOR
            || keys.contains(&self.sector)
            || (self.mesh_id == super::lights::GLOW_MESH_ID
                && scene_state == super::world::SCENE_SELECT_CHAR)
    }

    pub fn cull_on_field(&self, view: &CullView, mesh_loaded: bool) -> Visibility {
        if !self.enabled || !mesh_loaded {
            return Visibility::Rejected;
        }

        let to = self.position - view.eye;

        if to.length() <= view.radius {
            return Visibility::Kept;
        }

        match view.forward.dot(to) > 0.0 {
            true => Visibility::Projected,
            false => Visibility::Rejected,
        }
    }

    pub fn scalar(&self) -> f32 {
        if self.mode == 4 {
            return (std::f32::consts::PI * self.phase).sin();
        }

        if self.pulse {
            return PULSE_BASE + PULSE_SWING * (std::f32::consts::TAU * self.phase).sin();
        }

        1.0
    }

    pub fn vertex_diffuse(&self) -> u32 {
        let k = self.scalar();

        let [a, r, g, b] = self.channels();

        let scale = |c: u32| (c as f32 * k).round_ties_even() as i64 as u32;

        (((((scale(a) << 8) | scale(r)) << 8) | scale(g)) << 8) | scale(b)
    }

    pub fn tint(&self) -> [f32; 4] {
        let packed = self.vertex_diffuse();

        let byte = |shift: u32| ((packed >> shift) & 0xFF) as f32 / 255.0;

        [byte(16), byte(8), byte(0), byte(24)]
    }

    pub fn uv_step(&self, now_ms: u32) -> (f32, f32) {
        match self.uv_scroll {
            UvScroll::None => (0.0, 0.0),
            UvScroll::U => (self.phase * UV_SCROLL_U_RATE, 0.0),
            UvScroll::V => (0.0, ((now_ms % 100) as f32 / 100.0) * UV_SCROLL_V_RATE),
        }
    }

    pub fn mode_draw(&self) -> ModeDraw {
        let mut out = ModeDraw {
            scale_xz: self.scale_xz,
            scale_y: self.scale_y,
            texture_bias: None,
            yaw: self.yaw,
        };

        let t = self.phase;

        match self.mode {
            1 | 5 => out.texture_bias = Some(self.texture_bias),
            2 => {
                out.scale_y = self.scale_xz * t * MODE_SPIN_SCALE;

                out.yaw = t * std::f32::consts::PI;
            }
            3 => out.yaw = t * std::f32::consts::PI,
            4 => {
                out.texture_bias = Some(self.texture_bias);

                if t >= MODE_RAMP_KNEE {
                    let s = ((t - MODE_RAMP_KNEE) * std::f32::consts::PI * 0.5).sin();

                    out.scale_xz = (s + MODE_RAMP_BIAS) * self.scale_xz;

                    out.scale_y = (s + MODE_RAMP_BIAS) * self.scale_y;
                } else {
                    let ramp = t * MODE_RAMP_RATE + MODE_RAMP_BASE;

                    out.scale_xz = self.scale_xz * ramp;

                    out.scale_y = ramp * self.scale_y;
                }
            }
            _ => {}
        }

        out
    }

    pub fn world(&self) -> Matrix4 {
        let d = self.mode_draw();

        let r = Matrix4::rotation_yaw_pitch_roll(
            d.yaw,
            self.pitch - std::f32::consts::FRAC_PI_2,
            self.roll,
        );

        let m = Matrix4::multiply(&super::objects::DEVICE_WORLD, &r);

        let m = Matrix4::multiply(&m, &Matrix4::scaling(d.scale_xz, d.scale_y, d.scale_xz));

        Matrix4::multiply(
            &m,
            &Matrix4::translation(self.position.x, self.position.y, self.position.z),
        )
    }

    pub fn draws(&self) -> bool {
        self.visible && !UNDRAWN_MESH_IDS.contains(&self.mesh_id)
    }

    pub fn draw(&self) -> Option<Draw> {
        if !self.draws() {
            return None;
        }

        Some(Draw {
            mesh_id: self.mesh_id,
            world: self.world(),
            blend: self.blend,
            tint: self.tint(),
            texture_bias: self.mode_draw().texture_bias,
            uv_offset: self.uv_offset,
        })
    }

    pub fn as_glow(&self) -> Option<Glow> {
        if self.blend != EFFECT_ADDITIVE || !self.draws() {
            return None;
        }

        let d = self.mode_draw();

        Some(Glow {
            mesh_id: self.mesh_id,
            position: self.position,
            yaw: d.yaw,
            scale: (d.scale_xz, d.scale_y),
            colour: self.vertex_diffuse(),
            pulse: false,
            period_ms: self.period_ms,
            sector: self.sector,
            uv_scroll: self.uv_scroll,
            uv_offset: self.uv_offset,
        })
    }
}

impl Effect for EffectMesh {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        self.tick(tick.now_ms);
    }

    fn expired(&self) -> bool {
        self.unregistered
    }

    fn submit(&self, _cx: &Submit<'_>, out: &mut Sink<'_>) {
        use super::effect_tree::{AuthoredMesh, MeshMaterial, MeshSubmission, TexturedMesh};

        let Some(draw) = self.draw() else {
            return;
        };

        let Ok(mesh_id) = u32::try_from(draw.mesh_id) else {
            return;
        };

        out.mesh.push(
            match draw
                .texture_bias
                .and_then(|slot| usize::try_from(slot).ok())
            {
                Some(texture) => MeshSubmission::Textured(TexturedMesh {
                    mesh_id,
                    world: draw.world,
                    texture,
                    tint: draw.tint,
                    blend: draw.blend,
                    uv_offset: draw.uv_offset,
                }),
                None => MeshSubmission::Authored(AuthoredMesh {
                    mesh_id,
                    world: draw.world,
                    texture: None,
                    material: MeshMaterial::Effect {
                        blend: draw.blend,
                        tint: draw.tint,
                        uv_offset: draw.uv_offset,
                    },
                }),
            },
        );
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> TreeCull {
        match self.enabled {
            true => TreeCull::Kept,
            false => TreeCull::Rejected,
        }
    }
}
