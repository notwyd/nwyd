use super::{
    actors::{Actor, ActorScene},
    effect_tree::{Effect, Sink, Submit},
    shade::{Ground, Shade},
};
use glam::Vec3;

#[derive(Debug, Clone, Default)]
pub struct State {
    epoch: Option<u32>,
    pub mark: Option<Shade>,
}

fn recipe(model: u32, mounted: bool, part1: i16) -> Option<(i32, usize)> {
    Some(match model {
        0..=5 if mounted => (5, 4),
        7 | 29 => (4, 4),
        11 | 35 | 42 | 39 => (6, 4),
        12 | 24 | 32 => (2, 4),
        34 | 36 | 44 => (8, 4),
        37 => (2, 203),
        38 => (5, 4),
        20 if part1 == 0 => (6, 4),
        20 => return None,
        _ => (3, 4),
    })
}

fn refresh(actor: &mut Actor, quality: bool, now: u32) {
    if actor.body_shadow.epoch == Some(actor.appearance_epoch) {
        return;
    }

    actor.body_shadow.epoch = Some(actor.appearance_epoch);

    actor.body_shadow.mark = if quality {
        recipe(actor.model_id, actor.mounted, actor.parts[1].mesh).map(|(grid, texture)| {
            let mut mark = Shade::new(grid, texture, 0xFFFFFFFF, actor.position, -1);

            mark.pass = render::billboard::BillboardPass::Blended;

            mark.radius = actor.scale;

            mark.birth_ms = Some(now);

            mark
        })
    } else {
        None
    };
}

pub(crate) fn update(
    actors: &mut [Actor],
    foliage: &std::collections::BTreeMap<(u8, u8), Vec<bool>>,
    ground: &Ground<'_>,
    quality: bool,
    scene: u32,
    now: u32,
) {
    for actor in actors {
        refresh(actor, quality, now);

        if actor.is_dead {
            actor.body_shadow.mark = None;

            continue;
        }

        let factor = actor.evok_factor();

        let Some(mark) = &mut actor.body_shadow.mark else {
            continue;
        };

        let foliage = std::iter::once(ground.zone)
            .chain(ground.neighbours.iter().copied())
            .find(|z| actor.position.is_finite() && z.contains(actor.position.x, actor.position.z))
            .and_then(|zone| {
                let (ox, oz) = zone.origin();

                let (x, z) = (
                    (actor.position.x - ox) as usize,
                    (actor.position.z - oz) as usize,
                );

                foliage
                    .get(&(zone.zone_x, zone.zone_y))
                    .and_then(|f| f.get(z * 128 + x))
            })
            .copied()
            .unwrap_or(false);

        let foliage = foliage
            && (scene != 30000
                || super::footprints::geometry_height(ground.zone, actor.position)
                    .unwrap_or(-10000.0) as i32
                    == actor.position.y as i32);

        mark.visible = !foliage
            && actor.flags.get(0xB55) == 0
            && actor.flags.get(0xB3E) != 1
            && actor.class != 45;

        mark.centre = Vec3::new(actor.position.x, 0.0, actor.position.z);

        mark.place(ground);

        if factor != 1.0 {
            for vertex in &mut mark.vertices {
                vertex.position[0] = mark.centre.x + (vertex.position[0] - mark.centre.x) * factor;

                vertex.position[2] = mark.centre.z + (vertex.position[2] - mark.centre.z) * factor;

                if let Some(height) = ground.surface_height(vertex.position[0], vertex.position[2])
                {
                    vertex.position[1] = height + super::shade::LIFT;
                }
            }
        }
    }
}

pub(crate) fn submit(
    actors: &ActorScene,
    scene: &super::effects::BillboardScene,
    eye: Vec3,
    now: u32,
    frame: &mut super::effects::Frame,
) {
    let cx = Submit {
        scene,
        eye,
        now_ms: now,
    };

    let mut meshes = Vec::new();

    let mut sink = Sink {
        effect: frame,
        mesh: &mut meshes,
    };

    for actor in &actors.actors {
        if let Some(mark) = &actor.body_shadow.mark {
            mark.submit(&cx, &mut sink);
        }
    }
}
