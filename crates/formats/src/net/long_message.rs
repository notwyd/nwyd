use crate::error::{Error, Kind, Result};
use crate::text::codepage::{self, Codepage};

const KIND: Kind = Kind::LongMessage;

pub const INBOUND: u16 = 0x7B1;

pub const FRAME_LEN: usize = 0x210;

pub const HEADER_LEN: usize = 0x0C;

pub const MIN_PAYLOAD: usize = FRAME_LEN - HEADER_LEN;

pub const LINE_LEN: usize = 0x80;

pub const LINES: usize = 4;

pub mod at {

    pub const MODE: usize = 0x00;

    pub const PARM2: usize = 0x02;

    pub const LINES: usize = 0x04;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LongMessage {
    pub mode: u16,
    pub parm2: u16,
    pub lines: [String; LINES],
    pub tail: Vec<u8>,
}

fn forced_nul_text(record: &[u8]) -> String {
    let text = &record[..record.len() - 1];

    let end = text
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(text.len());

    codepage::decode(&text[..end], Codepage::Windows1252)
}

pub fn parse(payload: &[u8]) -> Result<LongMessage> {
    if payload.len() < MIN_PAYLOAD {
        return Err(Error::Truncated {
            kind: KIND,
            offset: 0,
            needed: MIN_PAYLOAD,
            len: payload.len(),
        });
    }

    let lines = std::array::from_fn(|index| {
        let start = at::LINES + index * LINE_LEN;

        forced_nul_text(&payload[start..start + LINE_LEN])
    });

    Ok(LongMessage {
        mode: u16::from_le_bytes([payload[at::MODE], payload[at::MODE + 1]]),
        parm2: u16::from_le_bytes([payload[at::PARM2], payload[at::PARM2 + 1]]),
        lines,
        tail: payload[MIN_PAYLOAD..].to_vec(),
    })
}
