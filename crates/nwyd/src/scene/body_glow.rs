use super::{actors::Actor, effects::Billboard};
use glam::Vec3;

#[derive(Debug, Clone, Copy, Default, PartialEq)]

pub struct State {
    pub last_ms: u32,
    pub points: [Option<Vec3>; 7],
}

pub(crate) fn publish(actor: &mut Actor) {
    if actor.model_id != 20 {
        return;
    }

    for (point, bone) in actor
        .body_glow
        .points
        .iter_mut()
        .zip([9, 13, 16, 11, 18, 24, 31])
    {
        if actor.tree.slot(bone).is_some() {
            let p = render::camera::Matrix4(actor.tree.combined(bone)).transform_coord(Vec3::ZERO);

            if p.is_finite() {
                *point = Some(p);
            }
        }
    }
}

pub(crate) fn emit(actor: &mut Actor, now: u32) -> Vec<Billboard> {
    if actor.model_id != 20
        || ((actor.action <= 1 || actor.action == u32::MAX as usize || actor.action == usize::MAX)
            && actor.removal_mode == 0)
        || !actor.scale.is_finite()
    {
        return Vec::new();
    }

    let small = actor.scale < 0.6;

    let (count, period, colour) = match (actor.parts[1].mesh, actor.parts[1].tex, small) {
        (2, _, false) => (7, 1000, 0xFFFF0000),
        (2, _, true) => (3, 1200, 0xFFFF0000),
        (4, 1, false) => (5, 1000, 0xFFFF9900),
        (4, 1, true) => (3, 800, 0xFFFF9900),
        _ => return Vec::new(),
    };

    if now.wrapping_sub(actor.body_glow.last_ms) <= period {
        return Vec::new();
    }

    actor.body_glow.last_ms = now;

    actor.body_glow.points[..count]
        .iter()
        .flatten()
        .map(|&point| {
            let mut b = Billboard::new(44, 2000, Vec3::splat(actor.scale * 4.0), 0.0005, 1, 80);

            b.position = point;

            b.set_color(colour);

            b.pass = render::billboard::BillboardPass::Additive;

            b
        })
        .collect()
}
