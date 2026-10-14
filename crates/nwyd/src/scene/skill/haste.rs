use crate::scene::effect_tree::{
    Base, Class, Cull, Effect, Pass, Sink, Submit, Tick, VISUAL_EFFECT_METHOD_COUNT,
};
use crate::scene::effects::{Billboard, Motion};
use crate::scene::lod::DetailGate;
use crate::scene::rng::Lcg;
use crate::scene::vfx::{Ambient, Cx, Declined, Recipe};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Spawn, SpawnArgument};
use glam::Vec3;
use render::billboard::BillboardPass;

pub const CLASS: Class = Class {
    name: "SkillHaste",
    method_count: VISUAL_EFFECT_METHOD_COUNT,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const SPAWN_ARGUMENT_COUNT: usize = 5;

pub const LIFETIME_MS: u32 = 3000;

pub const SOUND: u16 = 0x9F;

pub const COLOURS: [u32; 5] = [
    0xFFFF_0000,
    0xFFAA_00FF,
    0xFF00_AA00,
    0xFFFF_EEFF,
    0xFFFF_AA00,
];

pub const SWIRL_TEXTURE: usize = 0x38;

pub const SWIRL_FIRST_MS: u32 = 2000;

pub const SWIRL_STEP_MS: u32 = 300;

pub const SWIRL_LIMIT_MS: u32 = 6500;

pub const SWIRL_SCALE_STEP: f32 = 0.1;

pub const SWIRL_AMP_STEP: f32 = 0.05;

pub const SWIRL_AMP_BASE: f32 = 0.1;

pub const SWIRL_RISE: f32 = -7.0;

pub const SWIRL_LIFT: f32 = 5.0;

pub const JITTER_SPAN: u32 = 5;

pub const JITTER_BIAS: i32 = 3;

pub const JITTER_STEP: f32 = 0.1;

pub const PILLAR_TEXTURE: usize = 0x34;

pub const PILLAR_TEXTURE_MODE_1: usize = 0x33;

pub const PILLAR_TEXTURE_TINTED: usize = 0x7A;

pub const PILLAR_MS: u32 = 4000;

pub const PILLAR_SCALE: Vec3 = Vec3::new(1.5, 10.0, 1.5);

pub const PILLAR_ROLL: f32 = 2.792_527;

pub const PILLAR_LIFT: f32 = 2.0;

pub const PILLAR_TINTED_MODES: [i32; 3] = [2, 3, 4];

pub const SWIRL_DETAIL: DetailGate = DetailGate::Full;

pub const PILLAR_DETAIL: DetailGate = DetailGate::Always;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Haste {
    pub at: Vec3,
    pub mode: i32,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub gone: bool,
}

impl Haste {
    pub fn new(at: Vec3, mode: i32, now_ms: u32) -> Haste {
        Haste {
            at,
            mode,
            birth_ms: now_ms,
            lifetime_ms: LIFETIME_MS,
            gone: false,
        }
    }
}

impl Effect for Haste {
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

pub fn colour(mode: i32) -> Option<u32> {
    usize::try_from(mode)
        .ok()
        .and_then(|m| COLOURS.get(m).copied())
}

pub fn texture(mode: i32) -> usize {
    if mode == 1 {
        return PILLAR_TEXTURE_MODE_1;
    }

    if PILLAR_TINTED_MODES.contains(&mode) {
        return PILLAR_TEXTURE_TINTED;
    }

    PILLAR_TEXTURE
}

#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    pub swirl: Vec<Billboard>,
    pub pillar: Billboard,
    pub token: Haste,
    pub detail_gated: bool,
}

impl Built {
    pub fn nodes(&self) -> usize {
        self.swirl.len() + 2
    }

    pub fn push_into(self, out: &mut Vec<Box<dyn Effect>>) {
        let Built {
            swirl,
            pillar,
            token,
            ..
        } = self;

        for child in swirl {
            out.push(Box::new(child));
        }

        out.push(Box::new(pillar));

        out.push(Box::new(token));
    }
}

pub fn build(at: Vec3, mode: i32, now_ms: u32, rng: &mut Lcg, detail: i32) -> Built {
    let mut swirl = Vec::new();

    let gated = !SWIRL_DETAIL.admits(detail);

    if !gated {
        let mut life = SWIRL_FIRST_MS;

        let mut i = 0usize;

        while life < SWIRL_LIMIT_MS {
            swirl.push(swirl_child(at, mode, i, life, rng));

            i += 1;

            life += SWIRL_STEP_MS;
        }
    }

    let token = Haste::new(at, mode, now_ms);

    Built {
        swirl,
        pillar: pillar(at, mode),
        token,
        detail_gated: gated,
    }
}

fn swirl_child(at: Vec3, mode: i32, i: usize, life: u32, rng: &mut Lcg) -> Billboard {
    let scale = (i & 1) as f32 * SWIRL_SCALE_STEP + SWIRL_SCALE_STEP;

    let mut child = Billboard::new(
        SWIRL_TEXTURE,
        life,
        Vec3::splat(scale),
        0.0,
        CHILD_FRAMES,
        CHILD_FRAME_MS,
    );

    child.pass = BillboardPass::Additive;

    child.motion = Motion::Circle;

    child.amp_h = (i % 3) as f32 * SWIRL_AMP_STEP + SWIRL_AMP_BASE;

    child.rise = SWIRL_RISE;

    let d = ((rng.next() % JITTER_SPAN) as i32 - JITTER_BIAS) as f32 * JITTER_STEP;

    let placed = Vec3::new(at.x + d, at.y + d + SWIRL_LIFT, at.z + d);

    child.origin = placed;

    child.position = placed;

    if let Some(argb) = colour(mode) {
        child.set_color(argb);
    }

    child
}

fn pillar(at: Vec3, mode: i32) -> Billboard {
    let mut child = Billboard::new(
        texture(mode),
        PILLAR_MS,
        PILLAR_SCALE,
        0.0,
        CHILD_FRAMES,
        CHILD_FRAME_MS,
    );

    child.pass = BillboardPass::Additive;

    child.roll = PILLAR_ROLL;

    let placed = Vec3::new(at.x, at.y + PILLAR_LIFT, at.z);

    child.origin = placed;

    child.position = placed;

    if let Some(argb) = colour(mode).filter(|_| PILLAR_TINTED_MODES.contains(&mode)) {
        child.set_color(argb);
    }

    child
}

pub const CHILD_FRAMES: u32 = 1;

pub const CHILD_FRAME_MS: u32 = 0x50;

pub fn stream(now_ms: u32) -> Lcg {
    Lcg::new(now_ms)
}

pub const RECIPE: Recipe = Recipe {
    class: SkillClass::SkillHaste,
    build: build_spawn,
};

pub fn build_spawn(
    spawn: &Spawn,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    if spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return Err(Declined::Arguments);
    }

    let mode = match spawn.arguments[SPAWN_ARGUMENT_COUNT - 1] {
        SpawnArgument::Int(n) => n,
        SpawnArgument::Bits(n) => n as i32,
        _ => return Err(Declined::Arguments),
    };

    let Some(at) = cx.at.at(spawn.at) else {
        return Err(Declined::Unplaced);
    };

    build(at, mode, cx.now_ms, env.rng, cx.detail).push_into(out);

    Ok(())
}

pub fn describe(built: &Built) -> String {
    let gated = match built.detail_gated {
        true => " (the effect detail setting closed the swirl)",
        false => "",
    };

    format!(
        "NWYD: SkillHaste mode {} — {} node(s), {} swirl sprite(s), 1 pillar (texture {:#04X}), \
         sound {:#04X}{gated}",
        built.token.mode,
        built.nodes(),
        built.swirl.len(),
        built.pillar.kind,
        SOUND,
    )
}
