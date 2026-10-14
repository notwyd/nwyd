use crate::scene::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use crate::scene::effects::{Billboard, SECTOR_UNASSIGNED};
use crate::scene::quads::{GroundQuad, Slot};
use crate::scene::shade::{Ground, Shade};
use crate::scene::timers::TimedQuad;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::{Axis, Spawn, SpawnArgument};
use glam::Vec3;
use render::billboard::BillboardPass;

pub const CLASS: Class = Class {
    name: "SkillSpeedUp",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const LIFETIME_MS: u32 = 1000;

pub const QUAD_TEXTURE: usize = 8;

pub const QUAD_MS: u32 = 1000;

pub const QUAD_SCALE_BASE: f64 = 0.01;

pub const QUAD_GROWTH: f32 = 0.003;

pub const QUAD_DROP: f64 = 0.6;

pub const FLIPBOOK_TEXTURE: usize = 0x21;

pub const FLIPBOOK_MS: u32 = 1000;

pub const SCALE_PER_MODE: f64 = 1.5;

pub const FLIPBOOK_FRAMES: u32 = 9;

pub const FLIPBOOK_FRAME_MS: u32 = 0x6F;

pub const FLIPBOOK_DROP: f64 = 0.5;

pub const SHADE_GRID: i32 = 4;

pub const SHADE_TEXTURE: usize = 7;

pub const SHADE_ARGB: u32 = 0x7070_4000;

pub const SHADE_MS: u32 = 1500;

pub const SOUND: u16 = 0xA6;

pub const SOUND_MODE_1: u16 = 0x9A;

pub const KIND_21_UV: [[f32; 2]; 4] = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];

pub fn widen_flipbook_uv(child: &mut Billboard) {
    if child.kind != FLIPBOOK_TEXTURE {
        return;
    }

    for (vertex, uv) in child.quad.iter_mut().zip(KIND_21_UV) {
        vertex.uv = uv;
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpeedUp {
    pub at: Vec3,
    pub mode: i32,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub gone: bool,
}

impl SpeedUp {
    pub fn new(at: Vec3, mode: i32, now_ms: u32) -> SpeedUp {
        SpeedUp {
            at,
            mode,
            birth_ms: now_ms,
            lifetime_ms: LIFETIME_MS,
            gone: false,
        }
    }

    pub const fn sound(mode: i32) -> u16 {
        match mode {
            1 => SOUND_MODE_1,
            _ => SOUND,
        }
    }
}

impl Effect for SpeedUp {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let elapsed = tick.now_ms.wrapping_sub(self.birth_ms);

        if elapsed as f32 / self.lifetime_ms as f32 > 1.0 {
            self.gone = true;
        }
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, _cx: &Submit<'_>, _out: &mut Sink<'_>) {}

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    pub quad: TimedQuad,
    pub flipbook: Billboard,
    pub shade: Shade,
    pub token: SpeedUp,
}

impl Built {
    pub fn place(&mut self, ground: &Ground<'_>) {
        self.shade.place(ground);
    }

    pub fn into_nodes(self) -> Vec<Box<dyn Effect>> {
        vec![
            Box::new(self.quad) as Box<dyn Effect>,
            Box::new(self.flipbook),
            Box::new(self.shade),
            Box::new(self.token),
        ]
    }

    pub fn describe(&self) -> String {
        format!(
            "NWYD: SkillSpeedUp mode {} — 4 node(s): quad {:.3}, flipbook {:.3} x{} frames, \
             shade {} ms, sound {:#04X} (named, not played)",
            self.token.mode,
            self.quad.quad.base_scale.x,
            self.flipbook.scale.x,
            self.flipbook.frames,
            self.shade.lifetime_ms,
            SpeedUp::sound(self.token.mode),
        )
    }
}

pub fn build_at(at: Vec3, mode: i32, now_ms: u32) -> Built {
    Built {
        quad: ground_quad(at, mode),
        flipbook: flipbook(at, mode),
        shade: Shade::new(SHADE_GRID, SHADE_TEXTURE, SHADE_ARGB, at, SECTOR_UNASSIGNED)
            .with_lifetime(SHADE_MS),
        token: SpeedUp::new(at, mode, now_ms),
    }
}

fn ground_quad(at: Vec3, mode: i32) -> TimedQuad {
    let scale = (f64::from(mode) * SCALE_PER_MODE + QUAD_SCALE_BASE) as f32;

    TimedQuad::new(
        GroundQuad {
            owner: 0,
            slot: Slot::Placed,
            texture: QUAD_TEXTURE,
            pass: BillboardPass::Additive,
            anchor: Vec3::new(at.x, (f64::from(at.y) - QUAD_DROP) as f32, at.z),
            base_scale: Vec3::splat(scale),
            grow: QUAD_GROWTH,
            spin_period: 0,
            ordinal: 0,
            argb: 0xFFFF_FFFF,
        },
        QUAD_MS,
    )
}

fn flipbook(at: Vec3, mode: i32) -> Billboard {
    let scale = (f64::from(mode) * SCALE_PER_MODE + SCALE_PER_MODE) as f32;

    let mut child = Billboard::new(
        FLIPBOOK_TEXTURE,
        FLIPBOOK_MS,
        Vec3::splat(scale),
        0.0,
        FLIPBOOK_FRAMES,
        FLIPBOOK_FRAME_MS,
    );

    child.pass = BillboardPass::Additive;

    child.colour_mode = 0;

    child.position = Vec3::new(at.x, (f64::from(at.y) - FLIPBOOK_DROP) as f32, at.z);

    widen_flipbook_uv(&mut child);

    child
}

pub fn mode_of(spawn: &Spawn) -> Option<i32> {
    match spawn.arguments {
        [
            SpawnArgument::Allocation(_),
            SpawnArgument::Pos(Axis::X),
            SpawnArgument::Pos(Axis::Y),
            SpawnArgument::Pos(Axis::Z),
            mode,
        ] => match mode {
            SpawnArgument::Int(n) => Some(*n),
            SpawnArgument::Bits(b) => Some(*b as i32),
            _ => None,
        },
        _ => None,
    }
}

pub fn build(
    spawn: &Spawn,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let Some(at) = cx.at.at(spawn.at) else {
        return Err(Declined::Unplaced);
    };

    let Some(mode) = mode_of(spawn) else {
        return Err(Declined::Arguments);
    };

    let mut built = build_at(at, mode, cx.now_ms);

    if let Some(ground) = env.ground {
        built.place(ground);
    }

    out.extend(built.into_nodes());

    Ok(())
}
