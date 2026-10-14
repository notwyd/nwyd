use crate::scene::effect_tree::Effect;
use crate::scene::sp_change::{self, Built, SpChange};
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Axis, PositionRef, Spawn, SpawnArgument};

pub const SPAWN_ARGUMENT_COUNT: usize = 6;

pub const OBJECT_SIZE: u16 = 168;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Order {
    pub at: PositionRef,
    pub mode: i32,
    pub follows: bool,
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::SkillSpChange || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    for (i, axis) in [Axis::X, Axis::Y, Axis::Z].iter().enumerate() {
        match spawn.arguments[1 + i] {
            SpawnArgument::Pos(a) if a == *axis => {}
            _ => return None,
        }
    }

    let mode = match spawn.arguments[4] {
        SpawnArgument::Int(v) => v,
        SpawnArgument::Bits(v) => v as i32,
        _ => return None,
    };

    let follows = match spawn.arguments[5] {
        SpawnArgument::Body(_) => true,
        SpawnArgument::Bits(0) => false,
        _ => return None,
    };

    Some(Order {
        at: spawn.at,
        mode,
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

    let Some(at) = cx.at.at(order.at) else {
        return Err(Declined::Unplaced);
    };

    let owner = match spawn.arguments[5] {
        SpawnArgument::Body(body) => Some(
            *env.construction
                .as_ref()
                .and_then(|context| context.body(body))
                .ok_or(Declined::MissingContext)?,
        ),
        _ => None,
    };

    let mut built = sp_change::build(
        at,
        order.mode,
        owner.map_or(0.0, |body| body.yaw),
        cx.now_ms,
    );

    built.emitter.owner = owner.map(|body| body.key);

    if let Some(context) = env.construction.as_ref() {
        for ghost in &mut built.afterimages {
            crate::scene::skin_inputs::sp_change(context, owner, ghost);
        }
    }

    if let Some(ground) = env.ground {
        built.emitter.place(ground);
    }

    env.skins.extend(built.afterimages);

    out.push(Box::new(built.emitter));

    Ok(())
}

pub fn describe(built: &Built) -> String {
    let emitter: &SpChange = &built.emitter;

    format!(
        "NWYD: SkillSpChange mode {} at ({:.1}, {:.1}, {:.1}) — {} afterimage(s), life {} ms\
         , shade {}",
        emitter.mode,
        emitter.position.x,
        emitter.position.y,
        emitter.position.z,
        built.afterimages.len(),
        emitter.lifetime_ms,
        match &emitter.shade {
            Some(s) => format!("grid {}", s.grid),
            None => "none".to_string(),
        },
    )
}
