use crate::scene::aura::Detail;
use crate::scene::effect_tree::Effect;
use crate::scene::lod::DetailGate;
use crate::scene::rng::Lcg;
use crate::scene::shade::Ground;
use crate::scene::timers;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Axis, PositionRef, Spawn, SpawnArgument};
use glam::Vec3;

pub const SPAWN_ARGUMENT_COUNT: usize = 5;

pub const OBJECT_SIZE: u16 = 156;

pub const BILLBOARD_GATE: DetailGate = DetailGate::Full;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Order {
    pub at: PositionRef,
    pub kind: i32,
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::EffectLevelUp || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    for (i, axis) in [Axis::X, Axis::Y, Axis::Z].iter().enumerate() {
        match spawn.arguments[1 + i] {
            SpawnArgument::Pos(a) if a == *axis => {}
            _ => return None,
        }
    }

    let kind = match spawn.arguments[4] {
        SpawnArgument::Int(v) => v,
        SpawnArgument::Bits(v) => v as i32,
        _ => return None,
    };

    Some(Order { at: spawn.at, kind })
}

#[derive(Debug, Clone, PartialEq)]

pub struct Placed {
    pub built: timers::Built,
    pub shade_placed: bool,
}

impl Placed {
    pub fn into_effects(self) -> Vec<Box<dyn Effect>> {
        let timers::Built {
            emitter,
            billboards,
            quads,
            shade,
        } = self.built;

        let mut out: Vec<Box<dyn Effect>> = Vec::new();

        for child in billboards {
            out.push(Box::new(child));
        }

        for quad in quads {
            out.push(Box::new(quad));
        }

        out.push(Box::new(shade));

        out.push(Box::new(emitter));

        out
    }
}

pub fn build(
    order: &Order,
    at: Vec3,
    now_ms: u32,
    rng: &mut Lcg,
    detail: i32,
    ground: Option<&Ground<'_>>,
) -> Placed {
    let detail = Detail {
        effects: BILLBOARD_GATE.admits(detail),
        ..Detail::default()
    };

    let mut built = timers::build(at, order.kind, now_ms, rng, detail);

    let shade_placed = match ground {
        Some(ground) => {
            built.shade.place(ground);

            true
        }
        None => false,
    };

    Placed {
        built,
        shade_placed,
    }
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

    let placed = build(&order, at, cx.now_ms, env.rng, cx.detail, env.ground);

    out.extend(placed.into_effects());

    Ok(())
}
