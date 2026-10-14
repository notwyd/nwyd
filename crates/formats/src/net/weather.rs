use crate::error::{Error, Kind as ErrorKind, Result};
use crate::net::ground_item::HEADER_LEN;
use crate::util::u32_at;

const KIND: ErrorKind = ErrorKind::Weather;

pub const SET: u16 = 0x18B;

pub const FRAME_LEN: usize = 0x10;

pub const LEN: usize = FRAME_LEN - HEADER_LEN;

pub mod at {

    pub const CODE: usize = 0x00;
}

pub const CLEAR: u32 = 0;

pub const RAIN: u32 = 1;

pub const SNOW: u32 = 2;

pub const HEAVY_SNOW: u32 = 3;

pub const ARMS: [u32; 3] = [RAIN, SNOW, HEAVY_SNOW];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Forecast {
    pub code: u32,
}

impl Forecast {
    pub fn has_precipitation_effect(self) -> bool {
        ARMS.contains(&self.code)
    }

    pub fn stops_the_cycle(self) -> bool {
        self.code != CLEAR
    }

    pub fn to_bytes(self) -> [u8; LEN] {
        self.code.to_le_bytes()
    }
}

pub fn parse(payload: &[u8]) -> Result<Forecast> {
    if payload.len() < LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: at::CODE,
            needed: LEN,
            len: payload.len(),
        });
    }

    Ok(Forecast {
        code: u32_at(payload, at::CODE, KIND)?,
    })
}
