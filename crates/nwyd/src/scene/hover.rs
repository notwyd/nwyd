use super::actors::Actor;
use super::field::FieldScene;
use super::pick::{self, Ray};

pub const PROXIMITY: f32 = 1.5;

pub use super::nameplate::PLAYER_ID_LIMIT;

pub const CORPSE_ACTIONS: [usize; 2] = [0xC, 0xB];

pub const ARMED_CORPSE_ACTION: u8 = 0x1F;

pub fn distance(a: (f32, f32), b: (f32, f32)) -> f32 {
    let (dx, dz) = (a.0 - b.0, a.1 - b.1);

    (dx * dx + dz * dz).sqrt()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Gate {
    Parked,
    Clear,
    Release,
    Test,
}

pub fn gate(actor: &Actor, is_player: bool, viewer_sees_hidden: bool, armed: u8) -> Gate {
    if actor.flags.get(0xB55) != 0 || actor.entity < PLAYER_ID_LIMIT && actor.status.flags & 1 != 0
    {
        return Gate::Clear;
    }

    if actor.flags.invisible() && !is_player && !viewer_sees_hidden {
        return Gate::Clear;
    }

    let special = CORPSE_ACTIONS.contains(&actor.action) || actor.is_dead;

    match armed == ARMED_CORPSE_ACTION {
        true => match special {
            true => Gate::Test,
            false => Gate::Release,
        },
        false => match special {
            true => Gate::Release,
            false => Gate::Test,
        },
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Hovered {
    pub entity: Option<u16>,
    pub changed: bool,
}

pub fn update(scene: &mut FieldScene) -> Hovered {
    scene.name_hover.prune(&scene.world.actors.actors);

    let before = scene.hovered;

    let Some((view, projection)) = scene.projected else {
        return Hovered {
            entity: before,
            changed: false,
        };
    };

    let viewport = (scene.layout.width, scene.layout.height);

    let ray = Ray::unproject(&view, &projection, scene.cursor, viewport);

    let eye = (scene.camera.eye.x, scene.camera.eye.z);

    let player = scene
        .player
        .and_then(|i| scene.world.actors.actors.get(i))
        .map(|a| a.entity);

    let mut hovered = before;

    let armed = formats::net::enter::selected(&scene.entered.quick, scene.entered.quick_selected)
        .unwrap_or(0);

    for index in (0..scene.world.actors.actors.len()).rev() {
        let actor = &scene.world.actors.actors[index];

        if !actor.camera_visible {
            continue;
        }

        let is_player = player == Some(actor.entity);

        let prev = hovered;

        let hit = match gate(actor, is_player, false, armed) {
            Gate::Parked => continue,
            Gate::Clear => {
                scene.world.actors.actors[index].highlight = false;

                continue;
            }
            Gate::Release => {
                if hovered == Some(actor.entity) {
                    hovered = None;
                }

                scene.world.actors.actors[index].highlight = false;

                continue;
            }
            Gate::Test => {
                let mut shape = body(actor);

                if let Some(slot) = shape.weapon.checked_sub(2888).filter(|slot| *slot < 2)
                    && !scene.world.actors.siege_meshes[slot as usize]
                {
                    shape.weapon = 0;
                }

                scene.name_hover.contains(actor) || pick::hits(&ray, &shape)
            }
        };

        let here = (actor.position.x, actor.position.z);

        let entity = actor.entity;

        if !is_player {
            if !hit {
                if hovered == Some(entity) {
                    hovered = None;
                }

                scene.world.actors.actors[index].highlight = false;

                continue;
            }

            let nearer = match prev.and_then(|id| position_of(scene, id)) {
                None => true,
                Some(at) => distance(eye, here) < distance(eye, at),
            };

            if nearer {
                claim(scene, entity, &mut hovered);
            }
        } else if !hit {
            scene.world.actors.actors[index].highlight = false;

            continue;
        }

        let mut kept = true;

        if distance(eye, here) < PROXIMITY {
            kept = false;

            hovered = prev;
        } else if let Some(prev_id) = prev
            && !is_player
            && entity < PLAYER_ID_LIMIT
            && prev_id >= PLAYER_ID_LIMIT
            && tagged(scene, prev_id)
        {
            claim(scene, entity, &mut hovered);
        }

        scene.world.actors.actors[index].highlight = kept;
    }

    scene.hovered = hovered;

    scene.world.actors.hovered = hovered;

    Hovered {
        entity: hovered,
        changed: hovered != before,
    }
}

pub fn tagged(scene: &FieldScene, entity: u16) -> bool {
    let Some(index) = scene.world.actors.entity(entity) else {
        return false;
    };

    let actor = &scene.world.actors.actors[index];

    actor.green_name
}

fn claim(scene: &mut FieldScene, entity: u16, hovered: &mut Option<u16>) {
    if let Some(old) = *hovered
        && let Some(index) = scene.world.actors.entity(old)
    {
        scene.world.actors.actors[index].highlight = false;
    }

    *hovered = Some(entity);

    if let Some(index) = scene.world.actors.entity(entity) {
        scene.world.actors.actors[index].highlight = true;
    }
}

fn position_of(scene: &FieldScene, entity: u16) -> Option<(f32, f32)> {
    let index = scene.world.actors.entity(entity)?;

    let at = scene.world.actors.actors[index].position;

    Some((at.x, at.z))
}

pub fn body(actor: &Actor) -> pick::Body {
    pick::Body {
        position: actor.position,
        model: match actor.mounted {
            true => actor.mount_model.unwrap_or(actor.model_id),
            false => actor.model_id,
        },
        scale: actor.presentation_scale(),
        weapon: weapon_mesh(actor),
        billboard: actor.class == 0x38 && actor.parts[0].mesh == 0,
    }
}

pub fn weapon_mesh(actor: &Actor) -> u16 {
    actor.parts[7].mesh as u16
}
