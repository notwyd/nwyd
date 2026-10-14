use crate::error::{Error, Kind, Result};
use crate::net::ground_item::HEADER_LEN;

const KIND: Kind = Kind::Emote;

pub const OPCODE: u16 = 0x36A;

pub const FRAME_LEN: usize = 0x14;

pub const LEN: usize = FRAME_LEN - HEADER_LEN;

pub mod at {

    pub const CODE: usize = 0x00;

    pub const MODE: usize = 0x02;

    pub const UNREAD: usize = 0x04;
}

pub const FIREWORK: i16 = 100;

pub const CODE_LIMIT: i16 = 0x100;

pub const MODE_APPEARANCE: i16 = 1;

pub const MODE_UNDIE: i16 = 2;

pub const MODE_LEVEL_UP: i16 = 3;

pub const MODE_PLAIN: i16 = 0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Emote {
    pub code: i16,
    pub mode: i16,
    pub unread: f32,
}

impl Emote {
    pub fn outbound(code: i16) -> Emote {
        Emote {
            code,
            mode: MODE_PLAIN,
            unread: 0.0,
        }
    }

    pub fn to_bytes(&self) -> [u8; LEN] {
        let mut out = [0u8; LEN];

        out[at::CODE..at::CODE + 2].copy_from_slice(&self.code.to_le_bytes());

        out[at::MODE..at::MODE + 2].copy_from_slice(&self.mode.to_le_bytes());

        out[at::UNREAD..at::UNREAD + 4].copy_from_slice(&self.unread.to_le_bytes());

        out
    }
}

pub fn parse(payload: &[u8]) -> Result<Emote> {
    if payload.len() < LEN {
        return Err(Error::structure(
            KIND,
            format!("expected at least {LEN} bytes, found {}", payload.len()),
        ));
    }

    Ok(Emote {
        code: i16::from_le_bytes([payload[at::CODE], payload[at::CODE + 1]]),
        mode: i16::from_le_bytes([payload[at::MODE], payload[at::MODE + 1]]),
        unread: f32::from_le_bytes([
            payload[at::UNREAD],
            payload[at::UNREAD + 1],
            payload[at::UNREAD + 2],
            payload[at::UNREAD + 3],
        ]),
    })
}
