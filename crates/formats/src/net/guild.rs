use crate::error::{Error, Kind as ErrorKind, Result};
pub use crate::net::ground_item::HEADER_LEN;
use crate::util::{u16_at, u32_at};

const KIND: ErrorKind = ErrorKind::Guild;

pub const OPCODE: u16 = 0x3E1C;

pub const MIN_LEN: usize = 0x16;

pub const FRAME_LEN: usize = 0x38;

pub const FRAME_LEN_IS_INFERRED: bool = true;

pub mod at {

    pub const COMMAND: usize = 0x00;

    pub const GUILD: usize = 0x04;

    pub const CHARIDX: usize = 0x10;

    pub const RANK: usize = 0x14;
}

pub const REFRESH_COMMANDS: [i32; 3] = [1, 3, 5];

pub const CLOSE_COMMAND: i32 = 2;

pub const NO_EFFECT_COMMAND: i32 = 0;

pub const PANEL_CONTENT: u32 = 0x2F;

pub const SOUND_SLOT: i32 = 0x25;

pub const EFFECT_KIND: i32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    None,
    Refresh,
    Close,
    Other(i32),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Membership {
    pub command: i32,
    pub guild: u16,
    pub charidx: u32,
    pub rank: u16,
}

impl Membership {
    pub fn command(self) -> Command {
        match self.command {
            NO_EFFECT_COMMAND => Command::None,
            n if REFRESH_COMMANDS.contains(&n) => Command::Refresh,
            CLOSE_COMMAND => Command::Close,
            n => Command::Other(n),
        }
    }

    pub fn raises_effect(self) -> bool {
        self.command != NO_EFFECT_COMMAND
    }

    pub fn pings_the_web_service(self) -> bool {
        self.guild != 0
    }

    pub fn signed_rank(self) -> i32 {
        i32::from(self.rank as i16)
    }

    pub fn rank_low_byte(self) -> u8 {
        self.rank as u8
    }

    pub fn to_bytes(self) -> [u8; MIN_LEN] {
        let mut out = [0u8; MIN_LEN];

        out[at::COMMAND..at::COMMAND + 4].copy_from_slice(&self.command.to_le_bytes());

        out[at::GUILD..at::GUILD + 2].copy_from_slice(&self.guild.to_le_bytes());

        out[at::CHARIDX..at::CHARIDX + 4].copy_from_slice(&self.charidx.to_le_bytes());

        out[at::RANK..at::RANK + 2].copy_from_slice(&self.rank.to_le_bytes());

        out
    }
}

pub fn parse(payload: &[u8]) -> Result<Membership> {
    if payload.len() < MIN_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: at::RANK,
            needed: MIN_LEN - at::RANK,
            len: payload.len().saturating_sub(at::RANK),
        });
    }

    Ok(Membership {
        command: u32_at(payload, at::COMMAND, KIND)? as i32,
        guild: u16_at(payload, at::GUILD, KIND)?,
        charidx: u32_at(payload, at::CHARIDX, KIND)?,
        rank: u16_at(payload, at::RANK, KIND)?,
    })
}
