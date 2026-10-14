use super::Item;

pub const RECORDS: usize = 210;

pub const RECORD_LEN: usize = 0x70;

pub const LEN: usize = RECORDS * RECORD_LEN;

pub const NEEDED: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Entry {
    pub id: i32,
    pub request_id: i32,
    pub progress_index: i32,
    pub class_split: i32,
    pub d: i32,
    pub required_count: i32,
    pub need: [Item; NEEDED],
    pub primary_reward_effect: i32,
    pub secondary_reward_effect: i32,
    pub primary_reward_value: i32,
    pub secondary_reward_value: i32,
}

impl Entry {
    pub fn is_populated(&self) -> bool {
        self.id != 0 || self.request_id != 0 || self.progress_index != 0
    }

    pub fn has_requirements(&self) -> bool {
        self.need.iter().any(|i| !i.is_empty())
    }

    pub fn bit_index(&self) -> Option<usize> {
        usize::try_from(self.progress_index).ok()
    }

    pub fn class_split(&self) -> i32 {
        self.class_split
    }

    pub fn is_listed(&self) -> bool {
        self.required_count != 0
    }

    pub fn payload(&self) -> i32 {
        self.request_id
    }
}

fn i32_at(rec: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([rec[off], rec[off + 1], rec[off + 2], rec[off + 3]])
}

pub fn parse(bytes: &[u8]) -> Vec<Entry> {
    let mut out = Vec::with_capacity(RECORDS);

    for i in 0..RECORDS {
        let rec = bytes.get(i * RECORD_LEN..(i + 1) * RECORD_LEN);

        out.push(match rec {
            Some(rec) => Entry {
                id: i32_at(rec, 0x00),
                request_id: i32_at(rec, 0x04),
                progress_index: i32_at(rec, 0x08),
                class_split: i32_at(rec, 0x0C),
                d: i32_at(rec, 0x10),
                required_count: i32_at(rec, 0x14),
                need: std::array::from_fn(|k| Item::parse(&rec[0x18 + k * Item::LEN..])),
                primary_reward_effect: i32_at(rec, 0x60),
                secondary_reward_effect: i32_at(rec, 0x64),
                primary_reward_value: i32_at(rec, 0x68),
                secondary_reward_value: i32_at(rec, 0x6C),
            },
            None => Entry {
                id: 0,
                request_id: 0,
                progress_index: 0,
                class_split: 0,
                d: 0,
                required_count: 0,
                need: [Item::default(); NEEDED],
                primary_reward_effect: 0,
                secondary_reward_effect: 0,
                primary_reward_value: 0,
                secondary_reward_value: 0,
            },
        });
    }

    out
}

pub fn satisfies(requirement: Item, player_item_base_id: u32) -> bool {
    !requirement.is_empty() && requirement.index as u32 == player_item_base_id
}
