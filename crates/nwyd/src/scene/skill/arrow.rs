use crate::scene::arrow::Arrow;
use crate::scene::effect_tree::Effect;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Axis, PositionRef, PropertyOverride, Spawn, SpawnArgument};
use glam::Vec3;

pub const SPAWN_ARGUMENT_COUNT: usize = 12;

pub const OBJECT_SIZE: u16 = 148;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Unmodelled {
    Patch { offset: u16, bytes: u32 },
}

#[derive(Debug, Clone, PartialEq)]

pub struct Order {
    pub source: PositionRef,
    pub dest: PositionRef,
    pub submode: i32,
    pub kind: i32,
    pub flag: u8,
    pub variant: i32,
    pub owner: u32,
    pub delay_ms: u32,
    pub unmodelled: Vec<Unmodelled>,
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::Arrow || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    for (i, axis) in [Axis::X, Axis::Y, Axis::Z].iter().enumerate() {
        match spawn.arguments[1 + i] {
            SpawnArgument::Pos(a) if a == *axis => {}
            _ => return None,
        }
    }

    for (i, axis) in [Axis::X, Axis::Y, Axis::Z].iter().enumerate() {
        match spawn.arguments[4 + i] {
            SpawnArgument::Toward(a) if a == *axis => {}
            _ => return None,
        }
    }

    let int = |arg: SpawnArgument| match arg {
        SpawnArgument::Int(v) => Some(v),
        _ => None,
    };

    let bits = |arg: SpawnArgument| match arg {
        SpawnArgument::Bits(v) => Some(v),
        _ => None,
    };

    let mut delay_ms = 0;

    let mut unmodelled = Vec::new();

    for PropertyOverride { offset, bytes, .. } in spawn.property_overrides {
        if *offset == 0x24 {
            delay_ms = bytes.checked_sub(100_000)?;
        } else {
            unmodelled.push(Unmodelled::Patch {
                offset: *offset,
                bytes: *bytes,
            });
        }
    }

    Some(Order {
        source: spawn.at,
        dest: spawn.toward,
        submode: int(spawn.arguments[7])?,
        kind: bits(spawn.arguments[8])? as i32,
        flag: bits(spawn.arguments[9])? as u8,
        variant: int(spawn.arguments[10])?,
        owner: bits(spawn.arguments[11])?,
        delay_ms,
        unmodelled,
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

    if let Some(context) = env.construction.as_ref()
        && context.producer.is_some_and(|(chain, skill)| {
            chain == formats::vfx::Chain::Caster && matches!(skill, 0x68 | 0x69 | 0x97..=0x99)
        })
    {
        order.owner = context.caster.skill_child.map_or(0, u32::from);

        order.variant = i32::from(context.caster.primary_tier);

        order.flag = u8::from(context.caster.affect_29);
    }

    let (Some(source), Some(dest)) = (cx.at.at(order.source), cx.at.at(order.dest)) else {
        return Err(Declined::Unplaced);
    };

    out.push(Box::new(build(&order, source, dest, cx.now_ms)));

    Ok(())
}

pub fn build(order: &Order, source: Vec3, dest: Vec3, now_ms: u32) -> Arrow {
    let mut arrow = Arrow::new(
        source,
        dest,
        order.submode,
        order.kind,
        order.flag,
        order.variant,
        order.owner,
        now_ms,
    );

    arrow.birth_ms = now_ms.wrapping_add(order.delay_ms);

    arrow
}

pub fn describe(arrow: &Arrow, unmodelled: &[Unmodelled]) -> String {
    let model = match arrow.model {
        Some(id) => format!("{id:#X}"),
        None => "none".to_string(),
    };

    let mut line = format!(
        "NWYD: Arrow kind {:#X} submode {} — mesh {model}, {:.1} units in {} ms, variant {}",
        arrow.kind, arrow.submode, arrow.distance, arrow.lifetime_ms, arrow.variant,
    );

    if arrow.trail.is_some() {
        line.push_str("; trail");
    }

    if arrow.plane.is_some() {
        line.push_str("; plane");
    }

    for skipped in unmodelled {
        match skipped {
            Unmodelled::Patch { offset, bytes } => {
                line.push_str(&format!("; unmodelled store +{offset:#X} = {bytes:#010X}"));
            }
        }
    }

    line
}
