use super::ITEM_SLOTS;
use crate::text::{Codepage, codepage};

pub const RECORD_LEN: usize = 68;

pub const OBFUSCATED_PREFIX: usize = 0x3E;

pub const NAME_LEN: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Record {
    pub id: i32,
    pub name: Vec<u8>,
}

impl Record {
    pub fn text(&self, codepage: Codepage) -> String {
        codepage::decode(&self.name, codepage)
    }
}

pub fn deobfuscate(name: &mut [u8]) {
    for (i, b) in name.iter_mut().take(OBFUSCATED_PREFIX).enumerate() {
        *b = b.wrapping_sub(i as u8);
    }
}

pub fn obfuscate(name: &mut [u8]) {
    for (i, b) in name.iter_mut().take(OBFUSCATED_PREFIX).enumerate() {
        *b = b.wrapping_add(i as u8);
    }
}

pub fn parse(bytes: &[u8]) -> Vec<Record> {
    let mut out = Vec::new();

    for rec in bytes.chunks_exact(RECORD_LEN).take(ITEM_SLOTS) {
        let id = i32::from_le_bytes([rec[0], rec[1], rec[2], rec[3]]);

        if id < 0 || id as usize >= ITEM_SLOTS {
            continue;
        }

        let mut name = rec[4..4 + NAME_LEN].to_vec();

        deobfuscate(&mut name);

        let end = name.iter().position(|&b| b == 0).unwrap_or(name.len());

        name.truncate(end);

        out.push(Record { id, name });
    }

    out
}

pub const PAD: u8 = 0xFE;

pub fn to_bytes(records: &[Record]) -> Vec<u8> {
    let mut out = Vec::with_capacity(records.len() * RECORD_LEN);

    for r in records {
        out.extend_from_slice(&r.id.to_le_bytes());

        let mut field = [0u8; NAME_LEN];

        let n = r.name.len().min(OBFUSCATED_PREFIX - 1);

        field[..n].copy_from_slice(&r.name[..n]);

        field[n + 1..OBFUSCATED_PREFIX].fill(PAD);

        obfuscate(&mut field);

        out.extend_from_slice(&field);
    }

    out
}
