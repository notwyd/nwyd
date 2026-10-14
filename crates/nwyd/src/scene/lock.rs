use super::delete;
use formats::net::account::Block;
use ui::input::Mouse;

pub const LOCKED: u8 = 0;

pub const UNLOCKED: u8 = 1;

pub const AWAITING: u8 = 2;

pub const CHANGE_SENT: u8 = 3;

pub const CHANGE_NEW: u8 = 4;

pub const CHANGE_CONFIRM: u8 = 5;

pub const CONFIRM_DELETE: u8 = 0xFF;

pub const OPCODE: u16 = 0xFDE;

pub const LEN: usize = 0x20;

pub const RESET_OPCODE: u16 = 0xFDF;

pub const OPCODE_OFF: usize = 0x04;

pub const PIN_OFF: usize = 0x0C;

pub const PIN_LEN: usize = 12;

pub const MASKED_OFF: usize = 0x1A;

pub const FLAG_OFF: usize = 0x1C;

pub const CONFIRM_FLAG: u8 = 1;

pub const PIN_MIN: usize = 4;

pub const REOPEN_MS: u32 = 500;

pub fn packet(pin: &[u8], confirm: bool) -> [u8; LEN] {
    let mut out = [0u8; LEN];

    out[OPCODE_OFF..OPCODE_OFF + 2].copy_from_slice(&OPCODE.to_le_bytes());

    let n = pin.len().min(PIN_LEN);

    out[PIN_OFF..PIN_OFF + n].copy_from_slice(&pin[..n]);

    if confirm {
        out[FLAG_OFF] = CONFIRM_FLAG;
    }

    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ok {
    Unlock(Box<[u8; LEN]>),
    Delete(Box<[u8; delete::DELETE_LEN]>),
    TooShort,
    Ignored,
}

pub fn ok(state: u8, selected: Option<usize>, block: Option<&Block>, pin: &[u8]) -> Ok {
    match state {
        LOCKED => {
            if pin.len() < PIN_MIN {
                return Ok::TooShort;
            }

            Ok::Unlock(Box::new(packet(pin, false)))
        }
        CONFIRM_DELETE => {
            if pin.len() < PIN_MIN {
                return Ok::TooShort;
            }

            let slot = selected.map_or(0xFF, |s| s as u8);

            Ok::Delete(Box::new(delete::packet(slot, block, pin)))
        }
        _ => Ok::Ignored,
    }
}

impl Ok {
    pub fn sends(&self) -> bool {
        matches!(self, Ok::Unlock(_) | Ok::Delete(_))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Submit(Box<[u8; LEN]>),
    Saved,
    Confirm(Box<[u8; LEN]>),
    Mismatch,
    TooShort,
    Ignored,
}

pub fn change(state: u8, typed: &[u8], saved: &[u8]) -> Change {
    if !matches!(state, LOCKED | CHANGE_NEW | CHANGE_CONFIRM) {
        return Change::Ignored;
    }

    if typed.len() < PIN_MIN {
        return Change::TooShort;
    }

    match state {
        LOCKED => Change::Submit(Box::new(packet(typed, false))),
        CHANGE_NEW => Change::Saved,
        _ if saved == typed => Change::Confirm(Box::new(packet(typed, true))),
        _ => Change::Mismatch,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply {
    Unlocked,
    ChangeAccepted,
    Reset,
    Ignored,
}

pub fn reply(opcode: u16, state: u8) -> Reply {
    match (opcode, state) {
        (RESET_OPCODE, _) => Reply::Reset,
        (OPCODE, AWAITING) => Reply::Unlocked,
        (OPCODE, CHANGE_SENT) => Reply::ChangeAccepted,
        _ => Reply::Ignored,
    }
}

pub fn may_reopen(
    state: u8,
    pane_visible: bool,
    event: Mouse,
    now_ms: u32,
    last_send_ms: u32,
) -> bool {
    state == LOCKED
        && !pane_visible
        && event == Mouse::LeftDown
        && last_send_ms.wrapping_add(REOPEN_MS) <= now_ms
}
