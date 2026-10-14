use crate::scene::effect_tree::Effect;
use crate::scene::town_portal::TownPortal;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Axis, PositionRef, Spawn, SpawnArgument};
use glam::Vec3;

pub const SPAWN_ARGUMENT_COUNT: usize = 5;

pub const OBJECT_SIZE: u16 = 184;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Order {
    pub at: PositionRef,
    pub kind: i32,
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::SkillTownPortal || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
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

pub fn build_spawn(
    spawn: &Spawn,
    cx: &Cx<'_>,
    _env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let Some(order) = read(spawn) else {
        return Err(Declined::Arguments);
    };

    let Some(at) = cx.at.at(order.at) else {
        return Err(Declined::Unplaced);
    };

    out.push(Box::new(build(&order, at, cx.now_ms)));

    Ok(())
}

pub fn build(order: &Order, at: Vec3, now_ms: u32) -> TownPortal {
    TownPortal::new(at, order.kind, now_ms)
}
