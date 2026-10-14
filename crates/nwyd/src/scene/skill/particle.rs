use super::shade_decal::{as_f32, as_u32};
use crate::scene::burst::{self, Recipe as Burst};
use crate::scene::effect_tree::Effect;
use crate::scene::lod::DetailGate;
use crate::scene::rng::Lcg;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Axis, PositionRef, Spawn, SpawnArgument};
use glam::Vec3;

pub const SPAWN_ARGUMENT_COUNT: usize = 12;

pub const OBJECT_SIZE: u16 = 156;

pub const UNBUILT_KIND: i32 = 5;

pub const TAIL: Burst = Burst::MEASURED_TAIL;

pub const GATE: DetailGate = DetailGate::Always;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Order {
    pub at: PositionRef,
    pub recipe: Burst,
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::EffectParticle || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    for (i, axis) in [Axis::X, Axis::Y, Axis::Z].iter().enumerate() {
        match spawn.arguments[1 + i] {
            SpawnArgument::Pos(a) if a == *axis => {}
            _ => return None,
        }
    }

    let kind = as_u32(spawn.arguments[4])? as i32;

    if kind == UNBUILT_KIND {
        return None;
    }

    let recipe = Burst {
        at: Vec3::ZERO,
        kind,
        count: as_u32(spawn.arguments[5])? as i32,
        spread: as_f32(spawn.arguments[6])?,
        argb: as_u32(spawn.arguments[7])?,
        random_colour: as_u32(spawn.arguments[8])? != 0,
        texture: as_u32(spawn.arguments[9])? as usize,
        scatter: as_f32(spawn.arguments[10])?,
        additive: as_u32(spawn.arguments[11])? == 1,
        ..TAIL
    };

    Some(Order {
        at: spawn.at,
        recipe,
    })
}

#[derive(Debug, Clone, PartialEq)]

pub struct Placed {
    pub emitter: burst::Particle,
    pub children: Vec<crate::scene::effects::Billboard>,
}

impl Placed {
    pub fn into_effects(self) -> Vec<Box<dyn Effect>> {
        let mut out: Vec<Box<dyn Effect>> = Vec::with_capacity(self.children.len() + 1);

        for child in self.children {
            out.push(Box::new(child));
        }

        out.push(Box::new(self.emitter));

        out
    }
}

pub fn build(order: &Order, at: Vec3, now_ms: u32, rng: &mut Lcg) -> Placed {
    let (emitter, children) = burst::build(&Burst { at, ..order.recipe }, now_ms, rng);

    Placed { emitter, children }
}

pub fn build_spawn(
    spawn: &Spawn,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let Some(order) = read(spawn) else {
        return Err(Declined::Arguments);
    };

    let Some(at) = cx.at.at(order.at) else {
        return Err(Declined::Unplaced);
    };

    if !GATE.admits(cx.detail) {
        return Ok(());
    }

    out.extend(build(&order, at, cx.now_ms, env.rng).into_effects());

    Ok(())
}
