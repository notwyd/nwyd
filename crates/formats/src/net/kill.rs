use crate::error::{Kind, Result};
use crate::util::{u16_at, u32_at};

const KIND: Kind = Kind::Kill;

pub const KILL: u16 = 0x338;

pub const KILL_FRAME_LEN: usize = 0x20;

pub const HEADER_LEN: usize = 0x0C;

pub const KILL_PAYLOAD_LEN: usize = KILL_FRAME_LEN - HEADER_LEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Kill {
    pub cp: i32,
    pub victim: u16,
    pub killer: u16,
    pub exp: u64,
}

impl Kill {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; KILL_PAYLOAD_LEN];

        out[0x00..0x04].copy_from_slice(&self.cp.to_le_bytes());

        out[0x04..0x06].copy_from_slice(&self.victim.to_le_bytes());

        out[0x06..0x08].copy_from_slice(&self.killer.to_le_bytes());

        out[0x0C..0x14].copy_from_slice(&self.exp.to_le_bytes());

        out
    }
}

pub fn parse(payload: &[u8]) -> Result<Kill> {
    Ok(Kill {
        cp: u32_at(payload, 0x00, KIND)? as i32,
        victim: u16_at(payload, 0x04, KIND)?,
        killer: u16_at(payload, 0x06, KIND)?,
        exp: u64::from(u32_at(payload, 0x0C, KIND)?)
            | u64::from(u32_at(payload, 0x10, KIND)?) << 32,
    })
}
