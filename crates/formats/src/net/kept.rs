use crate::error::{Error, Kind as ErrorKind, Result};
use crate::text::codepage::{self, Codepage};
use crate::util::u32_at;

const LINE_KIND: ErrorKind = ErrorKind::FieldLines;

const RECORD_KIND: ErrorKind = ErrorKind::KeyedRecord;

pub const LINES: u16 = 0x2C8;

pub const RECORD: u16 = 0xDC3;

pub mod at {

    pub const INDEX: usize = 0x00;

    pub const TEXT: usize = 0x04;
}

pub mod record_at {

    pub const KEY: usize = 0x0C;

    pub const TENTHS: usize = 0x10;
}

pub const LINE_SLOTS: usize = 5;

pub const LINE_LEN: usize = 0x80;

pub const LINE_STRIDE: usize = 0x80;

pub const LINE_BASE: usize = 0x2_8A59;

pub const OBFUSCATION: u8 = 0x61;

pub const LINES_LEN: usize = at::TEXT + LINE_LEN;

pub const STAMP_AT: usize = 0x2_7716;

pub const RECORD_SLOTS: usize = 12;

pub const RECORD_LEN: usize = 0x40;

pub const RECORD_STRIDE: usize = 0x40;

pub const TABLE_AT: usize = 0x118C;

pub const GATE_AT: usize = 0x0099_976C;

pub const TENTHS_DIVISOR: i16 = 10;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Line {
    pub slot: usize,
    pub text: String,
    pub clamped: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Record {
    pub bytes: [u8; RECORD_LEN],
}

impl Record {
    pub fn key(&self) -> u32 {
        u32::from_le_bytes([
            self.bytes[record_at::KEY],
            self.bytes[record_at::KEY + 1],
            self.bytes[record_at::KEY + 2],
            self.bytes[record_at::KEY + 3],
        ])
    }

    pub fn tenths(&self) -> i16 {
        i16::from_le_bytes([
            self.bytes[record_at::TENTHS],
            self.bytes[record_at::TENTHS + 1],
        ])
    }

    pub fn split_tenths(&self) -> (i16, i16) {
        (
            self.tenths() / TENTHS_DIVISOR,
            self.tenths() % TENTHS_DIVISOR,
        )
    }

    pub fn sealed_character(&self) -> SealedCharacter {
        let word = |at| u16::from_le_bytes([self.bytes[at], self.bytes[at + 1]]);

        let signed = |at| i32::from(word(at) as i16);

        SealedCharacter {
            body: signed(0x10),
            level: signed(0x12),
            max_level: signed(0x38),
            stat: std::array::from_fn(|i| signed(0x14 + i * 2)),
            skills: std::array::from_fn(|i| signed(0x24 + i * 2)),
            crystals: u32::from(word(0x36)),
            held_item: i32::from(word(0x3A)),
            held_count: signed(0x3C),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct SealedCharacter {
    pub body: i32,
    pub level: i32,
    pub max_level: i32,
    pub stat: [i32; 4],
    pub skills: [i32; 9],
    pub crystals: u32,
    pub held_item: i32,
    pub held_count: i32,
}

pub fn clamp_slot(raw: i32) -> usize {
    raw.clamp(0, LINE_SLOTS as i32 - 1) as usize
}

pub fn uppercase_ascii_until_nul(bytes: &mut [u8]) {
    for b in bytes.iter_mut() {
        if *b == 0 {
            return;
        }

        if b.is_ascii_lowercase() {
            *b -= 0x20;
        }
    }
}

pub fn parse_line(payload: &[u8]) -> Result<Line> {
    let raw = u32_at(payload, at::INDEX, LINE_KIND)? as i32;

    let text = payload
        .get(at::TEXT..at::TEXT + LINE_LEN)
        .ok_or(Error::Truncated {
            kind: LINE_KIND,
            offset: at::TEXT,
            needed: LINE_LEN,
            len: payload.len(),
        })?;

    let mut plain: Vec<u8> = text.iter().map(|b| b ^ OBFUSCATION).collect();

    plain[LINE_LEN - 1] = 0;

    uppercase_ascii_until_nul(&mut plain);

    let cut = plain.iter().position(|b| *b == 0).unwrap_or(LINE_LEN - 1);

    Ok(Line {
        slot: clamp_slot(raw),
        text: codepage::decode(&plain[..cut], Codepage::Windows1252),
        clamped: raw != clamp_slot(raw) as i32,
    })
}

pub fn parse_record(frame: &[u8]) -> Result<Record> {
    let bytes: [u8; RECORD_LEN] = frame
        .get(..RECORD_LEN)
        .and_then(|s| s.try_into().ok())
        .ok_or(Error::Truncated {
            kind: RECORD_KIND,
            offset: 0,
            needed: RECORD_LEN,
            len: frame.len(),
        })?;

    Ok(Record { bytes })
}

pub fn slot_for(keys: &[u32], key: u32) -> usize {
    if let Some(i) = keys.iter().position(|k| *k == key) {
        return i;
    }

    if let Some(i) = keys.iter().position(|k| *k == 0) {
        return i;
    }

    0
}
