use crate::scene::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use crate::scene::effects::Billboard;
use crate::scene::lod::{DetailGate, Ladder};
use crate::scene::rng::Lcg;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::Spawn;
use glam::Vec3;
use render::billboard::BillboardPass;

pub const CLASS: Class = Class {
    name: "SkillHeavensDust",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const LIFETIME_MS: u32 = 1000;

pub const BILLBOARDS: Ladder = Ladder {
    rungs: &[(10.0, 2), (20.0, 5), (30.0, 8)],
    default: 10,
};

pub const BILLBOARD_GATE: DetailGate = DetailGate::Always;

pub const MIN_COUNT: i32 = 1;

pub const FIRST_MS: u32 = 1500;

pub const STEP_MS: u32 = 400;

pub const TEXTURE: usize = 0;

pub const GROWTH: f32 = f32::from_bits(0x3A83_126F);

pub const FRAMES: u32 = 1;

pub const FRAME_MS: u32 = 0x50;

pub const SCALE_STEP: f32 = 0.5;

pub const SCALE_BASE: f32 = 0.8;

pub const SCALE_SPAN: u32 = 5;

pub const JITTER_SPAN: u32 = 10;

pub const JITTER_BIAS: i32 = 5;

pub const JITTER_XZ: f32 = 0.2;

pub const ARGB: u32 = 0xFFAA_EEFF;

pub const SOUND: i64 = 0x99;

pub const SPAWN_ARGUMENT_COUNT: usize = 5;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct HeavensDust {
    pub at: Vec3,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub gone: bool,
}

impl HeavensDust {
    pub fn new(at: Vec3, now_ms: u32) -> HeavensDust {
        HeavensDust {
            at,
            birth_ms: now_ms,
            lifetime_ms: LIFETIME_MS,
            gone: false,
        }
    }
}

impl Effect for HeavensDust {
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
    pub token: HeavensDust,
    pub billboards: Vec<Billboard>,
}

impl Built {
    pub fn nodes(&self) -> usize {
        self.billboards.len() + 1
    }

    pub fn into_effects(self) -> Vec<Box<dyn Effect>> {
        let Built { token, billboards } = self;

        let mut out: Vec<Box<dyn Effect>> = Vec::with_capacity(billboards.len() + 1);

        out.extend(
            billboards
                .into_iter()
                .map(|c| Box::new(c) as Box<dyn Effect>),
        );

        out.push(Box::new(token));

        out
    }
}

pub fn build(at: Vec3, now_ms: u32, rate: f32, rng: &mut Lcg) -> Built {
    let token = HeavensDust::new(at, now_ms);

    let count = BILLBOARDS.at(rate);

    let mut billboards = Vec::new();

    if count >= MIN_COUNT {
        let mut life = FIRST_MS;

        for _ in 0..count {
            billboards.push(child(at, life, rng));

            life += STEP_MS;
        }
    }

    Built { token, billboards }
}

fn child(at: Vec3, life_ms: u32, rng: &mut Lcg) -> Billboard {
    let scale = (rng.next() % SCALE_SPAN) as f32 * SCALE_STEP + SCALE_BASE;

    let mut child = Billboard::new(
        TEXTURE,
        life_ms,
        Vec3::splat(scale),
        GROWTH,
        FRAMES,
        FRAME_MS,
    );

    let jitter = |rng: &mut Lcg| (rng.next() % JITTER_SPAN) as i32 - JITTER_BIAS;

    let x = jitter(rng) as f32 * JITTER_XZ + at.x;

    let z = jitter(rng) as f32 * JITTER_XZ + at.z;

    child.position = Vec3::new(x, at.y, z);

    child.pass = BillboardPass::Additive;

    child.lift = true;

    child.set_color(ARGB);

    child
}

pub fn recipe(
    spawn: &Spawn,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    if spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return Err(Declined::Arguments);
    }

    let Some(at) = cx.at.at(spawn.at) else {
        return Err(Declined::Unplaced);
    };

    let built = build(at, cx.now_ms, cx.rate, env.rng);

    out.extend(built.into_effects());

    Ok(())
}
