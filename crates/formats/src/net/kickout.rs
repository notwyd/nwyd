use crate::error::{Kind as ErrorKind, Result};
use crate::util::u16_at;

const KIND: ErrorKind = ErrorKind::Kickout;

pub const OPCODE: u16 = 0x292;

pub const DECLARED_FRAME_LEN: u16 = 0x10;

pub const FRAME_LEN: usize = 0x12;

pub const LEN: usize = FRAME_LEN - crate::net::ground_item::HEADER_LEN;

pub mod at {

    pub const CODE: usize = 0x04;
}

pub const DECADE: i16 = 10;

pub const CHARACTERS_QUOTIENT: i16 = 1;

pub const CHARACTERS: std::ops::RangeInclusive<i16> = 10..=19;

pub const DEATH_CODE: i16 = 0x16;

pub const DEATH_LEVEL_CAP: i16 = 1000;

pub const MESSAGE_STRDEF: usize = 13;

pub const MESSAGE_LIFE_MS: u32 = 2000;

pub const MESSAGE_SOUND: i64 = 0x21;

pub const SCENE_SELECT_CHAR: u32 = 5;

pub const SCENE_SELECT_SERVER: u32 = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kickout {
    Characters { code: i16 },
    Death { code: i16 },
    Disconnected { code: i16 },
}

impl Kickout {
    pub fn code(self) -> i16 {
        match self {
            Kickout::Characters { code } | Kickout::Death { code } => code,
            Kickout::Disconnected { code } => code,
        }
    }

    pub fn characters_code(code: i16) -> bool {
        code / DECADE == CHARACTERS_QUOTIENT
    }

    pub fn scene(self) -> Option<u32> {
        match self {
            Kickout::Characters { .. } => Some(SCENE_SELECT_CHAR),
            Kickout::Death { .. } => None,
            Kickout::Disconnected { .. } => Some(SCENE_SELECT_SERVER),
        }
    }
}

pub fn parse(payload: &[u8]) -> Result<Kickout> {
    let code = u16_at(payload, at::CODE, KIND)? as i16;

    Ok(classify_code(code))
}

pub fn classify_code(code: i16) -> Kickout {
    if Kickout::characters_code(code) {
        Kickout::Characters { code }
    } else if code == DEATH_CODE {
        Kickout::Death { code }
    } else {
        Kickout::Disconnected { code }
    }
}

pub fn to_bytes(code: i16) -> [u8; LEN] {
    let mut out = [0u8; LEN];

    out[at::CODE..at::CODE + 2].copy_from_slice(&code.to_le_bytes());

    out
}
