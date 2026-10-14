use super::dispatch::BatchEffects;
use crate::scene::{Scene, drops, placed};
use assets::GameDir;
use formats::net::{animation_probe, gate_command, ground_item};

pub enum Packet {
    GroundItem(Box<ground_item::Drop>),
    GateCommand(gate_command::Command),
    AnimationProbe(animation_probe::Request),
}

pub struct State<'a> {
    pub scene: &'a mut Scene,
    pub game: Option<&'a GameDir>,
}

pub trait Host {
    fn state(&mut self) -> State<'_>;

    fn refresh_screens(&mut self);

    fn redraw(&mut self);
}

pub fn apply(host: &mut impl Host, packet: Packet, effects: &mut BatchEffects) {
    match packet {
        Packet::GroundItem(drop) => {
            let state = host.state();

            let Some(game) = state.game else {
                return;
            };

            match state.scene.ground_item(game, &drop) {
                Some(Ok(arrival)) => {
                    tracing::info!(
                        "{}",
                        match &arrival {
                            placed::Arrival::Drop(placed) => drops::describe(placed),
                            placed::Arrival::Gate(placed) => placed::describe(placed),
                        }
                    );

                    effects.request_upload();
                }
                Some(Err(error)) => {
                    tracing::info!("NWYD: 0x26E for entity {}: {error:#}", drop.entity)
                }
                None => {
                    tracing::info!(
                        "NWYD: 0x26E for entity {} arrived with no world up",
                        drop.entity
                    )
                }
            }
        }
        Packet::GateCommand(command) => {
            match host.state().scene.gate_command(&command) {
                Some(commanded) => {
                    tracing::info!("{}", placed::describe_commanded(&commanded));

                    host.redraw();
                }
                None => {
                    tracing::info!(
                        "NWYD: 0x374 for entity {} named no gate this world holds",
                        command.entity
                    )
                }
            }

            host.refresh_screens();
        }
        Packet::AnimationProbe(request) => {
            let state = host.state();

            let Some(game) = state.game else {
                tracing::info!(
                    "NWYD: 0x1C1 arrived on a capture path with no install; no reply sent"
                );

                return;
            };

            match state.scene.animation_probe(game, request) {
                Some(Some(response)) => {
                    tracing::info!(
                        "NWYD: 0x1C1 model {}, byte offset {} answered with {}",
                        response.model,
                        response.byte_offset,
                        response.result,
                    )
                }
                Some(None) => {
                    tracing::info!(
                        "NWYD: 0x1C1 model {}, byte offset {} had no loadable animation row; no reply sent",
                        request.model,
                        request.byte_offset,
                    )
                }
                None => {
                    tracing::info!("NWYD: 0x1C1 arrived with no world up; no reply sent")
                }
            }
        }
    }
}
