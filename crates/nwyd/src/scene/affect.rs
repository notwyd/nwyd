use super::field::FieldScene;
use formats::net::affect::{Affects, Wearer};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Applied {
    pub entity: u16,
    pub index: Option<usize>,
    pub local: bool,
    pub active: usize,
    pub invisible: bool,
}

pub fn apply(scene: &mut FieldScene, entity: u16, affects: &Affects) -> Applied {
    let index = scene.world.actors.entity(entity);

    let active = affects
        .details
        .iter()
        .filter(|detail| detail.is_set())
        .count();

    let local = index.is_some() && index == scene.player;

    let Some(index) = index.filter(|_| local) else {
        return Applied {
            entity,
            index,
            local,
            active,
            invisible: false,
        };
    };

    scene.affect_stamps = [scene.time.now_ms; formats::net::affect::AFFECTS];

    let actor = &mut scene.world.actors.actors[index];

    let wearer = Wearer {
        class: actor.class,
        part0_mesh: actor.parts[0].mesh as u16,
    };

    affects.apply(&mut actor.details);

    let affects_on_body = actor.affects;

    actor
        .flags
        .rebuild(&affects_on_body, &actor.details, &wearer);

    affects.apply_hide_flags(&mut actor.flags);

    let flags = actor.flags;

    scene.world.actors.apply_affects(index, &flags);

    if affects.details.iter().any(|detail| detail.kind == 44) {
        scene.automatic_movement = false;
    }

    if scene.affect_time.query_due(scene.time.now_ms) {
        scene
            .outbox
            .push(super::affect_time::query_frame(scene.entered.client_id));
    }

    scene.entered.refresh_class();

    Applied {
        entity,
        index: Some(index),
        local,
        active,
        invisible: flags.invisible(),
    }
}

pub fn describe(applied: &Applied) -> String {
    match (applied.index, applied.local) {
        (None, _) => {
            format!(
                "NWYD: 0x3B9 for entity {} — no body in this world carries that id",
                applied.entity,
            )
        }
        (Some(_), false) => {
            format!(
                "NWYD: 0x3B9 for entity {} — declined, the update applies to the local player only",
                applied.entity,
            )
        }
        (Some(_), true) => {
            format!(
                "NWYD: 0x3B9 — {} affect(s) on the player{}",
                applied.active,
                match applied.invisible {
                    true => ", invisible",
                    false => "",
                },
            )
        }
    }
}
