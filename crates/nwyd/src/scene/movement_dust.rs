use super::{actors::Actor, effects::Billboard, shade::Ground};
use glam::Vec3;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct State {
    pub run_ms: u32,
    pub trail_deadline: Option<u32>,
}

fn recipe(actor: &mut Actor, now: u32, surface: u8, scene: u32, weather: u32) -> Option<Billboard> {
    if !actor.position.is_finite()
        || ((actor.action <= 1 || actor.action == u32::MAX as usize || actor.action == usize::MAX)
            && actor.removal_mode == 0)
        || (actor.class == 45 && actor.flags.get(0xB55) != 0)
        || actor.flags.get(0xB3E) != 0
    {
        return None;
    }

    let radius = super::pick::PICK_VOLUME.get(actor.model_id as usize)?.0 * actor.scale;

    let no_trail = if actor.mounted {
        actor.mount_model == Some(40)
    } else {
        actor.model_id == 40
    };

    let special = actor.flags.get(0xB5A) == 1
        && !actor.is_dead
        && actor
            .movement_dust
            .trail_deadline
            .map_or(now != 0, |d| now.wrapping_sub(d) as i32 > 0);

    let mut b = if special {
        if no_trail {
            return None;
        }

        actor.movement_dust.trail_deadline = Some(now.wrapping_add(20));

        let mut b = Billboard::new(10, 450, Vec3::splat(0.1), radius * 0.0392, 1, 80);

        b.position = actor.position - Vec3::Y * 0.9;

        b.set_color(0x30101010);

        b
    } else {
        let region = (actor.position.x as i32 >> 7, actor.position.z as i32 >> 7);

        if scene == 30000
            && (weather == 1 || ((27..=30).contains(&region.0) && (21..=24).contains(&region.1)))
        {
            return None;
        }

        if !((actor.action == 3 && actor.flags.get(0xB55) == 0)
            || (matches!(actor.class, 20 | 22) && actor.action == 2))
            || no_trail
            || !actor.speed.is_finite()
            || actor.speed <= 0.0
        {
            return None;
        }

        let period = (1000.0 / actor.speed) as u32;

        if now.wrapping_sub(actor.movement_dust.run_ms) <= period {
            return None;
        }

        actor.movement_dust.run_ms = now;

        let mut b = Billboard::new(
            if surface == 8 { 193 } else { 0 },
            2000,
            Vec3::splat(0.5),
            radius * 0.0012,
            1,
            80,
        );

        b.position = actor.position;

        b.set_color(0xFFFFFFFF);

        b
    };

    b.lift = true;

    b.pass = render::billboard::BillboardPass::Additive;

    Some(b)
}

pub(crate) fn emit(
    actors: &mut [Actor],
    foliage: &std::collections::BTreeMap<(u8, u8), Vec<bool>>,
    ground: &Ground<'_>,
    scene: u32,
    weather: u32,
    now: u32,
) -> Vec<Billboard> {
    actors
        .iter_mut()
        .filter_map(|actor| {
            let surface = std::iter::once(ground.zone)
                .chain(ground.neighbours.iter().copied())
                .find(|zone| zone.contains(actor.position.x, actor.position.z))
                .map_or(0, |zone| {
                    super::footprints::terrain_kind(
                        zone,
                        foliage.get(&(zone.zone_x, zone.zone_y)),
                        actor.position,
                    )
                });

            recipe(actor, now, surface, scene, weather)
        })
        .collect()
}
