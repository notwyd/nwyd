use super::{
    attack, enter,
    field::{self, FieldScene},
};

pub fn character(scene: &mut FieldScene, ch: char) -> bool {
    if !matches!(ch, ']' | '}') || scene.entered.status.hp as i32 <= 0 {
        return false;
    }

    let Some(player) = scene.player else {
        return false;
    };

    if attack::chase_block(&scene.world.actors.actors[player]) != 0 {
        return false;
    }

    if !matches!(
        scene.world.current,
        (13, 31) | (14, 30) | (15, 31) | (9, 28) | (8, 27) | (10, 27)
    ) {
        scene.automatic_movement = !scene.automatic_movement;
    }

    true
}

pub fn tick(scene: &mut FieldScene, now_ms: u32) -> bool {
    if !scene.automatic_movement
        || scene.entered.status.hp as i32 <= 0
        || scene
            .hud
            .as_ref()
            .is_some_and(|hud| hud.manager.carried.is_some())
    {
        return false;
    }

    let Some(player) = scene.player else {
        return false;
    };

    let actor = &scene.world.actors.actors[player];

    let direction = render::camera::forward(scene.camera.yaw, scene.camera.pitch);

    let x = actor.position.x + actor.speed * direction.x * 1.5;

    let z = actor.position.z + actor.speed * direction.z * 1.5;

    if !x.is_finite()
        || !z.is_finite()
        || !(0.0..4096.0).contains(&x)
        || !(0.0..4096.0).contains(&z)
    {
        return false;
    }

    let tile = (x as i32, z as i32);

    if scene
        .world
        .actors
        .attributes()
        .and_then(|map| map.at_cell(tile.0 as usize, tile.1 as usize))
        .is_some_and(|value| value & 0x80 != 0)
    {
        return false;
    }

    if actor.mover.server_driven
        || now_ms.wrapping_sub(scene.cc.last_action_ms) < 1000
        || attack::chase_block(actor) != 0
        || (!actor.mounted && !matches!(actor.action, usize::MAX | 0..=3 | 14))
        || scene
            .panels
            .as_ref()
            .and_then(|panels| panels.store.as_ref())
            .is_some_and(|store| store.owner == scene.entered.client_id)
        || ((scene.attack.stuck.is_some() || scene.attack.approach_target.is_some())
            && now_ms.wrapping_sub(scene.attack.last_pursue_ms) < 1000)
        || attack::action_busy(actor, &scene.world.actors.models, now_ms)
    {
        return false;
    }

    let here = attack::cell_of(actor);

    let alpha = actor.mover.move_alpha;

    let speed = enter::move_speed(&actor.status);

    scene.world.actors.actors[player].speed = speed;

    scene.wire_speed = speed as i32;

    if tile == here {
        return false;
    }

    scene.cc.auto_move = tile;

    let walked = (f64::from(alpha) >= 0.9 || alpha == 0.0) && field::order(scene, tile);

    scene.attack.auto_target = None;

    scene.attack.approach_target = None;

    scene.attack.stuck = None;

    scene.hovered = None;

    scene.pending_pickup = None;

    walked
}
