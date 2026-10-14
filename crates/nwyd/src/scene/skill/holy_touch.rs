use crate::scene::burst::{self, Particle};
use crate::scene::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use crate::scene::effects::{Billboard, SECTOR_UNASSIGNED};
use crate::scene::field::FieldScene;
use crate::scene::quads::{GroundQuad, Slot};
use crate::scene::rng::Lcg;
use crate::scene::shade::{Ground, Shade};
use crate::scene::timers::TimedQuad;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::{Spawn, SpawnArgument};
use glam::Vec3;
use render::billboard::BillboardPass;

pub const CLASS: Class = Class {
    name: "SkillHolyTouch",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const LIFETIME_MS: u32 = 1000;

pub const FULL_MODE: i32 = 0;

pub const BILLBOARD_TEXTURE: usize = 0x36;

pub const BILLBOARD_FIRST_MS: u32 = 1500;

pub const BILLBOARD_STEP_MS: u32 = 400;

pub const BILLBOARD_LIMIT_MS: u32 = 3100;

pub const BILLBOARD_SCALE: f32 = 0.8;

pub const OFFSET_X: [i32; 4] = [-2, 0, -1, -1];

pub const OFFSET_Z: [i32; 4] = [0, -1, 0, -1];

pub const OFFSET_STEP: f32 = 0.5;

pub const BILLBOARD_DROP: f32 = 1.0;

pub const GROWTH_Y_BASE: f32 = 0.002;

pub const GROWTH_Y_STEP: f32 = 0.001;

pub const QUAD_ONE_TEXTURE: usize = 0x37;

pub const QUAD_ONE_SCALE: f32 = 0.01;

pub const QUAD_ONE_GROWTH: f32 = f32::from_bits(0x3B44_9BA6);

pub const QUAD_ONE_LIFT: f32 = 0.1;

pub const QUAD_TWO_TEXTURE: usize = 2;

pub const QUAD_TWO_SCALE: f32 = 2.0;

pub const QUAD_TWO_GROWTH: f32 = f32::from_bits(0x3B03_126F);

pub const QUAD_TWO_LIFT: f32 = 0.3;

pub const QUAD_MS: u32 = 2000;

pub const SHADE_GRID: i32 = 4;

pub const SHADE_TEXTURE: usize = 7;

pub const SHADE_MS: u32 = 3000;

pub const SHADE_UNTINTED: u32 = 0xFFFF_FFFF;

pub const PARTICLE_KIND: i32 = 0;

pub const PARTICLE_COUNT: i32 = 10;

pub const PARTICLE_SPREAD: f32 = 0.05;

pub const PARTICLE_ARGB: u32 = 0xFFFF_FFFF;

pub const SOUND: i64 = 0x9F;

pub const SOUND_MODE_1: i64 = 0x9E;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModeTail {
    pub quad_two: Option<u32>,
    pub shade: Option<u32>,
    pub particle: bool,
    pub sound: i64,
}

impl ModeTail {
    pub const fn of(mode: i32) -> ModeTail {
        match mode {
            0 => ModeTail {
                quad_two: Some(0xFFAA_AAAA),
                shade: Some(0x5555_5555),
                particle: true,
                sound: SOUND,
            },
            1 => ModeTail {
                quad_two: Some(0xFFAA_77FF),
                shade: Some(0x5555_3388),
                particle: false,
                sound: SOUND_MODE_1,
            },
            2 => ModeTail {
                quad_two: Some(0xFFAA_0000),
                shade: Some(0x5588_3333),
                particle: true,
                sound: SOUND,
            },
            _ => ModeTail {
                quad_two: None,
                shade: None,
                particle: false,
                sound: SOUND,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HolyTouch {
    pub at: Vec3,
    pub mode: i32,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub gone: bool,
}

impl HolyTouch {
    pub fn new(at: Vec3, mode: i32, now_ms: u32) -> HolyTouch {
        HolyTouch {
            at,
            mode,
            birth_ms: now_ms,
            lifetime_ms: LIFETIME_MS,
            gone: false,
        }
    }
}

impl Effect for HolyTouch {
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
    pub quads: Vec<TimedQuad>,
    pub shade: Shade,
    pub particle: Option<(Particle, Vec<Billboard>)>,
    pub token: HolyTouch,
    pub sound: i64,
}

impl Built {
    pub fn nodes(&self) -> usize {
        let puff = self.particle.as_ref().map_or(0, |(_, kids)| kids.len() + 1);

        self.billboards.len() + self.quads.len() + puff + 2
    }
}

pub fn build(at: Vec3, mode: i32, now_ms: u32, rng: &mut Lcg) -> Built {
    let tail = ModeTail::of(mode);

    let mut billboards = Vec::new();

    let mut quads = Vec::new();

    if mode == FULL_MODE {
        let mut life = BILLBOARD_FIRST_MS;

        let mut i = 0usize;

        while life < BILLBOARD_LIMIT_MS {
            billboards.push(billboard(at, i, life));

            i += 1;

            life += BILLBOARD_STEP_MS;
        }

        quads.push(ground_quad(
            at,
            QUAD_ONE_TEXTURE,
            QUAD_ONE_SCALE,
            QUAD_ONE_GROWTH,
            QUAD_ONE_LIFT,
            SHADE_UNTINTED,
            0,
        ));
    }

    quads.push(ground_quad(
        at,
        QUAD_TWO_TEXTURE,
        QUAD_TWO_SCALE,
        QUAD_TWO_GROWTH,
        QUAD_TWO_LIFT,
        tail.quad_two.unwrap_or(SHADE_UNTINTED),
        quads.len(),
    ));

    let shade = Shade::new(
        SHADE_GRID,
        SHADE_TEXTURE,
        tail.shade.unwrap_or(SHADE_UNTINTED),
        at,
        SECTOR_UNASSIGNED,
    )
    .with_lifetime(SHADE_MS);

    let particle = tail
        .particle
        .then(|| burst::build(&particle_recipe(at), now_ms, rng));

    Built {
        billboards,
        quads,
        shade,
        particle,
        token: HolyTouch::new(at, mode, now_ms),
        sound: tail.sound,
    }
}

fn billboard(at: Vec3, i: usize, life: u32) -> Billboard {
    let mut child = Billboard::new(
        BILLBOARD_TEXTURE,
        life,
        Vec3::splat(BILLBOARD_SCALE),
        0.0,
        CHILD_FRAMES,
        CHILD_FRAME_MS,
    );

    let (dx, dz) = (OFFSET_X[i % 4], OFFSET_Z[i % 4]);

    child.position = Vec3::new(
        at.x + dx as f32 * OFFSET_STEP,
        at.y - BILLBOARD_DROP,
        at.z + dz as f32 * OFFSET_STEP,
    );

    child.growth.y = i as f32 * GROWTH_Y_STEP + GROWTH_Y_BASE;

    child.pass = BillboardPass::Additive;

    child.lift = true;

    child
}

fn ground_quad(
    at: Vec3,
    texture: usize,
    scale: f32,
    grow: f32,
    lift: f32,
    argb: u32,
    ordinal: usize,
) -> TimedQuad {
    TimedQuad::new(
        GroundQuad {
            owner: 0,
            slot: Slot::Placed,
            texture,
            pass: BillboardPass::Additive,
            anchor: Vec3::new(at.x, at.y + lift, at.z),
            base_scale: Vec3::splat(scale),
            grow,
            spin_period: 0,
            ordinal,
            argb,
        },
        QUAD_MS,
    )
}

pub fn particle_recipe(at: Vec3) -> burst::Recipe {
    burst::Recipe {
        at,
        kind: PARTICLE_KIND,
        count: PARTICLE_COUNT,
        spread: PARTICLE_SPREAD,
        argb: PARTICLE_ARGB,
        random_colour: false,
        ..burst::Recipe::MEASURED_TAIL
    }
}

const CHILD_FRAMES: u32 = 1;

const CHILD_FRAME_MS: u32 = 0x50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raised {
    pub mode: i32,
    pub nodes: usize,
    pub billboards: usize,
    pub quads: usize,
    pub particle: bool,
    pub sound: i64,
}

pub fn spawn(scene: &mut FieldScene, at: Vec3, mode: i32) -> Raised {
    let now_ms = scene.time.now_ms;

    let built = build(at, mode, now_ms, &mut scene.world.actors.effect_rng);

    let raised = Raised {
        mode,
        nodes: built.nodes(),
        billboards: built.billboards.len()
            + built.particle.as_ref().map_or(0, |(_, kids)| kids.len()),
        quads: built.quads.len(),
        particle: built.particle.is_some(),
        sound: built.sound,
    };

    let heights = &scene.world.actors.heights;

    let mut zones: Vec<&formats::world::trn::Terrain> = vec![&scene.world.terrain];

    zones.extend(scene.world.streamed.iter().map(|s| &s.terrain));

    let sound = built.sound;

    let mut out: Vec<Box<dyn Effect>> = Vec::new();

    push(
        built,
        Some(&Ground::new(&scene.world.terrain, heights, zones)),
        &mut out,
    );

    for node in out {
        scene.world.effects.spawn(node);
    }

    scene.world.play_sound(sound);

    raised
}

pub fn push(built: Built, ground: Option<&Ground<'_>>, out: &mut Vec<Box<dyn Effect>>) {
    let Built {
        billboards,
        quads,
        mut shade,
        particle,
        token,
        sound: _,
    } = built;

    for child in billboards {
        out.push(Box::new(child));
    }

    for quad in quads {
        out.push(Box::new(quad));
    }

    if let Some(ground) = ground {
        shade.place(ground);
    }

    out.push(Box::new(shade));

    if let Some((emitter, children)) = particle {
        for child in children {
            out.push(Box::new(child));
        }

        out.push(Box::new(emitter));
    }

    out.push(Box::new(token));
}

pub fn build_into(
    spawn: &Spawn,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let mode = mode_of(spawn)?;

    let Some(at) = cx.at.at(spawn.at) else {
        return Err(Declined::Unplaced);
    };

    let built = build(at, mode, cx.now_ms, env.rng);

    push(built, env.ground, out);

    Ok(())
}

pub fn mode_of(spawn: &Spawn) -> Result<i32, Declined> {
    match spawn.arguments {
        [
            SpawnArgument::Allocation(_),
            SpawnArgument::Pos(_),
            SpawnArgument::Pos(_),
            SpawnArgument::Pos(_),
            SpawnArgument::Int(mode),
        ] => Ok(*mode),
        _ => Err(Declined::Arguments),
    }
}

pub fn describe(raised: &Raised) -> String {
    let puff = match raised.particle {
        true => "",
        false => " (no puff — the mode tail skips it, not the detail gate)",
    };

    format!(
        "NWYD: SkillHolyTouch mode {} — {} node(s), {} billboard(s), {} ground quad(s), \
         1 shade, sound {:#04X}{puff}",
        raised.mode, raised.nodes, raised.billboards, raised.quads, raised.sound,
    )
}
