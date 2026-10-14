use super::{lifecycle::Retained, screens::Screens};
use crate::{
    clock::RawTime,
    net::Session,
    scene::{Scene, enter::Entered, field, listener::Picked},
};
use formats::net::{account, enter::EnterWorld};

pub enum Packet {
    LoginAccepted {
        block: Box<account::Block>,
        account: String,
    },
    LeaveWorld {
        client_id: u16,
    },
    EnterWorld {
        world: Box<EnterWorld>,
        received_raw_ms: RawTime,
    },
}

pub struct State<'a> {
    pub scene: &'a mut Scene,
    pub session: &'a mut Session,
    pub retained: &'a mut Retained,
    pub screens: Option<&'a dyn Screens>,
    pub picked: Option<&'a Picked>,
}

pub enum Prepared {
    Selection(Scene),
    World(Scene),
}

pub fn prepare(state: State<'_>, packet: Packet) -> Option<Prepared> {
    state.retained.remember_selection_body(state.scene);

    match packet {
        Packet::LoginAccepted { mut block, account } => {
            state.scene.login_accepted();

            state.retained.memo_inbox.clear();

            tracing::info!(
                "NWYD: logged in as {account:?} — {} of {} slot(s) used",
                block.occupied(),
                account::SLOTS,
            );

            let screens = state.screens?;

            state.scene.retire_account_experience(&mut block);

            state.session.replace_block(block);

            state.retained.prepare_selection(state.session);

            match screens.select_char(state.session.block.as_deref(), state.retained.account_lock) {
                Ok(scene) => Some(Prepared::Selection(scene)),
                Err(error) => {
                    tracing::info!("NWYD: could not build the character list: {error:#}");

                    None
                }
            }
        }
        Packet::LeaveWorld { client_id } => {
            let Some(block) = state.session.block.as_deref_mut() else {
                tracing::info!("NWYD: 0x116 arrived with no retained account block");

                return None;
            };

            let Some(applied) = state.scene.leave_world(client_id, block) else {
                tracing::info!("NWYD: 0x116 for client id {client_id} did not match a live field");

                return None;
            };

            state.scene.retire_account_experience(block);

            if let Scene::Field(field) = state.scene {
                state.session.warehouse = Some(Box::new(field.entered.warehouse));
            }

            state.session.left_world();

            let Some(screens) = state.screens else {
                tracing::info!(
                    "NWYD: 0x116 wrote slot {} back, but this run has no character-list factory",
                    applied.slot,
                );

                return None;
            };

            state.retained.prepare_selection(state.session);

            match screens.select_char(state.session.block.as_deref(), state.retained.account_lock) {
                Ok(scene) => {
                    tracing::info!(
                        "NWYD: 0x116 wrote slot {} back and returned to the character list",
                        applied.slot,
                    );

                    Some(Prepared::Selection(scene))
                }
                Err(error) => {
                    tracing::info!("NWYD: could not rebuild the character list: {error:#}");

                    None
                }
            }
        }
        Packet::EnterWorld {
            world,
            received_raw_ms,
        } => {
            let screens = state.screens?;

            let entered = Entered::from_packet(
                &world,
                state.session.slot.unwrap_or_default().index(),
                state.session.block.as_deref(),
            )
            .with_warehouse(
                state
                    .session
                    .warehouse
                    .as_deref()
                    .copied()
                    .unwrap_or_default(),
            );

            tracing::info!(
                "NWYD: entering the world as {:?} at ({}, {}) in {}",
                entered.name(),
                entered.x,
                entered.z,
                entered.zone_name(),
            );

            match screens.field(&entered) {
                Ok(mut scene) => {
                    if let Scene::Field(field) = &mut scene {
                        field.analytics = state.session.analytics.clone();

                        if let Some(picked) = state.picked {
                            field::adopt_session(field, picked);
                        }

                        if let Some(panels) = field.panels.as_mut() {
                            panels.analytics = state.session.analytics.clone();

                            panels.client_config = state.session.client_config.clone();

                            panels.guild_proxy = state.session.guild_proxy.clone();

                            panels.guild_identity.account =
                                state.session.guild_account().to_owned();
                        }

                        field
                            .affect_time
                            .seed(world.time, received_raw_ms.as_millis());
                    }

                    Some(Prepared::World(scene))
                }
                Err(error) => {
                    tracing::info!("NWYD: could not build the field screen: {error:#}");

                    None
                }
            }
        }
    }
}
