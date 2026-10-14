use crate::error::{Error, Kind, Result};
use crate::net::ground_item::HEADER_LEN;
use crate::util::{byte_at, u16_at, u32_at};

const KIND: Kind = Kind::GateCommand;

pub const OPCODE: u16 = 0x374;

pub const FRAME_LEN: usize = 0x14;

pub const LEN: usize = FRAME_LEN - HEADER_LEN;

pub const ASK_OPEN: i16 = 1;

pub const SWING_OFFSET: i32 = 3;

pub mod at {

    pub const ENTITY: usize = 0x00;

    pub const STATE: usize = 0x04;

    pub const FILL: usize = 0x06;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Command {
    pub entity: u32,
    pub state: i16,
    pub fill: i8,
}

impl Command {
    pub fn to_bytes(&self) -> [u8; LEN] {
        let mut out = [0u8; LEN];

        out[at::ENTITY..][..4].copy_from_slice(&self.entity.to_le_bytes());

        out[at::STATE..][..2].copy_from_slice(&self.state.to_le_bytes());

        out[at::FILL] = self.fill as u8;

        out
    }

    pub fn commanded(&self) -> i32 {
        i32::from(self.state) + SWING_OFFSET
    }
}

pub fn parse(payload: &[u8]) -> Result<Command> {
    if payload.len() < LEN {
        return Err(Error::structure(
            KIND,
            format!("expected at least {LEN} bytes, found {}", payload.len()),
        ));
    }

    Ok(Command {
        entity: u32_at(payload, at::ENTITY, KIND)?,
        state: u16_at(payload, at::STATE, KIND)? as i16,
        fill: byte_at(payload, at::FILL, KIND)? as i8,
    })
}
