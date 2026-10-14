use crate::scene::effect_tree::Effect;
use crate::scene::freeze_blade::FreezeBlade;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Axis, PositionRef, Spawn, SpawnArgument};
use glam::Vec3;

pub const SPAWN_ARGUMENT_COUNT: usize = 7;

pub const OBJECT_SIZE: u16 = 188;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Order {
    pub at: PositionRef,
    pub kind: i32,
    pub trail: bool,
    pub spin: bool,
    pub chain: bool,
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::SkillFreezeBlade || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT
    {
        return None;
    }

    for (i, axis) in [Axis::X, Axis::Y, Axis::Z].iter().enumerate() {
        match spawn.arguments[1 + i] {
            SpawnArgument::Pos(a) if a == *axis => {}
            _ => return None,
        }
    }

    let bits = |arg: SpawnArgument| match arg {
        SpawnArgument::Bits(v) => Some(v),
        SpawnArgument::Int(v) => Some(v as u32),
        _ => None,
    };

    Some(Order {
        at: spawn.at,
        kind: bits(spawn.arguments[4])? as i32,
        trail: bits(spawn.arguments[5])? == 1,
        spin: bits(spawn.arguments[6])? == 1,
        chain: matches!(spawn.at, PositionRef::Along { .. }),
    })
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

    let Some(blade) = build_placed(&order, cx.at, env.rng, cx.now_ms) else {
        return Err(Declined::Unplaced);
    };

    out.push(Box::new(blade));

    Ok(())
}

fn build_placed(
    order: &Order,
    anchors: &crate::scene::vfx::Anchors,
    rng: &mut crate::scene::rng::Lcg,
    now_ms: u32,
) -> Option<FreezeBlade> {
    let at = anchors.at(order.at)?;

    let mut blade = build(order, at, rng, now_ms);

    if order.chain {
        blade.step = (at.x - anchors.caster.x, at.z - anchors.caster.z);
    }

    Some(blade)
}

pub fn build(
    order: &Order,
    at: Vec3,
    rng: &mut crate::scene::rng::Lcg,
    now_ms: u32,
) -> FreezeBlade {
    FreezeBlade::new(at, order.kind, order.trail, order.spin, rng, now_ms)
}
