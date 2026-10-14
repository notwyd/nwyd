use crate::scene::rng::Lcg;
use formats::net::account::{self, Block};

pub const OPCODE: u16 = 0x213;

pub const LEN: usize = 0x24;

pub const OPCODE_OFF: usize = 0x04;

pub const SLOT_OFF: usize = 0x0C;

pub const TERM_OFF: usize = 0x14;

pub const OBFUSCATION_BASE: i32 = 64_396;

pub const TERM_MOD: u32 = 9;

pub const TERM_SCALE: i32 = 10;

pub const DISABLE_MS: u32 = 3_000;

pub const CREATE_DISABLE_MS: u32 = 2_000;

pub fn term(rng: &mut Lcg) -> i32 {
    let t = rng.next_mod(TERM_MOD) as i32 + 1;

    t + (OBFUSCATION_BASE << t) * TERM_SCALE
}

pub fn packet(slot: u8, rng: &mut Lcg) -> [u8; LEN] {
    let mut out = [0u8; LEN];

    out[OPCODE_OFF..OPCODE_OFF + 2].copy_from_slice(&OPCODE.to_le_bytes());

    out[SLOT_OFF..SLOT_OFF + 4].copy_from_slice(&i32::from(slot as i8).to_le_bytes());

    out[TERM_OFF..TERM_OFF + 4].copy_from_slice(&term(rng).to_le_bytes());

    out
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Pressed {
    Send(Box<[u8; LEN]>),
    NoSlot,
    Empty,
}

pub fn pressed(
    selected: Option<usize>,
    has_doll: bool,
    block: Option<&Block>,
    rng: &mut Lcg,
) -> Pressed {
    let Some(slot) = selected.filter(|s| *s < account::SLOTS) else {
        return Pressed::NoSlot;
    };

    let named = block
        .and_then(|b| b.slots.get(slot))
        .is_some_and(|s| s.name.first().is_some_and(|b| *b != 0));

    if !has_doll || !named {
        return Pressed::Empty;
    }

    Pressed::Send(Box::new(packet(slot as u8, rng)))
}
