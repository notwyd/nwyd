use crate::error::{Error, Kind, Result};

pub const MIN_PAYLOAD: usize = 8;

pub const RATIOS: [&str; 4] = ["1 : 1", "5 : 5", "10 : 10", "All : All"];

pub const RATIO_ADDRESSES: [u32; 4] = [0x0089_1B5C, 0x0089_1B54, 0x0089_1B4C, 0x0089_1B40];

pub const STAKES: [i32; 4] = [0, 1, 2, 3];

pub const ACCEPT: u32 = 4;

pub const BODY_STRDEF: usize = 153;

pub const TAIL_STRDEF: usize = 154;

pub const CAPTION_STRDEF: usize = 237;

pub const CONTEXT: u32 = 0x2B;

pub const KIND: i32 = 0xD;

pub const FLAGS: u32 = 0x2000_0001;

pub const AUTO_DISMISS_S: f32 = 0.0;

pub const REPLY_LEN: usize = 0x14;

pub const FORMAT_BOUND: usize = 0x80;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Challenge {
    pub subject: u32,
    pub stake: i32,
}

impl Challenge {
    pub fn ratio(&self) -> Option<&'static str> {
        let index = self.stake % RATIOS.len() as i32;

        usize::try_from(index).ok().map(|k| RATIOS[k])
    }
}

pub fn parse(payload: &[u8]) -> Result<Challenge> {
    if payload.len() < MIN_PAYLOAD {
        return Err(Error::Truncated {
            kind: Kind::Challenge,
            offset: 0,
            needed: MIN_PAYLOAD,
            len: payload.len(),
        });
    }

    Ok(Challenge {
        subject: u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]),
        stake: i32::from_le_bytes([payload[4], payload[5], payload[6], payload[7]]),
    })
}

pub fn reply_payload(subject: u32) -> [u8; 8] {
    let mut out = [0u8; 8];

    out[0..4].copy_from_slice(&subject.to_le_bytes());

    out[4..8].copy_from_slice(&ACCEPT.to_le_bytes());

    out
}
