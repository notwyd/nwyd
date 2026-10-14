use crate::error::{Error, Kind as ErrorKind, Result};
use crate::net::ground_item::HEADER_LEN;
use crate::util::u32_at;

const KIND: ErrorKind = ErrorKind::EventHud;

pub const COUNTDOWN: u16 = 0x3A1;

pub const REMAINING: u16 = 0x3B0;

pub const PACKED: u16 = 0x3BB;

pub const TARGET: u16 = 0x3CF;

pub const SCORE: u16 = 0x3D0;

pub const FAMILY: [u16; 5] = [COUNTDOWN, REMAINING, PACKED, TARGET, SCORE];

pub const COUNTER_FAMILY: [u16; 4] = [REMAINING, PACKED, TARGET, SCORE];

pub const FRAME_LEN: usize = 0x10;

pub const LEN: usize = FRAME_LEN - HEADER_LEN;

pub const SCORE_FRAME_LEN: usize = 0x14;

pub const SCORE_LEN: usize = SCORE_FRAME_LEN - HEADER_LEN;

pub mod at {

    pub const FIRST: usize = 0x00;

    pub const SECOND: usize = 0x04;
}

pub const COUNTDOWN_FORMAT: &str = "%d";

pub const REMAINING_FORMAT: &str = "%d Left";

pub const SCORE_FORMAT: &str = "%d / %d";

pub const TARGET_FORMAT: &str = "0 / %d";

pub const PACKED_LOW_MASK: u32 = 0xFF;

pub const PACKED_HIGH_SHIFT: u32 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Countdown {
    pub seconds: i32,
}

impl Countdown {
    pub fn armed_text(stamp: u32) -> String {
        format!("{}", stamp as i32)
    }

    pub fn to_bytes(self) -> [u8; LEN] {
        self.seconds.to_le_bytes()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Remaining {
    pub count: i32,
}

impl Remaining {
    pub fn text(self) -> String {
        format!("{} Left", self.count)
    }

    pub fn to_bytes(self) -> [u8; LEN] {
        self.count.to_le_bytes()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Packed {
    pub word: u32,
}

impl Packed {
    pub fn low(self) -> i32 {
        (self.word & PACKED_LOW_MASK) as i32
    }

    pub fn high(self) -> i32 {
        (self.word as i32) >> PACKED_HIGH_SHIFT
    }

    pub fn unread_byte(self) -> u8 {
        ((self.word >> 8) & 0xFF) as u8
    }

    pub fn text(self) -> String {
        format!("{} / {}", self.low(), self.high())
    }

    pub fn to_bytes(self) -> [u8; LEN] {
        self.word.to_le_bytes()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Target {
    pub total: i32,
}

impl Target {
    pub fn shows(self) -> bool {
        self.total != 0
    }

    pub fn text(self) -> Option<String> {
        self.shows().then(|| format!("0 / {}", self.total))
    }

    pub fn to_bytes(self) -> [u8; LEN] {
        self.total.to_le_bytes()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Score {
    pub current: i32,
    pub total: i32,
}

impl Score {
    pub fn text(self) -> String {
        format!("{} / {}", self.current, self.total)
    }

    pub fn to_bytes(self) -> [u8; SCORE_LEN] {
        let mut out = [0u8; SCORE_LEN];

        out[at::FIRST..at::FIRST + 4].copy_from_slice(&self.current.to_le_bytes());

        out[at::SECOND..at::SECOND + 4].copy_from_slice(&self.total.to_le_bytes());

        out
    }
}

pub fn parse_countdown(payload: &[u8]) -> Result<Countdown> {
    Ok(Countdown {
        seconds: first(payload)? as i32,
    })
}

pub fn parse_remaining(payload: &[u8]) -> Result<Remaining> {
    Ok(Remaining {
        count: first(payload)? as i32,
    })
}

pub fn parse_packed(payload: &[u8]) -> Result<Packed> {
    Ok(Packed {
        word: first(payload)?,
    })
}

pub fn parse_target(payload: &[u8]) -> Result<Target> {
    Ok(Target {
        total: first(payload)? as i32,
    })
}

pub fn parse_score(payload: &[u8]) -> Result<Score> {
    if payload.len() < SCORE_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: at::SECOND,
            needed: SCORE_LEN,
            len: payload.len(),
        });
    }

    Ok(Score {
        current: u32_at(payload, at::FIRST, KIND)? as i32,
        total: u32_at(payload, at::SECOND, KIND)? as i32,
    })
}

fn first(payload: &[u8]) -> Result<u32> {
    if payload.len() < LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: at::FIRST,
            needed: LEN,
            len: payload.len(),
        });
    }

    u32_at(payload, at::FIRST, KIND)
}
