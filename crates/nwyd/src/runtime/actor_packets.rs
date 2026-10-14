use super::dispatch::BatchEffects;
use super::identity::EntityId;
use crate::scene::{Scene, action, vanish};
use assets::GameDir;
use formats::net::{action::Action, live::Vanish, spawn::Spawn};

pub enum Packet {
    Spawn {
        opcode: u16,
        entity: Box<Spawn>,
    },
    Action {
        opcode: u16,
        entity: EntityId,
        action: Box<Action>,
    },
    Vanish {
        entity: EntityId,
        vanish: Vanish,
    },
    Remove {
        entity: EntityId,
    },
}

pub struct State<'a> {
    pub scene: &'a mut Scene,
    pub game: Option<&'a GameDir>,
}

pub trait Host {
    fn state(&mut self) -> State<'_>;

    fn refresh_party(&mut self);

    fn redraw(&mut self);
}

pub fn apply(host: &mut impl Host, packet: Packet, effects: &mut BatchEffects) {
    match packet {
        Packet::Spawn { opcode, entity } => {
            let state = host.state();

            let Some(game) = state.game else {
                return;
            };

            let id = entity.id;

            let landed = state.scene.spawn(game, opcode, *entity);

            if matches!(& landed, Some(Ok(spawned)) if spawned.index.is_some()) {
                state.scene.answered(id);
            }

            if matches!(& landed, Some(Ok(spawned)) if spawned.panels.party) {
                host.refresh_party();
            }

            match landed {
                Some(Ok(spawned)) if spawned.index.is_some() => {
                    if spawned.undrawable {
                        tracing::info!(
                            "NWYD: {opcode:#05x} entity {id} was placed on a model with no \
                             drawable part — it will wear a nameplate over nothing",
                        );
                    }

                    if !spawned.was_armed {
                        effects.request_upload();
                    }

                    host.redraw();
                }
                Some(Ok(spawned)) => {
                    tracing::info!(
                        "NWYD: {opcode:#05x} entity {id} not placed — its appearance did not resolve",
                    );

                    if spawned.panels.any() {
                        host.redraw();
                    }
                }
                Some(Err(error)) => {
                    tracing::info!("NWYD: {opcode:#05x} entity {id} not placed — {error:#}");
                }
                None => {
                    tracing::info!("NWYD: {opcode:#05x} entity {id} arrived with no world up")
                }
            }
        }
        Packet::Action {
            opcode,
            entity,
            action: movement,
        } => match host.state().scene.action(opcode, entity.wire(), &movement) {
            Some(applied) => {
                if let Some(line) = action::describe(&applied) {
                    tracing::info!("{line}");
                }

                if applied.walked {
                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: {opcode:#05x} for entity {entity} arrived with no world up")
            }
        },
        Packet::Vanish {
            entity,
            vanish: removal,
        } => match host.state().scene.vanish(entity.wire(), &removal) {
            Some(applied) => {
                tracing::info!("{}", vanish::describe(&applied));

                if applied.panels.party {
                    host.refresh_party();
                }

                if applied.redraws() {
                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: 0x165 for entity {entity} arrived with no world up")
            }
        },
        Packet::Remove { entity } => match host.state().scene.remove(entity.wire()) {
            Some(removed) => {
                if let Some(line) = action::describe_removal(&removed) {
                    tracing::info!("{line}");
                }

                if removed.gone {
                    host.redraw();
                }
            }
            None => {
                tracing::info!("NWYD: 0x16F for entity {entity} arrived with no world up")
            }
        },
    }
}
