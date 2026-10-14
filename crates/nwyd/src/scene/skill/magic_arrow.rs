use crate::scene::effect_tree::Effect;
use crate::scene::magic_arrow::MagicArrow;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Axis, PositionRef, Spawn, SpawnArgument};
use glam::Vec3;

pub const SPAWN_ARGUMENT_COUNT: usize = 9;

pub const OBJECT_SIZE: u16 = 204;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Order {
    pub at: PositionRef,
    pub toward: PositionRef,
    pub mode: i32,
    pub follows: bool,
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::SkillMagicArrow || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    for (i, axis) in [Axis::X, Axis::Y, Axis::Z].iter().enumerate() {
        match (spawn.arguments[1 + i], spawn.arguments[4 + i]) {
            (SpawnArgument::Pos(a), SpawnArgument::Toward(b)) if a == *axis && b == *axis => {}
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
        toward: spawn.toward,
        mode: bits(spawn.arguments[7])? as i32,
        follows: bits(spawn.arguments[8])? != 0,
    })
}

pub fn build_spawn(
    spawn: &Spawn,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let Some(mut order) = read(spawn) else {
        return Err(Declined::Arguments);
    };

    let (Some(from), Some(to)) = (cx.at.at(order.at), cx.at.at(order.toward)) else {
        return Err(Declined::Unplaced);
    };

    order.mode = live_mode(
        order.mode,
        env.construction.as_ref().map(|c| c.caster.class),
    );

    out.push(Box::new(build(&order, from, to, cx.now_ms)));

    Ok(())
}

fn live_mode(captured: i32, class: Option<i32>) -> i32 {
    if captured == 1 && class == Some(0x26) {
        2
    } else {
        captured
    }
}

pub fn build(order: &Order, from: Vec3, to: Vec3, now_ms: u32) -> MagicArrow {
    MagicArrow::new(from, to, order.mode, None, now_ms)
}
