use crate::error::{Error, Kind, Result};

pub const MIN_PAYLOAD: usize = 4;

pub fn parse_mode(payload: &[u8]) -> Result<i32> {
    if payload.len() < MIN_PAYLOAD {
        return Err(Error::Truncated {
            kind: Kind::BodyMark,
            offset: 0,
            needed: MIN_PAYLOAD,
            len: payload.len(),
        });
    }

    Ok(i32::from_le_bytes([
        payload[0], payload[1], payload[2], payload[3],
    ]))
}

pub const NAME_MARK_GREEN: i32 = 4;
