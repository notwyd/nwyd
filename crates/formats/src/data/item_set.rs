use super::ITEM_SLOTS;
use super::envelope::{self, Integrity};
use crate::error::{Kind, Result};

pub const RECORD_LEN: usize = 0x40;

pub const EXPECTED_BODY_LEN: usize = ITEM_SLOTS * RECORD_LEN;

pub const SHIPPED_RECORDS: usize = 6_500;

pub const CHECKSUM: i32 = 0x03BE_2B3E;

pub const EFFECT_OFFSET: usize = 0x3C;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SetBonus {
    pub effect: i32,
}

pub fn parse(bytes: &[u8]) -> Vec<SetBonus> {
    let records = bytes.len() / RECORD_LEN;

    let body_len = records * RECORD_LEN;

    let mut body = bytes[..body_len].to_vec();

    envelope::xor_5a(&mut body);

    body.chunks_exact(RECORD_LEN)
        .map(|rec| SetBonus {
            effect: i32::from_le_bytes(
                rec[EFFECT_OFFSET..EFFECT_OFFSET + 4]
                    .try_into()
                    .expect("4 bytes"),
            ),
        })
        .collect()
}

pub fn parse_strict(bytes: &[u8]) -> Result<Vec<SetBonus>> {
    let body = envelope::open(
        bytes,
        EXPECTED_BODY_LEN,
        Integrity::ConstantA(CHECKSUM),
        Kind::ItemSet,
    )?;

    Ok(body
        .chunks_exact(RECORD_LEN)
        .map(|rec| SetBonus {
            effect: i32::from_le_bytes(
                rec[EFFECT_OFFSET..EFFECT_OFFSET + 4]
                    .try_into()
                    .expect("4 bytes"),
            ),
        })
        .collect())
}

pub fn effect_for(table: &[SetBonus], set_index: u32) -> Option<i32> {
    table.get(set_index as usize).map(|s| s.effect)
}
