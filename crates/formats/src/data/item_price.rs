use super::{ITEM_RECORD_LEN, ITEM_SLOTS};

pub const ENTRIES: usize = 100;

pub const ENTRY_LEN: usize = 8;

pub const LEN: usize = ENTRIES * ENTRY_LEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct PriceOverride {
    pub item_id: i32,
    pub price: i32,
}

pub fn parse(bytes: &[u8]) -> Vec<PriceOverride> {
    let mut out = Vec::new();

    for i in 0..ENTRIES {
        let off = i * ENTRY_LEN;

        let Some(e) = bytes.get(off..off + ENTRY_LEN) else {
            break;
        };

        let item_id = i32::from_le_bytes(e[0..4].try_into().expect("4 bytes"));

        if item_id == 0 {
            break;
        }

        let price = i32::from_le_bytes(e[4..8].try_into().expect("4 bytes"));

        if (0..ITEM_SLOTS as i32).contains(&item_id) {
            out.push(PriceOverride { item_id, price });
        }
    }

    out
}

pub fn price_field_offset(item_id: i32) -> Option<usize> {
    (0..ITEM_SLOTS as i32)
        .contains(&item_id)
        .then(|| item_id as usize * ITEM_RECORD_LEN)
}
