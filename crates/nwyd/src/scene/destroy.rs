use crate::scene::enter::Entered;
use crate::scene::item_move::{self, Request};
use crate::scene::msgbox;
use formats::data::ItemList;
use formats::net::account::Item;
use ui::Strings;

pub const KIND: i32 = 0x17;

pub const CONTEXT: u32 = 0x17;

pub const CONFIRM_STRDEF: usize = super::select_char::DELETE_CONFIRM_STRDEF;

pub const CAPTION_STRDEF: usize = msgbox::DEFAULT_CAPTION_STRDEF;

pub const LINE_BREAK: &str = ui::text::FF_BREAK;

pub const MAX_ITEM_ID: u16 = 0x38A3;

pub const MAX_SLOT: i32 = 0x3C;

pub const CARRIED_GROUP: i32 = ui::widget::cell_group::INVENTORY;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Prompt {
    pub item_id: u16,
    pub slot: i32,
    pub snapshot: Item,
}

pub fn prompt(entered: &Entered, group: i32, item_id: u16, slot: i32) -> Option<Prompt> {
    if group != CARRIED_GROUP {
        return None;
    }

    if item_id == 0 || item_id > MAX_ITEM_ID {
        return None;
    }

    if !(0..MAX_SLOT).contains(&slot) {
        return None;
    }

    let snapshot = *entered.carry.get(usize::try_from(slot).ok()?)?;

    Some(Prompt {
        item_id,
        slot,
        snapshot,
    })
}

pub fn message(items: &ItemList, strings: &Strings, prompt: &Prompt) -> String {
    let name = items.name(usize::from(prompt.item_id)).unwrap_or_default();

    format!(
        "{name}{LINE_BREAK}{}",
        strings.translate(&format!("ss_{CONFIRM_STRDEF}"))
    )
}

pub fn request(items: &ItemList, strings: &Strings, prompt: &Prompt) -> msgbox::Request {
    msgbox::Request {
        caption: Some(CAPTION_STRDEF),
        kind: KIND,
        context: CONTEXT,
        body: msgbox::Body::Text(message(items, strings, prompt)),
        flags: msgbox::BUTTONS_YES_NO,
    }
}

pub fn confirm(entered: &mut Entered, prompt: &Prompt) -> Option<Request> {
    if prompt.item_id == 0 || prompt.item_id > MAX_ITEM_ID || !(0..MAX_SLOT).contains(&prompt.slot)
    {
        return None;
    }

    let index = usize::try_from(prompt.slot).ok()?;

    let live = *entered.carry.get(index)?;

    if live.index != prompt.item_id {
        return None;
    }

    if live != prompt.snapshot {
        return None;
    }

    entered.carry[index] = Item::default();

    Some(Request::Delete {
        slot: prompt.slot,
        item_id: prompt.item_id,
    })
}

pub const OPCODE: u16 = item_move::OPCODE_DELETE;
