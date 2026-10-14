use super::actors::Actor;
use super::field::FieldScene;
pub use super::skinfx::AFTERIMAGE_MS;
use formats::mesh::appearance;
use formats::net::action::{Action, Order};
use formats::world::path::{self, HeightField};

pub const QUIET_SUB_KINDS: [i32; 2] = [5, 10];

pub const LEVELUP_KIND: i32 = 0;

pub const CAMERA_CEILING_EDGE: i32 = 0x0C80;

fn camera_ceiling(
    camera: &mut super::camera::Camera,
    low_ceiling: &mut bool,
    old_z: f32,
    destination_z: u16,
) {
    let old_low = ((old_z as i32) & !0x7F) > CAMERA_CEILING_EDGE;

    let new_low = i32::from(destination_z & 0xFF80) > CAMERA_CEILING_EDGE;

    *low_ceiling = new_low;

    match (old_low, new_low) {
        (false, true) => {
            camera.max_dist = super::camera::VIEW_DIST_NEAR;

            camera.dist = camera.dist.min(super::camera::VIEW_DIST_NEAR);
        }
        (true, false) => {
            camera.max_dist = super::camera::FIELD_MAX_DIST;

            camera.dist = (camera.dist + 4.0).min(super::camera::FIELD_MAX_DIST);
        }
        _ => {}
    }
}

pub const ARRIVAL_POOF_CLASSES: [i32; 5] = [1, 2, 4, 8, 0x1A];

pub const ARRIVAL_POOF_STYLE_LISTED: i32 = 0;

pub fn poof_style(class: i32) -> i32 {
    match ARRIVAL_POOF_CLASSES.contains(&class) {
        true => ARRIVAL_POOF_STYLE_LISTED,
        false => ARRIVAL_POOF_STYLE_OTHER,
    }
}

pub const ARRIVAL_POOF_STYLE_OTHER: i32 = 1;

pub const ARRIVAL_LIFT: f32 = 0.05;

pub fn portal_kind(sub: i32) -> i32 {
    match sub == JUDGEMENT_SUB {
        true => 2,
        false => 1,
    }
}

pub const JUDGEMENT_SUB: i32 = 6;

pub const JUDGEMENT_CLASS: i32 = 0x3E;

pub const JUDGEMENT_PART0_MESH: i16 = 2;

pub const JUDGEMENT_MODE: i32 = 4;

pub const JUDGEMENT_ARG5: f32 = 0.1;

pub const JUDGEMENT_SINK: f32 = 2.0;

pub const JUDGEMENT_LIFT: f32 = 0.2;

pub fn should_spawn_judgement(sub: i32, class: i32, part0_mesh: i16) -> bool {
    sub == JUDGEMENT_SUB && class == JUDGEMENT_CLASS && part0_mesh == JUDGEMENT_PART0_MESH
}

pub const ARRIVAL_SOUND: i64 = 0x97;

pub const STAND_SOUND: i64 = 0xAF;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Raised {
    pub class: &'static str,
    pub at: glam::Vec3,
    pub kind: i32,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Applied {
    pub entity: u16,
    pub index: Option<usize>,
    pub order: Order,
    pub walked: bool,
    pub speed: f32,
    pub acted: bool,
    pub warped: bool,
    pub held: bool,
    pub afterimage: bool,
    pub levelled: bool,
    pub at: (i32, i32),
    pub raised: [Option<Raised>; 5],
}

pub fn apply_op(scene: &mut FieldScene, opcode: u16, entity: u16, action: &Action) -> Applied {
    let index = scene.world.actors.entity(entity);

    let order = action.order();

    let speed = action.tiles_per_second();

    let mut applied = Applied {
        entity,
        index,
        order,
        walked: false,
        speed,
        acted: false,
        warped: false,
        held: false,
        afterimage: false,
        levelled: false,
        at: action.to(),
        raised: [None; 5],
    };

    let Some(index) = index else {
        return applied;
    };

    let now_ms = scene.time.now_ms;

    let local = Some(index) == scene.player;

    let remote = !local;

    let act_mana_cost = scene
        .statics
        .skills
        .as_ref()
        .and_then(|skills| skills.get(super::cast::BLINK_CODE as usize))
        .map_or(0, |skill| skill.cost) as u32;

    let body = &scene.world.actors.actors[index];

    let departure = (body.position.x as i32, body.position.z as i32);

    let hidden = body.flags.get(0xB55) != 0
        || (body.entity < super::hover::PLAYER_ID_LIMIT && body.status.flags & 1 != 0);

    let ghost = (matches!(order, Order::Act(kind) if kind != 10)
        && opcode == protocol::server::ACT
        && !hidden)
        .then(|| scene.world.actors.teleport_afterimage(index, now_ms))
        .flatten();

    let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

    let actor = &mut scene.world.actors.actors[index];

    let starts_quest_timer = scene.hud.is_some()
        && super::quest_timer::starts(
            local,
            order,
            (actor.position.x as i32, actor.position.z as i32),
            action.to(),
        );

    let mut remember_move = false;

    match order {
        Order::Walk => {
            actor.speed = speed;

            if action.kind == 2 && !actor.is_dead {
                actor.mover.server_driven = true;

                if local && action.speed == 7 {
                    scene.automatic_movement = false;

                    scene.cc.auto_move = action.to();
                }
            }

            let here = (actor.position.x as i32, actor.position.z as i32);

            applied.walked =
                actor
                    .mover
                    .accept(&field, here, action, path::STEP_BUDGET, now_ms, remote);

            actor.destination = actor.mover.last_dest;

            remember_move = local && actor.mover.pathless;
        }
        Order::Teleport => {
            actor.speed = speed;

            actor.mover.teleport(action.to(), now_ms);

            actor.destination = action.to();

            applied.walked = true;
        }
        Order::Act(kind) => {
            applied.acted = true;

            if !hidden && opcode == protocol::server::ACT && kind != 10 {
                scene.entered.status.mp = scene.entered.status.mp.wrapping_sub(act_mana_cost);
            }

            let quiet = QUIET_SUB_KINDS.contains(&kind);

            applied.afterimage = !hidden && opcode == protocol::server::ACT && kind != 10;

            applied.held = !hidden && !applied.afterimage && !quiet;

            let leaving = applied.held.then(|| {
                glam::Vec3::new(
                    actor.position.x,
                    actor.position.y + ARRIVAL_LIFT,
                    actor.position.z,
                )
            });

            let class = actor.class;

            let part0_mesh = actor.parts[0].mesh;

            let green_name = actor.green_name;

            applied.levelled = !hidden && opcode != protocol::server::ACT;

            if local {
                camera_ceiling(
                    &mut scene.camera,
                    &mut scene.low_ceiling,
                    actor.position.z,
                    action.dest_z,
                );

                if scene.interface.zoom {
                    scene.camera.max_dist = scene.interface.camera_max(scene.low_ceiling);
                }
            }

            warp(actor, &field, action.to());

            let levelled_at = applied.levelled.then_some(actor.position);

            applied.walked = true;

            applied.warped = true;

            if let Some(effect) = ghost {
                scene.world.actors.skinfx.spawn(effect);
            }

            if let Some(column) = leaving {
                let kind = portal_kind(kind);

                let now = now_ms;

                let portal = super::town_portal::TownPortal::new(column, kind, now);

                applied.raised[0] = Some(Raised {
                    class: super::town_portal::CLASS.name,
                    at: column,
                    kind,
                });

                scene.world.effects.spawn(Box::new(portal));
            }

            let arriving = !hidden && kind != 5 && opcode != protocol::server::ACT;

            if arriving {
                let at = scene.world.actors.actors[index].position;

                let poof = glam::Vec3::new(at.x, at.y + ARRIVAL_LIFT, at.z);

                let style = poof_style(class);

                let mut start = Vec::new();

                super::start::spawn(poof, style, None, now_ms, &mut start);

                applied.raised[1] = Some(Raised {
                    class: super::mesh_fx::START_CLASS.name,
                    at: poof,
                    kind: style,
                });

                for effect in start {
                    scene.world.effects.spawn(effect);
                }

                if local {
                    scene.world.play_sound(ARRIVAL_SOUND);
                }

                if should_spawn_judgement(kind, class, part0_mesh) {
                    let sunk = glam::Vec3::new(at.x, at.y - JUDGEMENT_SINK, at.z);

                    scene.world.actors.actors[index].position = sunk;

                    let lifted = glam::Vec3::new(at.x, at.y + JUDGEMENT_LIFT, at.z);

                    let built = super::skill::judgement::build_at(
                        lifted,
                        JUDGEMENT_MODE,
                        JUDGEMENT_ARG5,
                        now_ms,
                    );

                    applied.raised[2] = Some(Raised {
                        class: super::skill::judgement::CLASS.name,
                        at: lifted,
                        kind: JUDGEMENT_MODE,
                    });

                    if let Some(sound) = built.sound {
                        scene.world.play_sound(i64::from(sound));
                    }

                    for orphan in built.orphans {
                        scene.world.effects.spawn(Box::new(orphan));
                    }

                    if let Some(mesh) = built.mesh {
                        scene.world.effects.spawn(Box::new(mesh));
                    }

                    scene.world.effects.spawn(Box::new(built.token));
                } else {
                    let column = glam::Vec3::new(at.x, at.y + ARRIVAL_LIFT, at.z);

                    let kind = portal_kind(kind);

                    let portal = super::town_portal::TownPortal::new(column, kind, now_ms);

                    applied.raised[3] = Some(Raised {
                        class: super::town_portal::CLASS.name,
                        at: column,
                        kind,
                    });

                    scene.world.effects.spawn(Box::new(portal));
                }
            }

            if let Some(at) = levelled_at {
                let raised = super::timers::spawn(scene, at, LEVELUP_KIND);

                tracing::info!("{}", super::timers::describe(&raised));

                if !green_name {
                    let column = glam::Vec3::new(at.x, at.y + ARRIVAL_LIFT, at.z);

                    let portal = super::town_portal::TownPortal::new(column, 1, now_ms);

                    applied.raised[4] = Some(Raised {
                        class: super::town_portal::CLASS.name,
                        at: column,
                        kind: 1,
                    });

                    scene.world.effects.spawn(Box::new(portal));
                }
            }
        }
        Order::Nothing => {}
    }

    if (order == Order::Teleport || applied.warped) && departure != action.to() {
        super::actor_effects::relocated(
            &mut scene.world.actors.actors[index],
            &mut scene.world.effects,
            now_ms,
        );
    }

    if order == Order::Teleport
        || (applied.warped
            && matches!(
                scene.world.actors.actors[index].action,
                appearance::ACTION_STAND01
                    | appearance::ACTION_STAND02
                    | appearance::ACTION_WALK
                    | appearance::ACTION_RUN
            ))
    {
        scene
            .world
            .actors
            .play_at(index, appearance::ACTION_STAND02, now_ms);
    }

    if remember_move {
        scene.attack.remember_move(action);
    }

    if local && (applied.warped || order == Order::Teleport) {
        super::gate::arrived(scene);

        let to = action.to();

        let cell = (to.0 as u16, to.1 as u16);

        scene.cc.auto_move = to;

        scene.sent_dest = Some(to);

        scene.attack.from = Some(cell);
    }

    if local && applied.held {
        scene.cc.warp_hold_ms = now_ms;
    }

    if applied.warped {
        if local {
            let cell = action.to();

            let byte = super::attack::attribute_at(scene.world.actors.attributes(), cell);

            scene.gate.last_attribute = byte.unwrap_or(0);
        }

        if opcode == protocol::server::ACT {
            scene
                .world
                .actors
                .play_at(index, appearance::ACTION_STAND02, now_ms);

            if local {
                scene.world.play_sound(STAND_SOUND);
            }
        } else {
            scene
                .world
                .actors
                .play_at(index, appearance::ACTION_LEVELUP, now_ms);
        }

        if local
            && opcode != protocol::server::ACT
            && scene.combat.movement == super::hud::MOVE_PLANS_A_PATH
        {
            let at = scene.world.actors.actors[index].position;

            scene.cc.anchor = (at.x, at.z);

            scene.cc.target = None;
        }

        if local {
            scene.world.actors.refresh_air_move_ground(index);

            super::field::finish_air_move(scene);
        }
    }

    if local && (applied.warped || order == Order::Teleport) {
        let restore_yaw = opcode == protocol::server::ACT || order == Order::Teleport;

        request_reanchor(scene, action.to(), restore_yaw);
    }

    if starts_quest_timer {
        let update = super::quest_timer::start(&mut scene.quest_timer, now_ms);

        if let Some(hud) = scene.hud.as_mut() {
            hud.refresh_quest_timer(&update);
        }
    }

    applied
}

pub fn warp(actor: &mut Actor, field: &HeightField<'_>, to: (i32, i32)) {
    let centre = [to.0 as f32 + 0.5, to.1 as f32 + 0.5];

    let ground = field.ground(to.0, to.1);

    actor.mover.reset_position(centre);

    actor.destination = to;

    actor.position = glam::Vec3::new(centre[0], ground, centre[1]);

    actor.is_dead = false;
}

fn request_reanchor(scene: &mut FieldScene, to: (i32, i32), restore_yaw: bool) {
    let cells = formats::world::ZONE_CELLS as i32;

    let (Ok(bx), Ok(bz)) = (u8::try_from(to.0 / cells), u8::try_from(to.1 / cells)) else {
        tracing::info!(
            "NWYD: warped to ({}, {}), which is off the zone grid",
            to.0,
            to.1
        );

        return;
    };

    scene.world.force_load = Some((bx, bz));

    scene.world.force_load_restore_yaw = restore_yaw.then_some(scene.camera.yaw);
}

pub fn remove(scene: &mut FieldScene, entity: u16) -> Removed {
    scene.hovered_drop = None;

    if entity == 0 {
        return Removed {
            entity,
            index: None,
            gone: false,
            was_player: false,
            was_drop: false,
            was_gate: false,
        };
    }

    let index = scene.world.actors.entity(entity);

    if index.is_some() && index == scene.player {
        return Removed {
            entity,
            index,
            gone: false,
            was_player: true,
            was_drop: false,
            was_gate: false,
        };
    }

    if scene.drops.remove(entity) {
        if scene.pending_pickup == Some(entity) {
            scene.pending_pickup = None;
        }

        return Removed {
            entity,
            index: None,
            gone: true,
            was_player: false,
            was_drop: true,
            was_gate: false,
        };
    }

    if scene.gates.remove(entity) {
        if scene.pending_pickup == Some(entity) {
            scene.pending_pickup = None;
        }

        return Removed {
            entity,
            index: None,
            gone: true,
            was_player: false,
            was_drop: false,
            was_gate: true,
        };
    }

    let Some(index) = index else {
        return Removed {
            entity,
            index: None,
            gone: false,
            was_player: false,
            was_drop: false,
            was_gate: false,
        };
    };

    scene.world.effects.tree.detach_entity(entity);

    scene.world.actors.remove(entity);

    if let Some(player) = scene.player.as_mut()
        && *player > index
    {
        *player -= 1;
    }

    if scene.hovered == Some(entity) {
        scene.hovered = None;
    }

    if scene.target == Some(entity) {
        scene.target = None;
    }

    scene.attack.despawned(entity);

    if let Some(queued) = scene.cc.queued.as_mut()
        && queued.target == Some(entity)
    {
        queued.target = None;
    }

    if scene.cc.target == Some(entity) {
        scene.cc.target = None;
    }

    scene.roster.answered(entity);

    super::nameplate::forget(scene, entity);

    Removed {
        entity,
        index: Some(index),
        gone: true,
        was_player: false,
        was_drop: false,
        was_gate: false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Removed {
    pub entity: u16,
    pub index: Option<usize>,
    pub gone: bool,
    pub was_player: bool,
    pub was_drop: bool,
    pub was_gate: bool,
}

pub fn describe(applied: &Applied) -> Option<String> {
    match (applied.index, applied.order) {
        (None, _) => None,
        (Some(_), Order::Act(kind)) => Some(format!(
            "NWYD: entity {} warped to ({}, {}) on sub-kind {kind}{}",
            applied.entity,
            applied.at.0,
            applied.at.1,
            match (applied.afterimage, applied.levelled, applied.held) {
                (true, _, _) => ", leaving a EffectSkinMesh afterimage on the departure cell",
                (false, true, _) => ", with its EffectLevelUp burst on the arrival cell",
                (false, false, true) => ", holding the macro for two seconds",
                (false, false, false) => "",
            },
        )),
        (Some(_), Order::Nothing) => Some(format!(
            "NWYD: entity {} sent a negative sub-kind, which the original also ignores",
            applied.entity,
        )),
        (Some(_), Order::Teleport) => Some(format!("NWYD: entity {} warped", applied.entity)),
        (Some(_), Order::Walk) => match applied.walked {
            true => None,
            false => Some(format!(
                "NWYD: entity {} was told to move somewhere the local grid cannot reach",
                applied.entity,
            )),
        },
    }
}

pub fn describe_removal(removed: &Removed) -> Option<String> {
    match (removed.gone, removed.was_player) {
        (true, _) => None,
        (false, true) => Some(format!(
            "NWYD: 0x16F named the local player ({}) — refused, that would leave no body at all",
            removed.entity,
        )),
        (false, false) => Some(format!(
            "NWYD: 0x16F for entity {} — no body in this world carries that id",
            removed.entity,
        )),
    }
}
