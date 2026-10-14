use crate::scene::effect_tree::Effect;
use crate::scene::fire::Fire;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Axis, Body, PositionRef, Spawn, SpawnArgument};
use glam::Vec3;

pub const SPAWN_ARGUMENT_COUNT: usize = 8;

pub const OBJECT_SIZE: u16 = 172;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Order {
    pub at: PositionRef,
    pub mode: i32,
    pub follows: bool,
    pub spark_colour: i32,
    pub colour: u32,
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::SkillFire || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
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
        mode: bits(spawn.arguments[4])? as i32,
        follows: match spawn.arguments[5] {
            SpawnArgument::Body(Body::Target) => true,
            SpawnArgument::Bits(0) => false,
            _ => return None,
        },
        spark_colour: bits(spawn.arguments[6])? as i32,
        colour: bits(spawn.arguments[7])?,
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

    let Some(at) = cx.at.at(order.at) else {
        return Err(Declined::Unplaced);
    };

    let mut effect = build(&order, at, cx.now_ms, env.ground);

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

pub fn build(
    order: &Order,
    at: Vec3,
    now_ms: u32,
    ground: Option<&crate::scene::shade::Ground<'_>>,
) -> Fire {
    Fire::new(
        at,
        order.mode,
        None,
        order.spark_colour,
        order.colour,
        now_ms,
        ground,
    )
}
