use crate::error::{Error, Kind, Result};
use crate::net::spawn::GUILD_NAME_LEN;
use crate::text::{Codepage, codepage};
use crate::util::u16_at;

const KIND: Kind = Kind::Plate;

pub const LEVEL_FRAME_LEN: usize = 0x12;

pub const LEVEL_LEN: usize = 0x06;

pub const GUILD_FRAME_LEN: usize = 0x28;

pub const GUILD_LEN: usize = 0x1C;

pub const PLAYER_ID_LIMIT: u16 = 1000;

pub const MOB_NAME_LEVEL: u8 = 0x4B;

pub const GUILD_NAME_MAX: usize = GUILD_NAME_LEN - 1;

pub const GUILD_NAME_OFF: usize = 2;

pub const TERMINATOR_AT: usize = 0x1B;

pub const UNDERSCORE: u8 = b'_';

pub const UNDERSCORE_BECOMES: u8 = b' ';

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct LevelChange {
    pub entity: u16,
    pub name_level: u8,
    pub level: u16,
}

impl LevelChange {
    pub fn is_mob(&self) -> bool {
        self.entity >= PLAYER_ID_LIMIT
    }

    pub fn stored(&self) -> (u16, u8) {
        match self.is_mob() {
            true => (0, MOB_NAME_LEVEL),
            false => (self.level, self.name_level),
        }
    }

    pub fn to_bytes(&self) -> [u8; LEVEL_LEN] {
        let mut out = [0u8; LEVEL_LEN];

        out[0..2].copy_from_slice(&self.entity.to_le_bytes());

        out[2] = self.name_level;

        out[3] = 0;

        out[4..6].copy_from_slice(&self.level.to_le_bytes());

        out
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GuildLine {
    pub entity: u16,
    pub name: String,
}

pub fn set_text(text: &str) -> String {
    let swapped: String = text
        .chars()
        .map(|c| match c == UNDERSCORE as char {
            true => UNDERSCORE_BECOMES as char,
            false => c,
        })
        .collect();

    match swapped == " " {
        true => String::new(),
        false => swapped,
    }
}

impl GuildLine {
    pub fn displayed(&self) -> String {
        set_text(&self.name)
    }

    pub fn to_bytes(&self) -> [u8; GUILD_LEN] {
        let mut out = [0u8; GUILD_LEN];

        out[0..2].copy_from_slice(&self.entity.to_le_bytes());

        let encoded = codepage::encode(&self.name, Codepage::Windows1252);

        let n = encoded.len().min(GUILD_NAME_MAX);

        out[GUILD_NAME_OFF..GUILD_NAME_OFF + n].copy_from_slice(&encoded[..n]);

        out[TERMINATOR_AT] = 0;

        out
    }
}

pub fn parse_level(payload: &[u8]) -> Result<LevelChange> {
    if payload.len() < LEVEL_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: 0,
            needed: LEVEL_LEN,
            len: payload.len(),
        });
    }

    Ok(LevelChange {
        entity: u16_at(payload, 0, KIND)?,
        name_level: payload[2],
        level: u16_at(payload, 4, KIND)?,
    })
}

pub fn parse_guild(payload: &[u8]) -> Result<GuildLine> {
    if payload.len() < GUILD_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: 0,
            needed: GUILD_LEN,
            len: payload.len(),
        });
    }

    let bytes = &payload[GUILD_NAME_OFF..TERMINATOR_AT];

    let cut = bytes.iter().position(|b| *b == 0).unwrap_or(GUILD_NAME_MAX);

    Ok(GuildLine {
        entity: u16_at(payload, 0, KIND)?,
        name: codepage::decode(&bytes[..cut], Codepage::Windows1252),
    })
}
