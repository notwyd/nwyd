use crate::error::{Error, Kind, Result};

const KIND: Kind = Kind::Reel;

pub const RESULT: u16 = 0x1BF;

pub const SPIN: u16 = 0x2BE;

pub const RESULT_FRAME_LEN: usize = 0x24;

pub const RESULT_PAYLOAD_LEN: usize = RESULT_FRAME_LEN - 0x0C;

pub const SPIN_FRAME_LEN: usize = 0x14;

pub const LINES: usize = 5;

pub const REELS: usize = 3;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct ResultMessage {
    pub lines: [i8; LINES],
    pub stops: [i8; REELS],
    pub ignored: [u32; 2],
    pub payout: i32,
    pub counter: u32,
    pub tail: Vec<u8>,
}

pub fn parse(payload: &[u8]) -> Result<ResultMessage> {
    if payload.len() < RESULT_PAYLOAD_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: 0,
            needed: RESULT_PAYLOAD_LEN,
            len: payload.len(),
        });
    }

    let dword = |at: usize| {
        u32::from_le_bytes(
            payload[at..at + 4]
                .try_into()
                .expect("result floor checked above"),
        )
    };

    Ok(ResultMessage {
        lines: std::array::from_fn(|index| payload[index] as i8),
        stops: std::array::from_fn(|index| payload[5 + index] as i8),
        ignored: [dword(8), dword(12)],
        payout: dword(16) as i32,
        counter: dword(20),
        tail: payload[RESULT_PAYLOAD_LEN..].to_vec(),
    })
}

pub fn spin_frame(client_id: u16, wager: i32) -> [u8; SPIN_FRAME_LEN] {
    let mut frame = [0u8; SPIN_FRAME_LEN];

    frame[4..6].copy_from_slice(&SPIN.to_le_bytes());

    frame[6..8].copy_from_slice(&client_id.to_le_bytes());

    frame[12..16].copy_from_slice(&1u32.to_le_bytes());

    frame[16..20].copy_from_slice(&wager.to_le_bytes());

    frame
}
