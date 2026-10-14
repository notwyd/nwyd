use crate::error::{Error, Kind, Result};
use crate::text::codepage::{self, Codepage};

pub const MIN_PAYLOAD: usize = 5;

pub const NAME_MAX: usize = 0x80;

pub const ACCEPT: u32 = 4;

pub const INVITE: u32 = 0;

pub const AUTO_DISMISS_S: f32 = 0.0;

pub const FLAGS: u32 = 0x2000_0001;

pub const BODY_STRDEF: usize = 362;

pub const CAPTION_STRDEF: usize = 237;

pub const CONTEXT: u32 = 0x2C;

pub const KIND: i32 = 0xD;

#[derive(Debug, Clone, PartialEq, Eq, Default)]

pub struct Invite {
    pub subject: u32,
    pub guild: String,
}

pub fn parse(payload: &[u8]) -> Result<Invite> {
    if payload.len() < MIN_PAYLOAD {
        return Err(Error::Truncated {
            kind: Kind::GuildInvite,
            offset: 0,
            needed: MIN_PAYLOAD,
            len: payload.len(),
        });
    }

    let subject = u32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]);

    let rest = &payload[4..];

    let end = rest
        .iter()
        .position(|b| *b == 0)
        .unwrap_or(rest.len())
        .min(NAME_MAX);

    Ok(Invite {
        subject,
        guild: codepage::decode(&rest[..end], Codepage::Windows1252),
    })
}

pub fn reply_payload(subject: u32) -> [u8; 8] {
    let mut out = [0u8; 8];

    out[0..4].copy_from_slice(&subject.to_le_bytes());

    out[4..8].copy_from_slice(&ACCEPT.to_le_bytes());

    out
}

pub const REPLY_LEN: usize = 0x14;
