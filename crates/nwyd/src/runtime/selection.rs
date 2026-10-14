use super::lifecycle::Retained;
use crate::{
    clock::GameTime,
    net::Session,
    scene::{Scene, char_listener, lock, rng::Lcg},
};
use assets::GameDir;
use formats::{data::item_list::ItemList, net::account};

pub enum Packet {
    Notice { opcode: u16 },
    Metadata(account::SelectionMetadata),
    DeleteHold(account::DeleteHold),
    NameNotice,
    NameResult(account::NameResult),
    AccountRefresh(Box<account::Block>),
    SlotEffect { slot: u32 },
    AccountLock { opcode: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Presentation {
    Unchanged,
    Redraw,
}

pub struct Resources<'a> {
    pub game: &'a GameDir,
    pub items: &'a ItemList,
}

pub struct State<'a> {
    pub scene: &'a mut Scene,
    pub session: &'a mut Session,
    pub retained: &'a mut Retained,
    pub rng: &'a mut Lcg,
    pub resources: Option<Resources<'a>>,
    pub now_ms: GameTime,
}

pub fn apply(state: State<'_>, packet: Packet) -> Presentation {
    let packet = match packet {
        Packet::AccountRefresh(block) => {
            state.session.replace_block(block);

            if let Scene::Field(field) = &mut *state.scene
                && let Some(slot) = state
                    .session
                    .block
                    .as_ref()
                    .and_then(|block| block.slots.get(field.entered.slot))
            {
                field.entered.account_body = slot.equipment.body().index;
            }

            let (Scene::SelectChar(scene), Some(resources)) = (state.scene, state.resources) else {
                return Presentation::Unchanged;
            };

            if let Some(block) = state.session.block.as_deref() {
                if let Err(error) = scene.refresh_account(
                    resources.game,
                    block,
                    resources.items,
                    state.now_ms.as_millis(),
                ) {
                    tracing::info!("NWYD: could not rebuild the character list: {error:#}");
                }
            }

            return Presentation::Redraw;
        }
        other => other,
    };

    let Scene::SelectChar(scene) = state.scene else {
        return Presentation::Unchanged;
    };

    match packet {
        Packet::Notice { opcode } => {
            scene.packet_notice(opcode, state.now_ms.as_millis());

            Presentation::Redraw
        }
        Packet::Metadata(value) => {
            state.session.apply_selection_metadata(value);

            Presentation::Unchanged
        }
        Packet::DeleteHold(hold) => {
            scene.delete_hold(hold, state.now_ms.as_millis());

            Presentation::Redraw
        }
        Packet::NameNotice => {
            state.retained.name_prompt_required = true;

            scene.name_dialog.prompt_required = true;

            Presentation::Unchanged
        }
        Packet::NameResult(reply) => {
            if !scene.name_result(reply, state.now_ms.as_millis()) {
                return Presentation::Unchanged;
            }

            if reply.result == 0
                && let Some(block) = scene.account
            {
                state.session.replace_block(Box::new(block));
            }

            Presentation::Redraw
        }
        Packet::SlotEffect { slot } => {
            scene.slot_effect(slot, state.now_ms.as_millis());

            Presentation::Redraw
        }
        Packet::AccountLock { opcode } => {
            let reply = char_listener::received(scene, opcode, state.rng);

            state.retained.account_lock = scene.lock;

            tracing::info!("NWYD: {opcode:#05x} account-lock reply — {reply:?}");

            if reply == lock::Reply::Ignored {
                Presentation::Unchanged
            } else {
                Presentation::Redraw
            }
        }
        Packet::AccountRefresh(_) => Presentation::Unchanged,
    }
}
