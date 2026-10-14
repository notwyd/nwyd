use super::envelope::{self, Integrity};
use super::fixed_string;
use crate::error::{Kind, Result};
use crate::text::Codepage;

pub const RECORDS: usize = 100;

pub const RECORD_LEN: usize = 0x28;

pub const BODY_LEN: usize = RECORDS * RECORD_LEN;

pub const LEN: usize = BODY_LEN + 4;

pub const NAME_LEN: usize = 16;

pub const SLOTS: usize = 5;

pub const NO_SLOT: u8 = 0xFF;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Slot {
    pub char_name: String,
    pub slot: [u32; SLOTS],
    pub last_used: u32,
}

impl Slot {
    pub fn is_free(&self) -> bool {
        self.last_used == 0
    }
}

pub fn parse(bytes: &[u8], codepage: Codepage) -> Result<Vec<Slot>> {
    let body = envelope::open(bytes, BODY_LEN, Integrity::TrailerB, Kind::QuickSlot)?;

    Ok(body
        .chunks_exact(RECORD_LEN)
        .map(|rec| Slot {
            char_name: fixed_string(&rec[..NAME_LEN], codepage),
            slot: std::array::from_fn(|k| {
                let off = NAME_LEN + k * 4;

                u32::from_le_bytes(rec[off..off + 4].try_into().expect("4 bytes"))
            }),
            last_used: u32::from_le_bytes(rec[0x24..0x28].try_into().expect("4 bytes")),
        })
        .collect())
}

pub fn to_bytes(slots: &[Slot]) -> Vec<u8> {
    let mut body = vec![0u8; BODY_LEN];

    for (i, s) in slots.iter().take(RECORDS).enumerate() {
        let rec = &mut body[i * RECORD_LEN..(i + 1) * RECORD_LEN];

        let name = s.char_name.as_bytes();

        let n = name.len().min(NAME_LEN - 1);

        rec[..n].copy_from_slice(&name[..n]);

        for (k, v) in s.slot.iter().enumerate() {
            let off = NAME_LEN + k * 4;

            rec[off..off + 4].copy_from_slice(&v.to_le_bytes());
        }

        rec[0x24..0x28].copy_from_slice(&s.last_used.to_le_bytes());
    }

    envelope::xor_5a(&mut body);

    let checksum = envelope::seven_step_checksum(&body);

    body.extend_from_slice(&checksum.to_le_bytes());

    body
}

pub fn select(slots: &[Slot], char_name: &str, now: u32) -> Option<usize> {
    if let Some(i) = slots.iter().position(|s| s.char_name == char_name) {
        return Some(i);
    }

    if let Some(i) = slots.iter().position(Slot::is_free) {
        return Some(i);
    }

    slots
        .iter()
        .enumerate()
        .filter(|(_, s)| s.last_used < now)
        .min_by_key(|(_, s)| s.last_used)
        .map(|(i, _)| i)
}

pub fn claim(slots: &mut [Slot], char_name: &str, now: u32) -> Option<usize> {
    let matched = slots.iter().any(|s| s.char_name == char_name);

    let free = slots.iter().any(Slot::is_free);

    let index = select(slots, char_name, now)?;

    if !matched && !free {
        slots[index] = Slot::default();
    }

    slots[index].char_name = char_name.chars().take(NAME_LEN - 1).collect();

    slots[index].last_used = now;

    Some(index)
}
