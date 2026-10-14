use crate::error::{Error, Kind as ErrorKind, Result};
use crate::util::{u16_at, u32_at};

const KIND: ErrorKind = ErrorKind::Siege;

pub const STATE: u16 = 0x3A8;

pub const WAR: u16 = 0x3AC;

pub const STATE_FRAME_A: u16 = 0x10;

pub const STATE_FRAME_B: u16 = 0x14;

pub const STATE_FRAME_C: u16 = 0x18;

pub const STATE_FRAMES: [u16; 3] = [STATE_FRAME_A, STATE_FRAME_B, STATE_FRAME_C];

pub const WAR_FRAME_LEN: usize = 0x10;

pub const WAR_LEN: usize = 4;

pub mod at {

    pub const GUILD: usize = 0x00;

    pub const KINGDOM: usize = 0x04;

    pub const ALLY: usize = 0x08;
}

pub const NO_GUILD: u16 = 0xFFFF;

pub const KINGDOM_UNSET: u8 = 0xFF;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Siege {
    Set(State),
    Ignored { size: u16 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub size: u16,
    pub war_guild: u16,
    pub kingdom: Option<u8>,
    pub ally: Option<u16>,
}

impl State {
    pub fn has_zero_guild_id(self) -> bool {
        self.war_guild == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarFlag {
    pub flag: u32,
}

impl WarFlag {
    pub fn raised(self) -> bool {
        self.flag != 0
    }

    pub fn clears_field_music(self) -> bool {
        self.flag == 0
    }

    pub fn clears_arena_music(self) -> bool {
        let _ = self;

        true
    }

    pub fn to_bytes(self) -> [u8; WAR_LEN] {
        self.flag.to_le_bytes()
    }
}

pub const MUSIC_SLOT_INVALID: i32 = -1;

pub fn parse(size: u16, payload: &[u8]) -> Result<Siege> {
    if !STATE_FRAMES.contains(&size) {
        return Ok(Siege::Ignored { size });
    }

    let wide = u32_at(payload, at::GUILD, KIND)?;

    let war_guild = match wide {
        0 => NO_GUILD,
        _ => (wide & 0xFFFF) as u16,
    };

    let kingdom = match size {
        STATE_FRAME_A => None,
        _ => Some(*payload.get(at::KINGDOM).ok_or(Error::Truncated {
            kind: KIND,
            offset: at::KINGDOM,
            needed: 1,
            len: payload.len(),
        })?),
    };

    let ally = match size {
        STATE_FRAME_C => Some(u16_at(payload, at::ALLY, KIND)?),
        _ => None,
    };

    Ok(Siege::Set(State {
        size,
        war_guild,
        kingdom,
        ally,
    }))
}

pub fn parse_war(payload: &[u8]) -> Result<WarFlag> {
    Ok(WarFlag {
        flag: u32_at(payload, at::GUILD, KIND)?,
    })
}
