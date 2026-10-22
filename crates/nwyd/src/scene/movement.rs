use super::{action, attack, field::FieldScene};
use formats::{
    net::action::Action,
    world::path::{self, HeightField},
};

pub(crate) fn plan_interval(speed: f32) -> u32 {
    match speed as i32 {
        4 | 5 => 500,
        6 | 7 => 100,
        _ => 1000,
    }
}

pub(crate) fn boundary(alpha: f32, speed: f32) -> bool {
    alpha == 0.0 || f64::from(alpha) >= if speed >= 5.0 { 0.8 } else { 0.9 }
}

pub(crate) fn preview(scene: &FieldScene, tile: (i32, i32)) -> Option<path::Path> {
    let actor = scene.world.actors.actors.get(scene.player?)?;

    let from = attack::cell_of(actor);

    let height = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

    let mut plan = preview_route(&height, from, tile, scene.sent_dest)?;

    while !plan.dirs.is_empty()
        && scene
            .world
            .actors
            .actors
            .iter()
            .any(|a| !a.is_dead && attack::cell_of(a) == plan.reached)
    {
        let (dx, dz) = path::delta(plan.dirs.pop().unwrap());

        plan.reached.0 -= dx;

        plan.reached.1 -= dz;
    }

    Some(plan)
}

fn preview_route(
    height: &HeightField<'_>,
    from: (i32, i32),
    tile: (i32, i32),
    last: Option<(i32, i32)>,
) -> Option<path::Path> {
    let mut plan = path::walk(height, from, tile, 12, 8);

    if plan.reached != tile {
        plan = path::walk(height, from, plan.reached, 12, 8);
    }

    if let Some(last) = last.filter(|(x, z)| *x != 0 && *z != 0) {
        if (height.byte(plan.reached.0, plan.reached.1) - height.byte(last.0, last.1)).abs() > 30 {
            return None;
        }
    }

    for budget in [6, 3] {
        if (height.byte(plan.reached.0, plan.reached.1) - height.byte(from.0, from.1)).abs() > 30 {
            plan = path::walk(height, from, plan.reached, budget, 8);
        }
    }

    if plan.dirs.is_empty() {
        return None;
    }

    Some(plan)
}

pub(crate) fn order(scene: &mut FieldScene, tile: (i32, i32)) -> bool {
    let Some(index) = scene.player else {
        return false;
    };

    let actor = &scene.world.actors.actors[index];

    if actor.is_dead
        || scene.sent_dest == Some(tile)
        || scene.time.now_ms.wrapping_sub(scene.cc.last_order_ms) <= plan_interval(actor.speed)
        || !scene.lock.allows(scene.time.now_ms)
    {
        return false;
    }

    let from = attack::cell_of(actor);

    let Some(plan) = preview(scene, tile) else {
        return false;
    };

    let to = plan.reached;

    scene.cc.auto_move = to;

    if to == from || to == scene.world.actors.actors[index].mover.last_dest {
        return false;
    }

    let packet = Action::walk(from, to, scene.wire_speed, &plan.dirs);

    if scene.sent_dest != Some(to) && scene.time.now_ms.wrapping_sub(scene.cc.last_order_ms) > 1000
    {
        report(scene, &packet);
    }

    action::apply_op(
        scene,
        protocol::server::MOVE,
        scene.entered.client_id,
        &packet,
    )
    .walked
}

pub(crate) fn has_pending_report(outbox: &[Vec<u8>]) -> bool {
    outbox
        .iter()
        .any(|body| protocol::header::opcode_of(body) == Some(protocol::client::MOVE))
}

fn queue_report(outbox: &mut Vec<Vec<u8>>, packet: &Action) -> bool {
    if has_pending_report(outbox) {
        return false;
    }

    outbox.push(protocol::client::move_to(packet));

    true
}

pub(crate) fn discard_spatial_outbox(outbox: &mut Vec<Vec<u8>>) {
    outbox.retain(|body| {
        !matches!(
            protocol::header::opcode_of(body),
            Some(
                protocol::client::MOVE
                    | protocol::client::MOVE_STOP
                    | protocol::client::ACT
                    | formats::net::attack::ATTACK
                    | formats::net::attack::ATTACK_AREA
                    | formats::net::attack::ATTACK_MANY
            )
        )
    });
}

fn report(scene: &mut FieldScene, packet: &Action) -> bool {
    if !queue_report(&mut scene.outbox, packet) {
        return false;
    }

    scene.attack.remember_move(packet);

    scene.sent_dest = Some(packet.to());

    scene.cc.last_order_ms = scene.time.now_ms;

    scene.last_move_sent_ms = scene.time.now_ms;

    true
}

pub(crate) fn tick(scene: &mut FieldScene) {
    let Some(index) = scene.player else {
        return;
    };

    let actor = &scene.world.actors.actors[index];

    if actor.is_dead
        || scene.entered.status.hp == 0
        || scene.air_move.is_some()
        || !boundary(actor.mover.move_alpha, actor.speed)
    {
        return;
    }

    let to = scene.cc.auto_move;

    if attack::cell_of(actor) != to {
        order(scene, to);

        return;
    }

    if scene.sent_dest == Some(to) || scene.time.now_ms.wrapping_sub(scene.cc.last_order_ms) <= 1000
    {
        return;
    }

    let packet = if actor.speed >= 5.0 {
        let from = scene
            .attack
            .from
            .map(|(x, z)| (i32::from(x), i32::from(z)))
            .unwrap_or((0, 0));

        let height = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

        let mut plan = path::walk(&height, from, to, 23, 8);

        if plan.reached != to {
            plan = path::walk(&height, from, plan.reached, 23, 8);
        }

        Action::walk(from, plan.reached, scene.wire_speed, &plan.dirs)
    } else {
        Action::walk(to, to, scene.wire_speed, &[])
    };

    if report(scene, &packet) {
        scene.sent_dest = Some(to);
    }
}

#[cfg(test)]
mod tests;
