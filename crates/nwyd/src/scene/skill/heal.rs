use crate::scene::burst::{self, Particle};
use crate::scene::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use crate::scene::effects::{Billboard, SECTOR_UNASSIGNED};
use crate::scene::lod::{DetailGate, Ladder};
use crate::scene::rng::Lcg;
use crate::scene::shade::{Ground, Shade};
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::Spawn;
use glam::Vec3;

pub const CLASS: Class = Class {
    name: "SkillHeal",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const LIFETIME_MS: u32 = 2000;

pub const PARTICLES: Ladder = Ladder {
    rungs: &[(10.0, 2), (20.0, 5), (30.0, 8)],
    default: 12,
};

pub const PARTICLE_GATE: DetailGate = DetailGate::Full;

pub const PARTICLE_KIND: i32 = 0;

pub const PARTICLE_SPREAD: f32 = 0.1;

pub const PARTICLE_ARGB: u32 = 0xFF77_AAFF;

pub const PARTICLE_TEXTURE: usize = 0x38;

pub const PARTICLE_MS: u32 = 1000;

pub const SHADE_GRID: i32 = 4;

pub const SHADE_TEXTURE: usize = 7;

pub const SHADE_ARGB: u32 = 0x0011_5588;

pub const SHADE_MS: u32 = 3000;

pub const SOUND: i64 = 0x9E;

pub const SPAWN_ARGUMENT_COUNT: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Heal {
    pub at: Vec3,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub gone: bool,
}

impl Heal {
    pub fn new(at: Vec3, now_ms: u32) -> Heal {
        Heal {
            at,
            birth_ms: now_ms,
            lifetime_ms: LIFETIME_MS,
            gone: false,
        }
    }
}

impl Effect for Heal {
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

pub fn particle_recipe(at: Vec3, count: i32) -> burst::Recipe {
    burst::Recipe {
        at,
        kind: PARTICLE_KIND,
        count,
        spread: PARTICLE_SPREAD,
        argb: PARTICLE_ARGB,
        random_colour: false,
        ..burst::Recipe::MEASURED_TAIL
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    pub token: Heal,
    pub emitter: Option<Particle>,
    pub particles: Vec<Billboard>,
    pub shade: Shade,
    pub shade_placed: bool,
}

impl Built {
    pub fn nodes(&self) -> usize {
        self.particles.len() + usize::from(self.emitter.is_some()) + 2
    }

    pub fn into_effects(self) -> Vec<Box<dyn Effect>> {
        let Built {
            token,
            emitter,
            particles,
            shade,
            ..
        } = self;

        let mut out: Vec<Box<dyn Effect>> = Vec::with_capacity(particles.len() + 3);

        out.extend(
            particles
                .into_iter()
                .map(|c| Box::new(c) as Box<dyn Effect>),
        );

        if let Some(emitter) = emitter {
            out.push(Box::new(emitter));
        }

        out.push(Box::new(shade));

        out.push(Box::new(token));

        out
    }
}

pub fn build(
    at: Vec3,
    now_ms: u32,
    rate: f32,
    detail: i32,
    rng: &mut Lcg,
    ground: Option<&Ground<'_>>,
) -> Built {
    let token = Heal::new(at, now_ms);

    let count = PARTICLES.at(rate);

    let (emitter, particles) = match PARTICLE_GATE.admits(detail) {
        true => {
            let (emitter, children) = burst::build(&particle_recipe(at, count), now_ms, rng);

            (Some(emitter), children)
        }
        false => (None, Vec::new()),
    };

    let mut shade = Shade::new(SHADE_GRID, SHADE_TEXTURE, SHADE_ARGB, at, SECTOR_UNASSIGNED)
        .with_lifetime(SHADE_MS);

    let shade_placed = match ground {
        Some(ground) => {
            shade.place(ground);

            true
        }
        None => false,
    };

    Built {
        token,
        emitter,
        particles,
        shade,
        shade_placed,
    }
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

    let built = build(at, cx.now_ms, cx.rate, cx.detail, env.rng, env.ground);

    out.extend(built.into_effects());

    Ok(())
}
