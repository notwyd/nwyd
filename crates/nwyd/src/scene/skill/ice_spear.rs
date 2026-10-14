use crate::scene::effect_tree::Effect;
use crate::scene::ice_spear::IceSpear;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Axis, PositionRef, Spawn, SpawnArgument};
use glam::Vec3;

pub const SPAWN_ARGUMENT_COUNT: usize = 9;

pub const OBJECT_SIZE: u16 = 188;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Order {
    pub source: PositionRef,
    pub dest: PositionRef,
    pub spare: i32,
    pub follows: bool,
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::SkillIceSpear || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    for (i, axis) in [Axis::X, Axis::Y, Axis::Z].iter().enumerate() {
        match (spawn.arguments[1 + i], spawn.arguments[4 + i]) {
            (SpawnArgument::Pos(a), SpawnArgument::Toward(b)) if a == *axis && b == *axis => {}
            _ => return None,
        }
    }

    let spare = match spawn.arguments[7] {
        SpawnArgument::Bits(v) => v as i32,
        SpawnArgument::Int(v) => v,
        _ => return None,
    };

    let follows = match spawn.arguments[8] {
        SpawnArgument::Body(_) => true,
        SpawnArgument::Bits(0) => false,
        _ => return None,
    };

    Some(Order {
        source: spawn.at,
        dest: spawn.toward,
        spare,
        follows,
    })
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

    let (Some(source), Some(dest)) = (cx.at.at(order.source), cx.at.at(order.dest)) else {
        return Err(Declined::Unplaced);
    };

    out.push(Box::new(build(&order, source, dest, cx.now_ms, cx.detail)));

    Ok(())
}

pub fn build(order: &Order, source: Vec3, dest: Vec3, now_ms: u32, detail: i32) -> IceSpear {
    IceSpear::new(source, dest, order.spare, None, now_ms, detail)
}
