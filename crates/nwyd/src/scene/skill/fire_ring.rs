use crate::scene::effect_tree::Effect;
use crate::scene::fire_ring::FireRing;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{PositionRef, Spawn, SpawnArgument};
use glam::Vec3;

pub const SPAWN_ARGUMENT_COUNT: usize = 8;

pub const OBJECT_SIZE: u16 = 168;

pub const Y: f32 = 0.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Order {
    pub mode: i32,
    pub scale: f32,
    pub interval_ms: i32,
    pub colour: u32,
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::SkillFireRing || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    let bits = |arg: SpawnArgument| match arg {
        SpawnArgument::Bits(v) => Some(v),
        SpawnArgument::Int(v) => Some(v as u32),
        _ => None,
    };

    Some(Order {
        mode: bits(spawn.arguments[4])? as i32,
        scale: f32::from_bits(bits(spawn.arguments[5])?),
        interval_ms: bits(spawn.arguments[6])? as i32,
        colour: bits(spawn.arguments[7])?,
    })
}

pub fn point(at: &crate::scene::vfx::Anchors) -> Option<Vec3> {
    at.at(PositionRef::Target).map(|t| Vec3::new(t.x, Y, t.z))
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

    let Some(at) = point(cx.at) else {
        return Err(Declined::Unplaced);
    };

    let mut boom = build(&order, at, cx.now_ms, env.ground);

    out.extend(Effect::take_spawned(&mut boom));

    out.push(Box::new(boom));

    Ok(())
}

pub fn build(
    order: &Order,
    at: Vec3,
    now_ms: u32,
    ground: Option<&crate::scene::shade::Ground<'_>>,
) -> FireRing {
    FireRing::new(
        at,
        order.mode,
        order.scale,
        order.interval_ms,
        order.colour,
        now_ms,
        ground,
    )
}
