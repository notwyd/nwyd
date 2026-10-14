use crate::scene::effect_tree::Effect;
use crate::scene::slow_slash::SlowSlash;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Axis, Body, PositionRef, Spawn, SpawnArgument};
use glam::Vec3;

pub const SPAWN_ARGUMENT_COUNT: usize = 9;

pub const OBJECT_SIZE: u16 = 184;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Order {
    pub source: PositionRef,
    pub dest: PositionRef,
    pub kind: i32,
    pub follows: bool,
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::SkillSlowSlash || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    for (i, axis) in [Axis::X, Axis::Y, Axis::Z].iter().enumerate() {
        match (spawn.arguments[1 + i], spawn.arguments[4 + i]) {
            (SpawnArgument::Pos(a), SpawnArgument::Toward(b)) if a == *axis && b == *axis => {}
            _ => return None,
        }
    }

    let kind = match spawn.arguments[7] {
        SpawnArgument::Bits(v) => v as i32,
        SpawnArgument::Int(v) => v,
        _ => return None,
    };

    let follows = match spawn.arguments[8] {
        SpawnArgument::Body(Body::Target) => true,
        SpawnArgument::Bits(0) => false,
        _ => return None,
    };

    Some(Order {
        source: spawn.at,
        dest: spawn.toward,
        kind,
        follows,
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

    let (Some(source), Some(dest)) = (cx.at.at(order.source), cx.at.at(order.dest)) else {
        return Err(Declined::Unplaced);
    };

    let mut effect = build(&order, source, dest, cx.now_ms);

    if order.follows {
        effect.target_key = Some(
            env.construction
                .as_ref()
                .and_then(|cx| cx.target)
                .ok_or(Declined::MissingContext)?
                .key,
        );
    }

    out.push(Box::new(effect));

    Ok(())
}

pub fn build(order: &Order, source: Vec3, dest: Vec3, now_ms: u32) -> SlowSlash {
    SlowSlash::new(source, dest, order.kind, None, now_ms)
}
