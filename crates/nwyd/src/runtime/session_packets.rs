use crate::{
    net::Session,
    scene::{Scene, cc},
};
use formats::net::dynamic_key::Update;

pub enum Packet {
    QuestFlags([u8; 11]),
    ClientConfig(formats::net::client_config::Update),
    DynamicKey(Update),
    ServerReturnNotice,
    LoginDenied { opcode: u16 },
}

pub struct State<'a> {
    pub scene: &'a mut Scene,
    pub session: &'a mut Session,
}

pub trait Host {
    fn state(&mut self) -> State<'_>;

    fn sync_ime(&mut self);

    fn redraw(&mut self);
}

pub fn apply(host: &mut impl Host, packet: Packet) {
    let state = host.state();

    let presented = match packet {
        Packet::QuestFlags(flags) => {
            if let Scene::Field(field) = state.scene {
                field.entered.quest_flags[1..12].copy_from_slice(&flags);

                if let Some(panels) = &mut field.panels {
                    panels.quest_response_pending = true;
                }
            }

            false
        }
        Packet::ClientConfig(update) => {
            std::sync::Arc::make_mut(&mut state.session.client_config).apply(&update);

            if let Scene::Field(field) = state.scene
                && let Some(panels) = field.panels.as_mut()
            {
                panels.client_config = state.session.client_config.clone();
            }

            false
        }
        Packet::DynamicKey(update) => {
            let tier = match state.scene {
                Scene::Field(field) => cc::evolution_tier(field),
                _ => state.session.evolution_tier(),
            };

            state.session.dynamic_key(update, tier);

            false
        }
        Packet::ServerReturnNotice => {
            state.session.retain_server_return_notice();

            state.scene.server_return_notice(false)
        }
        Packet::LoginDenied { opcode } => state.scene.login_denied(opcode),
    };

    if presented {
        host.sync_ime();

        host.redraw();
    }
}
