use super::dispatch::BatchEffects;
use crate::scene::{Scene, enter::Warehouse, live};
use assets::GameDir;
use formats::net::live as wire;

pub enum Packet {
    QuickBar {
        bar: [u8; formats::net::enter::QUICK_SLOTS],
    },
    Vitals {
        entity: u16,
        vitals: wire::Vitals,
    },
    HpDelta {
        entity: u16,
        delta: wire::HpDelta,
    },
    ActorStatus {
        entity: u16,
        score: Box<wire::ActorStatus>,
    },
    PlayerProgression {
        entity: u16,
        score: wire::PlayerProgression,
    },
    Gold {
        entity: u16,
        gold: wire::Gold,
    },
    EquipChanged {
        entity: u16,
        change: Box<wire::EquipChange>,
    },
    SlotPatch {
        entity: u16,
        patch: wire::SlotPatch,
    },
}

pub struct State<'a> {
    pub scene: &'a mut Scene,
    pub warehouse: Option<&'a mut Warehouse>,
    pub game: Option<&'a GameDir>,
}

pub trait Host {
    fn state(&mut self) -> State<'_>;

    fn broadcast_panels(&mut self);

    fn refresh_screens(&mut self);

    fn load_panel_icons(&mut self);

    fn redraw(&mut self);
}

pub fn apply(host: &mut impl Host, packet: Packet, effects: &mut BatchEffects) {
    match packet {
        Packet::QuickBar { bar } => match host.state().scene.quick_bar(&bar) {
            Some(applied) => {
                tracing::info!("{}", live::describe_quick_bar(&applied));

                if applied.changed {
                    host.refresh_screens();

                    host.redraw();
                }
            }
            None => tracing::info!("NWYD: 0x378 arrived with no world up"),
        },
        Packet::Vitals { entity, vitals } => match host.state().scene.vitals(entity, &vitals) {
            Some(applied) => {
                tracing::info!("{}", live::describe_vitals(&applied));

                if applied.broadcast {
                    host.broadcast_panels();
                }

                if applied.index.is_some() {
                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: 0x181 for entity {entity} arrived with no world up")
            }
        },
        Packet::HpDelta { entity, delta } => match host.state().scene.hp_delta(entity, &delta) {
            Some(applied) => {
                tracing::info!("{}", live::describe_hp_delta(&applied));

                if applied.broadcast {
                    host.broadcast_panels();
                }

                if applied.index.is_some() {
                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: 0x18A for entity {entity} arrived with no world up")
            }
        },
        Packet::ActorStatus { entity, score } => {
            match host.state().scene.apply_actor_status(entity, &score) {
                Some(applied) => {
                    tracing::info!("{}", live::describe_actor_status(&applied));

                    if applied.local {
                        host.refresh_screens();
                    }

                    if applied.index.is_some() {
                        host.redraw();
                    }
                }
                None => {
                    tracing::info!("NWYD: 0x336 for entity {entity} arrived with no world up")
                }
            }
        }
        Packet::PlayerProgression { entity, score } => {
            match host.state().scene.apply_player_progression(entity, &score) {
                Some(applied) => {
                    tracing::info!("{}", live::describe_player_progression(&applied));

                    if applied.local {
                        host.refresh_screens();

                        host.redraw();
                    }
                }
                None => {
                    tracing::info!("NWYD: 0x337 for entity {entity} arrived with no world up")
                }
            }
        }
        Packet::Gold { entity, gold } => match host.state().scene.gold(entity, &gold) {
            Some(applied) => {
                tracing::info!("{}", live::describe_gold(&applied));

                if applied.local {
                    host.refresh_screens();

                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: 0x3AF for entity {entity} arrived with no world up")
            }
        },
        Packet::EquipChanged { entity, change } => {
            let state = host.state();

            let Some(game) = state.game else {
                return;
            };

            match state.scene.equip_changed(game, entity, &change) {
                Some(Ok(applied)) => {
                    tracing::info!("{}", live::describe_equip(&applied));

                    if applied.redressed {
                        effects.request_upload();
                    }

                    if applied.local {
                        host.refresh_screens();
                    }

                    host.redraw();
                }
                Some(Err(error)) => {
                    tracing::info!("NWYD: 0x36B for entity {entity} — {error:#}")
                }
                None => {
                    tracing::info!("NWYD: 0x36B for entity {entity} arrived with no world up")
                }
            }
        }
        Packet::SlotPatch { entity, patch } => {
            let state = host.state();

            if let Some(written) = state.scene.selection_slot_patch(state.warehouse, &patch) {
                tracing::debug!(
                    event = "selection_cargo_patch",
                    entity,
                    slot = patch.slot,
                    written,
                    "character-selection item patch consumed"
                );

                return;
            }

            let Some(game) = state.game else {
                return;
            };

            match state.scene.slot_patch(game, entity, &patch) {
                Some(Ok(applied)) => {
                    tracing::info!("{}", live::describe_patch(&applied));

                    if applied.redressed {
                        effects.request_upload();
                    }

                    if applied.index.is_some() {
                        host.refresh_screens();
                    }

                    if applied.written {
                        host.load_panel_icons();
                    }

                    host.redraw();
                }
                Some(Err(error)) => {
                    tracing::info!("NWYD: 0x182 for entity {entity} — {error:#}")
                }
                None => {
                    tracing::info!("NWYD: 0x182 for entity {entity} arrived with no world up")
                }
            }
        }
    }
}
