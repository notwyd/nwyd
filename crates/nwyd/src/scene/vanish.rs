use super::corpse::{MODE_ARMED, MODE_DEAD, MODE_NONE, MODE_VANISHING};
use super::field::FieldScene;
use formats::net::live::Vanish;

pub const IMMUNE_PET: u16 = 0x301;

pub const DIE_MODEL: u32 = 0x25;

pub const VANISH_DELAY_MS: u32 = 1_000;

pub const PORTAL_COLOUR_MOB: u32 = 0x00FF_5500;

pub const PORTAL_COLOUR_CHARACTER: u32 = 0xFFFF_FFFF;

pub const PORTAL_LIFE_MS: u32 = 1_000;

pub const PORTAL_SOUND: u32 = 0x9F;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Vanished {
    pub entity: u16,
    pub index: Option<usize>,
    pub mode: i32,
    pub gone: bool,
    pub fell: bool,
    pub armed: bool,
    pub already: bool,
    pub stamped: bool,
    pub ignored: bool,
    pub was_player: bool,
    pub dropped_target: bool,
    pub panels: PanelTeardown,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct PanelTeardown {
    pub pgt_menu: bool,
    pub item_shop: bool,
    pub store: bool,
    pub party: bool,
}

impl PanelTeardown {
    pub fn any(self) -> bool {
        self.pgt_menu || self.item_shop || self.store || self.party
    }
}

impl Vanished {
    pub fn redraws(&self) -> bool {
        self.gone || self.fell || self.stamped || self.panels.any()
    }
}

pub fn apply(scene: &mut FieldScene, entity: u16, packet: &Vanish, now_ms: u32) -> Vanished {
    let mode = packet.mode;

    let blank = Vanished {
        entity,
        index: None,
        mode,
        gone: false,
        fell: false,
        armed: false,
        already: false,
        stamped: false,
        ignored: false,
        was_player: false,
        dropped_target: false,
        panels: PanelTeardown::default(),
    };

    let Some(index) = scene.world.actors.entity(entity) else {
        return blank;
    };

    let out = Vanished {
        index: Some(index),
        ..blank
    };

    if scene.player == Some(index) {
        if scene.world.actors.actors[index].status.hp > 0 {
            return Vanished {
                ignored: true,
                was_player: true,
                ..out
            };
        }

        let fell = super::kill::die(scene, index, now_ms);

        return Vanished {
            fell,
            was_player: true,
            ..out
        };
    }

    let panels = teardown_panels(scene, entity);

    let out = Vanished { panels, ..out };

    let already = scene.world.actors.actors[index].removal_mode;

    if already != MODE_NONE {
        if already != MODE_DEAD {
            return remove(scene, entity, index, out);
        }

        if scene.world.actors.actors[index].corpse_ms.is_some() {
            return Vanished {
                already: true,
                ..out
            };
        }

        scene.world.actors.actors[index].is_dead = false;

        let fell = super::kill::die(scene, index, now_ms);

        let actor = &mut scene.world.actors.actors[index];

        let stamped = actor.corpse_ms.is_none();

        if stamped {
            actor.corpse_ms = Some(now_ms.wrapping_add(VANISH_DELAY_MS));
        }

        return Vanished {
            fell,
            already: true,
            stamped,
            ..out
        };
    }

    if mode == MODE_ARMED {
        scene.world.actors.actors[index].removal_mode = MODE_ARMED;

        return Vanished { armed: true, ..out };
    }

    if mode == MODE_DEAD || scene.world.actors.actors[index].model_id == DIE_MODEL {
        {
            let actor = &mut scene.world.actors.actors[index];

            actor.removal_mode = mode;

            if actor.corpse_ms.is_none() {
                actor.corpse_ms = Some(now_ms);
            }

            actor.status.hp = 0;
        }

        let fell = super::kill::die(scene, index, now_ms);

        return Vanished {
            fell,
            stamped: true,
            ..out
        };
    }

    if mode != MODE_VANISHING {
        super::start::vanishing(scene, index, now_ms, true);

        return remove(scene, entity, index, out);
    }

    super::start::vanishing(scene, index, now_ms, false);

    let actor = &mut scene.world.actors.actors[index];

    actor.removal_mode = MODE_VANISHING;

    if actor.corpse_ms.is_none() {
        actor.corpse_ms = Some(now_ms.wrapping_add(VANISH_DELAY_MS));
    }

    Vanished {
        stamped: true,
        ..out
    }
}

pub(super) fn teardown_panels(scene: &mut FieldScene, entity: u16) -> PanelTeardown {
    use super::panels::{CONTENT_PGT_MENU, CONTENT_STORE};

    let mut out = PanelTeardown::default();

    if scene.target == Some(entity)
        && let Some(panels) = scene.panels.as_mut()
        && panels.is_open(CONTENT_PGT_MENU)
    {
        panels.set_open(CONTENT_PGT_MENU, false);

        out.pgt_menu = true;
    }

    let item_shop = scene.panels.as_ref().is_some_and(|panels| {
        panels.shop_id == entity && panels.is_open(super::panels::CONTENT_ITEM_SHOP)
    });

    if item_shop && let Some(panels) = scene.panels.as_mut() {
        out.item_shop = super::shop::close(panels);
    }

    let store = scene.panels.as_ref().is_some_and(|panels| {
        panels.is_open(CONTENT_STORE)
            && panels
                .store
                .as_ref()
                .is_some_and(|contents| contents.owner == entity)
    });

    if store {
        let closed = super::store::close(scene);

        out.store = !matches!(closed, super::store::Closed::WasShut);

        debug_assert!(
            !closed.wire(),
            "a nonlocal body's vanish cannot close a shop owned by the local id"
        );
    }

    out.party = super::party::vanished(scene, entity);

    out
}

fn remove(scene: &mut FieldScene, entity: u16, index: usize, out: Vanished) -> Vanished {
    let _ = index;

    let dropped_target = scene.attack.auto_target == Some(entity);

    if dropped_target {
        scene.attack.auto_target = None;
    }

    let removed = super::action::remove(scene, entity);

    Vanished {
        gone: removed.gone,
        dropped_target,
        ..out
    }
}

pub fn reap_idle(scene: &mut FieldScene) -> Vec<u16> {
    let due: Vec<u16> = scene
        .world
        .actors
        .actors
        .iter()
        .enumerate()
        .filter(|(index, actor)| {
            Some(*index) != scene.player
                && actor.entity != 0
                && actor.removal_mode == super::corpse::MODE_ARMED
                && matches!(
                    actor.action,
                    formats::mesh::appearance::ACTION_STAND01
                        | formats::mesh::appearance::ACTION_STAND02
                )
        })
        .map(|(_, actor)| actor.entity)
        .collect();

    for &entity in &due {
        if scene.attack.auto_target == Some(entity) {
            scene.attack.auto_target = None;
        }

        super::action::remove(scene, entity);
    }

    due
}

pub fn describe(applied: &Vanished) -> String {
    let mut line = format!(
        "NWYD: 0x165 — entity {}, mode {}",
        applied.entity, applied.mode
    );

    if applied.index.is_none() {
        line.push_str(" (no body in this world carries that id)");

        return line;
    }

    if applied.was_player {
        line.push_str(match applied.ignored {
            true => " — that is us, and we still have health, so it is ignored",
            false => " — that is us; the body falls and is never removed",
        });

        return line;
    }

    line.push_str(match applied {
        _ if applied.gone => " — gone",
        _ if applied.armed => " — armed, and still standing",
        _ if applied.already && !applied.stamped => {
            " — it was already counting down, so the packet was ignored"
        }
        _ if applied.fell => " — killed, and counting down",
        _ if applied.stamped => " — counting down; it had already fallen, so nothing replayed",
        _ => " — nothing changed",
    });

    if applied.dropped_target {
        line.push_str("; the auto-attack let go of it");
    }

    line
}
