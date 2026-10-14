use crate::frame::PanelId;
use crate::scene::chat;
use crate::scene::panels::{Action, Panels};
use formats::ui::pane::ElementType;

pub const CONTENT: PanelId = 0x23;

pub const SCRIPT: &[u8] = b"ui/Panel/game/RpsGame.pane";

pub const TARGET: &str = "_RPS_";

pub const WORDS: [(&str, &str); 3] = [("R", "rock"), ("P", "paper"), ("S", "scissor")];

pub const CLOSE_BUTTON: &str = "Close";

pub use formats::net::arena::{ACCEPT, ACCEPT_BOX, ACCEPT_LEN, ACCEPT_REPLY};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Played {
    Threw(&'static str),
    Closed,
    Ignored,
}

pub fn command(panels: &mut Panels, client_id: u16, name: &str) -> Vec<Action> {
    let played = play(panels, client_id, name);

    match played {
        Played::Threw(word) => tracing::info!("NWYD: rps {word} -> {TARGET}"),
        Played::Closed => return vec![Action::Toggle(CONTENT)],
        Played::Ignored => return Vec::new(),
    }

    if panels.is_open(CONTENT) {
        vec![Action::Toggle(CONTENT)]
    } else {
        Vec::new()
    }
}

pub fn play(panels: &mut Panels, client_id: u16, name: &str) -> Played {
    if name == CLOSE_BUTTON {
        return Played::Closed;
    }

    match WORDS.iter().find(|(button, _)| name == *button) {
        Some(&(_, word)) => {
            panels
                .wire
                .push(chat::channel_packet(client_id, chat::WHISPER, TARGET, word));

            Played::Threw(word)
        }
        None => Played::Ignored,
    }
}

pub fn accept_frame(reply: u32) -> Vec<u8> {
    formats::net::arena::accept_frame(reply).to_vec()
}

pub fn elements_present(panels: &Panels) -> bool {
    let Some(screen) = panels.screen(CONTENT) else {
        return false;
    };

    WORDS
        .iter()
        .all(|(name, _)| screen.element(name, ElementType::Button).is_some())
}
