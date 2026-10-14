use crate::scene::{
    effect_tree::Effect,
    meteor_storm::MeteorStorm,
    vfx::{Ambient, Cx, Declined},
};
use formats::vfx::dispatch::Class;
use formats::vfx::{Axis, Spawn, SpawnArgument};
use glam::Vec3;

pub fn read(spawn: &Spawn) -> Option<i32> {
    if spawn.class != Class::SkillMeteorStorm || spawn.size != 212 || spawn.arguments.len() != 9 {
        return None;
    }

    let mode = match spawn.arguments[7] {
        SpawnArgument::Int(mode) => mode,
        SpawnArgument::Bits(mode) => mode as i32,
        _ => return None,
    };

    if !matches!(mode, 0 | 1 | 2 | 3 | 4 | 9 | 10) || spawn.arguments[8] != SpawnArgument::Bits(0) {
        return None;
    }

    for (i, axis) in [Axis::X, Axis::Y, Axis::Z].into_iter().enumerate() {
        if spawn.arguments[4 + i] != SpawnArgument::Toward(axis) {
            return None;
        }

        if spawn.arguments[1 + i] != SpawnArgument::Pos(axis)
            && spawn.arguments[1 + i] != SpawnArgument::Float(0.0)
        {
            return None;
        }
    }

    Some(mode)
}

pub fn build_spawn(
    spawn: &Spawn,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let mode = read(spawn).ok_or(Declined::Arguments)?;

    let dest = cx.at.at(spawn.toward).ok_or(Declined::Unplaced)?;

    let source = if spawn.arguments[1..4]
        .iter()
        .all(|arg| *arg == SpawnArgument::Float(0.0))
    {
        Vec3::ZERO
    } else if mode == 3 {
        let context = env.construction.as_ref().ok_or(Declined::MissingContext)?;

        let y = match context.producer {
            Some((_, 0x6E)) => 1.0,
            Some((_, 0x73 | 0x74)) => 10.0,
            _ => return Err(Declined::MissingContext),
        };

        Vec3::new(dest.x, y, dest.z)
    } else {
        cx.at.at(spawn.at).ok_or(Declined::Unplaced)?
    };

    if !source.is_finite() || !dest.is_finite() {
        return Err(Declined::Arguments);
    }

    let mut meteor = MeteorStorm::new(source, dest, mode, cx.now_ms);

    if mode == 2 {
        let context = env.construction.as_ref().ok_or(Declined::MissingContext)?;

        meteor = MeteorStorm::new(
            crate::scene::meteor_storm::cannon_source(source, context.cannon),
            dest,
            mode,
            cx.now_ms,
        );

        meteor.viewer_distance = context.viewer.map_or(f32::INFINITY, |viewer| {
            Vec3::new(dest.x, 0.0, dest.z).distance(Vec3::new(
                viewer.position.x,
                0.0,
                viewer.position.z,
            ))
        });
    }

    if mode == 3
        && spawn
            .property_overrides
            .iter()
            .any(|patch| patch.offset == 0x8C)
    {
        meteor.birth_ms = meteor.birth_ms.wrapping_add(env.rng.next() % 1000);
    } else if mode == 4 {
        let ordinal = match spawn.arguments[0] {
            SpawnArgument::Allocation(n) if n < 5 => n,
            _ => return Err(Declined::Arguments),
        };

        meteor.birth_ms = meteor.birth_ms.wrapping_add(if ordinal == 4 {
            270
        } else {
            u32::from(ordinal) * 200
        });
    }

    out.push(Box::new(meteor));

    Ok(())
}
