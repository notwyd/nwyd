use crate::data::Item;
use crate::error::{Error, Kind, Result};

pub const RECORDS: usize = 5_000;

pub const RECORD_LEN: usize = 0x1500;

pub const LEN: usize = RECORDS * RECORD_LEN;

pub const OPTIONS: usize = 0x18;

pub const MATERIALS: usize = 8;

pub const REWARDS: usize = 10;

const OPTIONS_AT: usize = 0x10;

const MATERIALS_AT: usize = 0x70;

const REWARDS_AT: usize = 0xD0;

pub const RULES_AT: usize = 0x1C0;

pub const RULE_BLOCK: usize = 0x1E0;

pub const RULE_GROUPS: usize = MATERIALS;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Rule {
    pub kind: i32,
    pub count: i32,
}

pub const TAIL_AT: usize = REWARDS_AT + REWARDS * Item::LEN;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mission {
    pub gold_cost: i32,
    pub key: i32,
    pub id: i32,
    pub parent: i32,
    pub options: [i32; OPTIONS],
    pub material: [Item; MATERIALS],
    pub reward: [Item; REWARDS],
    pub tail: Vec<u8>,
}

impl Mission {
    pub fn condition(&self, block: usize, row: usize) -> Option<[i32; 6]> {
        if block >= 10 || row >= 20 {
            return None;
        }

        let at = RULES_AT + block * RULE_BLOCK + row * 24 - TAIL_AT;

        let raw = self.tail.get(at..at + 24)?;

        Some(std::array::from_fn(|i| i32_at(raw, i * 4)))
    }

    pub fn alternative(&self, slot: usize) -> Option<(Item, u32)> {
        if slot >= 8 {
            return None;
        }

        let at = 0x1480 + slot * 12 - TAIL_AT;

        let item = Item::parse(self.tail.get(at..at + 12)?);

        let at = 0x14E0 + slot * 4 - TAIL_AT;

        let bytes = self.tail.get(at..at + 4)?;

        Some((item, u32::from_le_bytes(bytes.try_into().ok()?)))
    }

    pub fn is_empty(&self) -> bool {
        self.id == 0
    }

    pub fn is_top_level(&self) -> bool {
        self.parent == -1
    }

    pub fn rule(&self, group: usize) -> Option<Rule> {
        if group >= RULE_GROUPS {
            return None;
        }

        let at = RULES_AT + group * RULE_BLOCK - TAIL_AT;

        let raw = self.tail.get(at..at + 8)?;

        let kind = i32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]);

        if kind == 0 {
            return None;
        }

        Some(Rule {
            kind,
            count: i32::from_le_bytes([raw[4], raw[5], raw[6], raw[7]]),
        })
    }

    pub fn children(&self) -> impl Iterator<Item = i32> + '_ {
        self.options
            .iter()
            .copied()
            .take_while(|&o| o != 0 && o != -1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissionItems {
    pub missions: Vec<Mission>,
}

impl MissionItems {
    pub fn get(&self, index: usize) -> Option<&Mission> {
        self.missions.get(index).filter(|m| !m.is_empty())
    }

    pub fn menu(&self, key: i32) -> Vec<&Mission> {
        self.missions
            .iter()
            .filter(|m| !m.is_empty() && m.key == key && m.is_top_level())
            .take(OPTIONS)
            .collect()
    }

    pub fn siblings_of_parent(&self, index: usize) -> Vec<&Mission> {
        let Some(row) = self.get(index) else {
            return Vec::new();
        };

        let Ok(parent_index) = usize::try_from(row.parent) else {
            return Vec::new();
        };

        let Some(parent) = self.get(parent_index) else {
            return Vec::new();
        };

        let (key, grandparent) = (parent.key, parent.parent);

        self.missions
            .iter()
            .filter(|m| !m.is_empty() && m.key == key && m.parent == grandparent)
            .take(OPTIONS)
            .collect()
    }

    pub fn children_of(&self, index: usize) -> Vec<&Mission> {
        let Some(row) = self.get(index) else {
            return Vec::new();
        };

        row.children()
            .filter_map(|c| usize::try_from(c).ok())
            .filter_map(|c| self.get(c))
            .take(OPTIONS)
            .collect()
    }

    pub fn occupied(&self) -> Vec<usize> {
        (0..self.missions.len())
            .filter(|&i| self.get(i).is_some())
            .collect()
    }

    pub fn keys(&self) -> Vec<i32> {
        let mut out: Vec<i32> = self
            .missions
            .iter()
            .filter(|m| !m.is_empty() && m.is_top_level())
            .map(|m| m.key)
            .collect();

        out.sort_unstable();

        out.dedup();

        out
    }
}

fn i32_at(r: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]])
}

fn items<const N: usize>(r: &[u8], at: usize) -> [Item; N] {
    let mut out = [Item::default(); N];

    for (i, slot) in out.iter_mut().enumerate() {
        *slot = Item::parse(&r[at + i * Item::LEN..]);
    }

    out
}

pub fn parse(bytes: &[u8]) -> Result<MissionItems> {
    if bytes.len() != LEN {
        return Err(Error::structure(
            Kind::MissionItems,
            format!("expected exactly {LEN} bytes, found {}", bytes.len()),
        ));
    }

    let missions = bytes
        .chunks_exact(RECORD_LEN)
        .map(|r| {
            let mut options = [0i32; OPTIONS];

            for (i, o) in options.iter_mut().enumerate() {
                *o = i32_at(r, OPTIONS_AT + i * 4);
            }

            Mission {
                gold_cost: i32_at(r, 0x00),
                key: i32_at(r, 0x04),
                id: i32_at(r, 0x08),
                parent: i32_at(r, 0x0C),
                options,
                material: items(r, MATERIALS_AT),
                reward: items(r, REWARDS_AT),
                tail: r[TAIL_AT..].to_vec(),
            }
        })
        .collect();

    Ok(MissionItems { missions })
}
