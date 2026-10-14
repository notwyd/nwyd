use crate::error::{Error, Kind, Result};

pub const MIN_PAYLOAD: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NpcLine {
    pub kind: u8,
    pub index: i16,
}

impl NpcLine {
    pub const CHAT: u8 = 1;
}

pub fn parse(payload: &[u8]) -> Result<NpcLine> {
    if payload.len() < MIN_PAYLOAD {
        return Err(Error::Truncated {
            kind: Kind::NpcLine,
            offset: 0,
            needed: MIN_PAYLOAD,
            len: payload.len(),
        });
    }

    Ok(NpcLine {
        kind: payload[0],
        index: i16::from_le_bytes([payload[2], payload[3]]),
    })
}

pub const BALLOON_LONG_PREFIX: u8 = b'*';

pub const BALLOON_LONG_MS: u32 = 10_000;

pub const BALLOON_SHORT_MS: u32 = 3_000;

pub const BALLOON_SUPPRESS: [u8; 3] = *b"=-@";

pub const NOTICE_LIFE_MS: u32 = 4_000;

pub const NOTICE_SOUND: bool = true;
