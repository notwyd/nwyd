use formats::net::account::{self, Block};

pub const CONFIRM_STRDEF: usize = super::select_char::DELETE_CONFIRM_STRDEF;

pub const CONTEXT: u32 = super::msgbox::CONTEXT_DELETE;

pub const DELETE_OPCODE: u16 = 0x211;

pub const DELETE_LEN: usize = 0x30;

pub const OPCODE_OFF: usize = 0x04;

pub const SLOT_OFF: usize = 0x0C;

pub const NAME_OFF: usize = 0x10;

pub const PIN_OFF: usize = 0x20;

pub fn packet(slot: u8, block: Option<&Block>, pin: &[u8]) -> [u8; DELETE_LEN] {
    let mut out = [0u8; DELETE_LEN];

    out[OPCODE_OFF..OPCODE_OFF + 2].copy_from_slice(&DELETE_OPCODE.to_le_bytes());

    out[SLOT_OFF..SLOT_OFF + 4].copy_from_slice(&i32::from(slot as i8).to_le_bytes());

    let name = block
        .and_then(|b| b.slots.get(usize::from(slot)))
        .map(|s| s.name)
        .unwrap_or([0u8; account::NAME_LEN]);

    let name = &name[..name.iter().position(|b| *b == 0).unwrap_or(name.len())];

    let n = name.len().min(account::NAME_LEN - 1);

    out[NAME_OFF..NAME_OFF + n].copy_from_slice(&name[..n]);

    let n = pin.len().min(account::NAME_LEN - 1);

    out[PIN_OFF..PIN_OFF + n].copy_from_slice(&pin[..n]);

    out
}

pub fn may_confirm(selected: Option<usize>) -> bool {
    selected.is_some_and(|slot| slot < account::SLOTS)
}
