use crate::scene::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use crate::scene::effects::Billboard;
use crate::scene::lod::DetailGate;
use crate::scene::rng::Lcg;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::{Spawn, SpawnArgument};
use glam::Vec3;
use render::billboard::BillboardPass;

pub const CLASS: Class = Class {
    name: "SkillPoison",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const LIFETIME_MS: u32 = 3000;

pub const UNUSED_FIELD_98: i32 = 0;

pub const CHILD_TEXTURE: usize = 0;

pub const CHILD_FIRST_MS: u32 = 1500;

pub const CHILD_STEP_MS: u32 = 400;

pub const CHILD_GROWTH: f32 = f32::from_bits(0x3A83_126F);

pub const MOTE_STEP: f32 = 0.1;

pub const MOTE_BASE: f32 = 0.2;

pub const SHEET_BIAS: i32 = 5;

pub const SHEET_STEP: f32 = 4.0;

pub const SHEET_BASE: f32 = 1.0;

pub const SCALE_SPAN: u32 = 5;

pub const JITTER_SPAN: u32 = 10;

pub const JITTER_BIAS: i32 = 5;

pub const JITTER_STEP: f32 = 0.1;

pub const SOUND: i64 = 0x9C;

pub const SOUND_ON: i32 = 1;

const CHILD_FRAMES: u32 = 1;

const CHILD_FRAME_MS: u32 = 0x50;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Recipe {
    pub at: Vec3,
    pub argb: u32,
    pub count: i32,
    pub play_sound: i32,
    pub style: i32,
}

impl Recipe {
    pub const SHIPPED: Recipe = Recipe {
        at: Vec3::ZERO,
        argb: 0xFFFF_FFFF,
        count: 10,
        play_sound: SOUND_ON,
        style: 0,
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Poison {
    pub at: Vec3,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub gone: bool,
}

impl Poison {
    pub fn new(at: Vec3, now_ms: u32) -> Poison {
        Poison {
            at,
            birth_ms: now_ms,
            lifetime_ms: LIFETIME_MS,
            gone: false,
        }
    }
}

impl Effect for Poison {
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
    pub billboards: Vec<Billboard>,
    pub token: Poison,
    pub sound: Option<i64>,
    pub detail_gated: bool,
}

impl Built {
    pub fn nodes(&self) -> usize {
        self.billboards.len() + 1
    }
}

pub fn build(recipe: &Recipe, now_ms: u32, rng: &mut Lcg, detail: i32) -> Built {
    let admitted = DetailGate::Full.admits(detail);

    let mut billboards = Vec::new();

    if admitted && recipe.count > 0 {
        let mut life = CHILD_FIRST_MS;

        for _ in 0..recipe.count {
            billboards.push(child(recipe, life, rng));

            life += CHILD_STEP_MS;
        }
    }

    Built {
        billboards,
        token: Poison::new(recipe.at, now_ms),
        sound: (recipe.play_sound == SOUND_ON).then_some(SOUND),
        detail_gated: !admitted,
    }
}

fn child(recipe: &Recipe, life: u32, rng: &mut Lcg) -> Billboard {
    let draw = (rng.next() % SCALE_SPAN) as i32;

    let scale = match recipe.style {
        0 => draw as f32 * MOTE_STEP + MOTE_BASE,
        _ => (draw + SHEET_BIAS) as f32 * SHEET_STEP + SHEET_BASE,
    };

    let mut child = Billboard::new(
        CHILD_TEXTURE,
        life,
        Vec3::splat(scale),
        CHILD_GROWTH,
        CHILD_FRAMES,
        CHILD_FRAME_MS,
    );

    let jitter = |rng: &mut Lcg| (rng.next() % JITTER_SPAN) as i32 - JITTER_BIAS;

    let x = jitter(rng) as f32 * JITTER_STEP + recipe.at.x;

    let z = jitter(rng) as f32 * JITTER_STEP + recipe.at.z;

    child.position = Vec3::new(x, recipe.at.y, z);

    child.pass = BillboardPass::Additive;

    child.lift = true;

    child.set_color(recipe.argb);

    child
}

pub fn push(built: Built, out: &mut Vec<Box<dyn Effect>>) {
    let Built {
        billboards,
        token,
        sound: _,
        detail_gated: _,
    } = built;

    for child in billboards {
        out.push(Box::new(child));
    }

    out.push(Box::new(token));
}

pub fn build_into(
    spawn: &Spawn,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let recipe = recipe_of(spawn)?;

    let Some(at) = cx.at.at(spawn.at) else {
        return Err(Declined::Unplaced);
    };

    push(
        build(&Recipe { at, ..recipe }, cx.now_ms, env.rng, cx.detail),
        out,
    );

    Ok(())
}

pub fn recipe_of(spawn: &Spawn) -> Result<Recipe, Declined> {
    let [
        SpawnArgument::Allocation(_),
        SpawnArgument::Pos(_),
        SpawnArgument::Pos(_),
        SpawnArgument::Pos(_),
        argb,
        SpawnArgument::Int(count),
        SpawnArgument::Int(play_sound),
        SpawnArgument::Int(style),
    ] = spawn.arguments
    else {
        return Err(Declined::Arguments);
    };

    let argb = match argb {
        SpawnArgument::Bits(bits) => *bits,
        SpawnArgument::Int(n) => *n as u32,
        _ => return Err(Declined::Arguments),
    };

    Ok(Recipe {
        at: Vec3::ZERO,
        argb,
        count: *count,
        play_sound: *play_sound,
        style: *style,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Raised {
    pub nodes: usize,
    pub billboards: usize,
    pub detail_gated: bool,
    pub sound: Option<i64>,
}

pub fn describe(raised: &Raised) -> String {
    let gated = match raised.detail_gated {
        true => " (the effect detail setting closed the loop — invisible on purpose, not unbuilt)",
        false => "",
    };

    format!(
        "NWYD: SkillPoison — {} node(s), {} billboard(s), {} sound(s){gated}",
        raised.nodes,
        raised.billboards,
        usize::from(raised.sound.is_some()),
    )
}
