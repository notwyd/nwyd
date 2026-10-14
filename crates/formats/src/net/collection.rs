use crate::error::{Error, Kind, Result};

pub const FLAGS_OFF: usize = 0x0C;

pub const FLAG_BYTES: usize = 0x30;

pub const PAYLOAD_LEN: usize = FLAGS_OFF + FLAG_BYTES;

pub const FRAME_LEN: usize = PAYLOAD_LEN + 0x0C;

pub const READ_INDICES: usize = 113;

pub const SLOT_BITS: usize = crate::data::collection::NEEDED;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Progress {
    pub flags: [u8; FLAG_BYTES],
}

impl Default for Progress {
    fn default() -> Self {
        Progress {
            flags: [0; FLAG_BYTES],
        }
    }
}

impl Progress {
    pub fn byte(&self, index: usize) -> Option<u8> {
        self.flags.get(index).copied()
    }

    pub fn complete(&self, index: usize) -> Option<bool> {
        self.byte(index).map(|b| b & 1 == 1)
    }

    pub fn slot(&self, index: usize, slot: usize) -> Option<bool> {
        if slot >= SLOT_BITS {
            return None;
        }

        self.byte(index).map(|b| b & (1 << (slot + 1)) != 0)
    }

    pub fn completed(&self) -> usize {
        self.flags.iter().filter(|b| *b & 1 == 1).count()
    }

    pub fn to_payload(&self) -> Vec<u8> {
        let mut out = vec![0u8; PAYLOAD_LEN];

        out[FLAGS_OFF..FLAGS_OFF + FLAG_BYTES].copy_from_slice(&self.flags);

        out
    }
}

pub fn parse(payload: &[u8]) -> Result<Progress> {
    if payload.len() < PAYLOAD_LEN {
        return Err(Error::Truncated {
            kind: Kind::Collection,
            offset: FLAGS_OFF,
            needed: PAYLOAD_LEN,
            len: payload.len(),
        });
    }

    let mut flags = [0u8; FLAG_BYTES];

    flags.copy_from_slice(&payload[FLAGS_OFF..FLAGS_OFF + FLAG_BYTES]);

    Ok(Progress { flags })
}
