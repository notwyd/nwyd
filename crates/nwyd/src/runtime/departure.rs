use super::{dispatch::PacketFlow, lifecycle::Retained, screens::Screens};
use crate::{
    net::Session,
    scene::{Scene, field::RedirectFollow, kickout, listener::Picked, redirect},
};
use formats::net::kickout::Kickout;
use protocol::server::Redirect;

pub enum Packet {
    Redirect(Redirect),
    Disconnected { reason: String, socket_closed: bool },
    Kickout(Kickout),
    Quit,
}

pub struct State<'a> {
    pub scene: &'a mut Scene,
    pub session: &'a mut Session,
    pub retained: &'a mut Retained,
    pub picked: Option<&'a mut Picked>,
    pub screens: Option<&'a dyn Screens>,
    pub launcher_login: bool,
}

pub trait Host {
    fn state(&mut self) -> State<'_>;

    fn game_time(&self) -> u32;

    fn reconnect(&mut self, address: &str, ticket: &Redirect) -> Result<(), String>;

    fn disconnect(&mut self);

    fn show(&mut self, scene: Scene);

    fn sync_ime(&mut self);

    fn redraw(&mut self);

    fn show_message(&mut self, record: usize);
}

pub fn show_server_select(host: &mut impl Host) {
    let result = {
        let state = host.state();

        let Some(screens) = state.screens else {
            tracing::info!("NWYD: this run has no way back to the server list (a capture path)");

            return;
        };

        screens.server_select()
    };

    match result {
        Ok(scene) => {
            tracing::info!("NWYD: back at the server list");

            host.show(scene);
        }
        Err(error) => {
            tracing::info!("NWYD: could not rebuild the server-select screen: {error:#}")
        }
    }
}

fn redirect_failed(host: &mut impl Host) {
    let now = host.game_time();

    host.state().scene.redirect_failed_message(now);

    host.disconnect();

    if !matches!(
        host.state().scene,
        Scene::ServerSelect(_) | Scene::LoginPanels(_)
    ) {
        show_server_select(host);
    }
}

fn follow_redirect(host: &mut impl Host, follow: RedirectFollow) {
    {
        let state = host.state();

        state.retained.name_prompt_required = false;

        if let Ok(channel) = usize::try_from(follow.channel)
            && let Some(picked) = state.picked
        {
            picked.channel = channel;
        }
    }

    let Some(address) = follow.address.as_deref() else {
        tracing::info!(
            "NWYD: redirect channel {} has no address in the current server record",
            follow.channel,
        );

        redirect_failed(host);

        return;
    };

    match host.reconnect(address, &follow.ticket) {
        Ok(()) => {
            let state = host.state();

            if let Scene::Field(field) = state.scene {
                field.analytics = state.session.analytics.clone();

                if let Some(panels) = field.panels.as_mut() {
                    panels.guild_identity.account = state.session.guild_account().to_owned();
                }
            }

            tracing::info!(
                "NWYD: following 0x52A on channel {} at {address}",
                follow.channel
            );
        }
        Err(error) => {
            tracing::info!("NWYD: redirect failed — {error}");

            redirect_failed(host);
        }
    }
}

fn apply_kickout(host: &mut impl Host, packet: Kickout) {
    let now = host.game_time();

    let applied = {
        let state = host.state();

        state.scene.kickout(&packet, state.launcher_login, now)
    };

    let Some(applied) = applied else {
        tracing::info!(
            "NWYD: 0x292 session teardown (code {}) arrived with no world up",
            packet.code(),
        );

        return;
    };

    if matches!(
        applied,
        kickout::Applied::Characters { .. } | kickout::Applied::Disconnected { .. }
    ) {
        host.state().scene.discard_spatial_outbox();
    }

    tracing::info!("{}", kickout::describe(&applied));

    if let kickout::Applied::Disconnected {
        banner: Some(banner),
        ..
    } = &applied
    {
        host.show_message(banner.record);
    }

    match applied.scene() {
        None => host.redraw(),
        Some(formats::net::kickout::SCENE_SELECT_CHAR) => {
            let result = {
                let state = host.state();

                let Some(screens) = state.screens else {
                    tracing::info!("NWYD: no way back to the character list (a capture path)");

                    return;
                };

                if let Some(block) = state.session.block.as_deref_mut() {
                    state.scene.retire_account_experience(block);
                }

                state.retained.remember_selection_body(state.scene);

                state.retained.prepare_selection(state.session);

                screens.select_char(state.session.block.as_deref(), state.retained.account_lock)
            };

            match result {
                Ok(scene) => host.show(scene),
                Err(error) => {
                    tracing::info!("NWYD: could not build the character list: {error:#}")
                }
            }
        }
        Some(_) => show_server_select(host),
    }
}

pub fn apply(host: &mut impl Host, packet: Packet) -> PacketFlow {
    match packet {
        Packet::Redirect(ticket) => {
            match host.state().scene.cache_redirect(ticket) {
                Some(redirect::Cached::Stored { channel }) => {
                    tracing::info!(
                        "NWYD: 0x52A admitted for channel {channel}; waiting for the transfer socket to close"
                    );
                }
                Some(redirect::Cached::WrongClient { expected, got }) => {
                    tracing::info!(
                        "NWYD: 0x52A named client {got}, not the local client {expected}; ignored"
                    );
                }
                None => tracing::info!("NWYD: 0x52A arrived with no field scene up"),
            }

            PacketFlow::Continue
        }
        Packet::Disconnected {
            reason,
            socket_closed,
        } => {
            host.state().scene.discard_spatial_outbox();

            tracing::info!("NWYD: connection lost — {reason}");

            let now = host.game_time();

            let follow = host
                .state()
                .scene
                .redirect_after_disconnect(socket_closed, now);

            if let Some(follow) = follow {
                follow_redirect(host, follow);
            } else {
                let now = host.game_time();

                {
                    let state = host.state();

                    state
                        .scene
                        .connection_lost_message(state.launcher_login, now);
                }

                host.sync_ime();

                host.disconnect();

                let state = host.state();

                if !state.launcher_login
                    && !matches!(state.scene, Scene::ServerSelect(_) | Scene::LoginPanels(_))
                {
                    show_server_select(host);
                }
            }

            PacketFlow::RetireSocket
        }
        Packet::Kickout(packet) => {
            apply_kickout(host, packet);

            PacketFlow::Continue
        }
        Packet::Quit => {
            tracing::info!("NWYD: the server asked the client to close");

            PacketFlow::Exit
        }
    }
}

#[cfg(test)]
mod tests;
