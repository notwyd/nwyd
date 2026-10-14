use crate::data::item_list::{Entry, ItemList, item_type};
use crate::data::mount::MountData;
use crate::mesh::appearance::refine_level;
use crate::net::account::Item;

pub const EXCLUDED_IDS: std::ops::RangeInclusive<i32> = 0xC80..=0xCE4;

pub const MAX_ID: i32 = 0x38A3;

pub mod effect {

    pub const REQ_LEVEL: u8 = 0x01;

    pub const DAMAGE: u8 = 0x02;

    pub const DEFENSE: u8 = 0x03;

    pub const ATTACK_SPEED: u8 = 0x1A;

    pub const MOVE_SPEED: u8 = 0x1D;

    pub const EVASION: u8 = 0x28;

    pub const EQUIP_MASK: u8 = 0x11;

    pub const CLASS: u8 = 0x12;

    pub const REQ_STR: u8 = 0x16;

    pub const REQ_INT: u8 = 0x17;

    pub const REQ_DEX: u8 = 0x18;

    pub const REQ_CON: u8 = 0x19;

    pub const HP_RECOVERY: u8 = 0x2F;

    pub const MANA_RECOVERY: u8 = 0x30;

    pub const MAX_HP_RATE: u8 = 0x2D;

    pub const MAX_MANA_RATE: u8 = 0x2E;

    pub const MAX_HP_RATE_ALT: u8 = 0x45;

    pub const MAX_MANA_RATE_ALT: u8 = 0x46;

    pub const CRITICAL: u8 = 0x2A;

    pub const CRITICAL_ALT: u8 = 0x47;

    pub const DEFENSE_ALT: u8 = 0x35;

    pub const DEFENSE_ALT2: u8 = 0x48;

    pub const DAMAGE_BOOT: u8 = 0x49;

    pub const MAGIC_ATTACK: u8 = 0x3C;

    pub const DAMAGE_GRADED: u8 = 0x43;

    pub const MAGIC_ATTACK_GRADED: u8 = 0x44;

    pub const PVP_DAMAGE: u8 = 0x38;

    pub const PVP_ARMOR: u8 = 0x39;

    pub const TRADE: u8 = 0x6F;

    pub const GRADE_LETTER: u8 = 0x57;

    pub const PET_PROGRESS: u8 = 0x50;

    pub const PET_MATURITY: u8 = 0x51;

    pub const PET_VITALITY: u8 = 0x4F;

    pub const PET_FOOD: u8 = 0x52;

    pub const PET_QUALITY: u8 = 0x53;

    pub const USE_KIND: u8 = 0x26;

    pub const ALL_RESIST: u8 = 0x36;

    pub const RESTRICTION: u8 = 0x70;

    pub const OFF_HAND_LADDER: u8 = 0x15;
}

pub const NO_SCALE_SLOT: [u8; 13] = [
    0x12, 0x11, 0x15, 0x1B, 0x01, 0x16, 0x17, 0x18, 0x19, 0x26, 0x4E, 0x54, 0x56,
];

pub const NO_SCALE_TEMPLATE: [u8; 15] = [
    0x12, 0x11, 0x15, 0x1B, 0x01, 0x16, 0x17, 0x18, 0x19, 0x26, 0x4E, 0x54, 0x56, 0x30, 0x2F,
];

pub const NO_SCALE_TOTAL: [u8; 19] = [
    0x12, 0x11, 0x15, 0x1B, 0x01, 0x16, 0x17, 0x18, 0x19, 0x26, 0x4E, 0x54, 0x56, 0x70, 0x57, 0x58,
    0x30, 0x2F, 0x71,
];

pub const SCALE_MIN_ID: i32 = 0x29;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectRow {
    pub id: u8,
    pub strdef: u16,
}

pub const ROWS: [EffectRow; 59] = [
    EffectRow {
        id: 0x12,
        strdef: 73,
    },
    EffectRow {
        id: 0x01,
        strdef: 74,
    },
    EffectRow {
        id: 0x16,
        strdef: 75,
    },
    EffectRow {
        id: 0x17,
        strdef: 76,
    },
    EffectRow {
        id: 0x18,
        strdef: 77,
    },
    EffectRow {
        id: 0x19,
        strdef: 78,
    },
    EffectRow {
        id: 0x02,
        strdef: 79,
    },
    EffectRow {
        id: 0x03,
        strdef: 80,
    },
    EffectRow {
        id: 0x04,
        strdef: 81,
    },
    EffectRow {
        id: 0x05,
        strdef: 82,
    },
    EffectRow {
        id: 0x2D,
        strdef: 83,
    },
    EffectRow {
        id: 0x2E,
        strdef: 84,
    },
    EffectRow {
        id: 0x2C,
        strdef: 85,
    },
    EffectRow {
        id: 0x2A,
        strdef: 86,
    },
    EffectRow {
        id: 0x2F,
        strdef: 87,
    },
    EffectRow {
        id: 0x30,
        strdef: 88,
    },
    EffectRow {
        id: 0x36,
        strdef: 89,
    },
    EffectRow {
        id: 0x28,
        strdef: 174,
    },
    EffectRow {
        id: 0x1D,
        strdef: 90,
    },
    EffectRow {
        id: 0x31,
        strdef: 91,
    },
    EffectRow {
        id: 0x32,
        strdef: 92,
    },
    EffectRow {
        id: 0x33,
        strdef: 93,
    },
    EffectRow {
        id: 0x34,
        strdef: 94,
    },
    EffectRow {
        id: 0x0B,
        strdef: 95,
    },
    EffectRow {
        id: 0x0C,
        strdef: 96,
    },
    EffectRow {
        id: 0x0D,
        strdef: 97,
    },
    EffectRow {
        id: 0x0E,
        strdef: 98,
    },
    EffectRow {
        id: 0x1A,
        strdef: 99,
    },
    EffectRow {
        id: 0x4A,
        strdef: 140,
    },
    EffectRow {
        id: 0x07,
        strdef: 100,
    },
    EffectRow {
        id: 0x08,
        strdef: 101,
    },
    EffectRow {
        id: 0x09,
        strdef: 102,
    },
    EffectRow {
        id: 0x0A,
        strdef: 103,
    },
    EffectRow {
        id: 0x3C,
        strdef: 104,
    },
    EffectRow {
        id: 0x5B,
        strdef: 105,
    },
    EffectRow {
        id: 0x40,
        strdef: 128,
    },
    EffectRow {
        id: 0x41,
        strdef: 129,
    },
    EffectRow {
        id: 0x42,
        strdef: 130,
    },
    EffectRow {
        id: 0x35,
        strdef: 80,
    },
    EffectRow {
        id: 0x43,
        strdef: 79,
    },
    EffectRow {
        id: 0x44,
        strdef: 104,
    },
    EffectRow {
        id: 0x49,
        strdef: 79,
    },
    EffectRow {
        id: 0x4E,
        strdef: 162,
    },
    EffectRow {
        id: 0x4F,
        strdef: 163,
    },
    EffectRow {
        id: 0x50,
        strdef: 164,
    },
    EffectRow {
        id: 0x51,
        strdef: 165,
    },
    EffectRow {
        id: 0x52,
        strdef: 166,
    },
    EffectRow {
        id: 0x54,
        strdef: 169,
    },
    EffectRow {
        id: 0x53,
        strdef: 171,
    },
    EffectRow {
        id: 0x4C,
        strdef: 502,
    },
    EffectRow {
        id: 0x4D,
        strdef: 503,
    },
    EffectRow {
        id: 0x29,
        strdef: 445,
    },
    EffectRow {
        id: 0x4B,
        strdef: 196,
    },
    EffectRow {
        id: 0x1C,
        strdef: 525,
    },
    EffectRow {
        id: 0x38,
        strdef: 540,
    },
    EffectRow {
        id: 0x39,
        strdef: 541,
    },
    EffectRow {
        id: 0x5E,
        strdef: 551,
    },
    EffectRow {
        id: 0x5F,
        strdef: 552,
    },
    EffectRow {
        id: 0x60,
        strdef: 553,
    },
];

pub const REQUIREMENT_ROWS: std::ops::Range<usize> = 0..6;

#[derive(Debug, Clone, Copy)]
pub struct Effects<'a> {
    pub items: &'a ItemList,
    pub mounts: &'a [MountData],
}

impl<'a> Effects<'a> {
    pub fn new(items: &'a ItemList, mounts: &'a [MountData]) -> Effects<'a> {
        Effects { items, mounts }
    }

    pub fn template(&self, item: &Item, id: u8) -> i32 {
        let Some((item_id, entry)) = self.lookup(item) else {
            return 0;
        };

        let mut sum = self.preload(&entry, id);

        if id != 0x4E {
            sum += template_sum(&entry, id);
        }

        if let Some(v) = self.pet_effect_override(item, &entry, id, sum, false) {
            return v;
        }

        if entry.item_type == item_type::MOUNT {
            return self.mount_table_effect(&entry, id).unwrap_or(sum);
        }

        let refine = self.scaling_refine(item, &entry);

        if refine != 0 && sum > 0 && !NO_SCALE_TEMPLATE.contains(&id) {
            sum = refine_scale_with_mask(item_id, refine, sum, entry.equip_mask);
        }

        post_scale(id, refine, sum)
    }

    pub fn slot(&self, item: &Item, id: u8) -> i32 {
        let Some((_, entry)) = self.lookup(item) else {
            return 0;
        };

        if item_type::is_mount_backed(entry.item_type) {
            return 0;
        }

        slot_sum(item, id)
    }

    pub fn slot_scaled(&self, item: &Item, id: u8) -> i32 {
        let Some((item_id, entry)) = self.lookup(item) else {
            return 0;
        };

        if item_type::is_mount_backed(entry.item_type) {
            return 0;
        }

        let sum = slot_sum(item, id);

        let refine = self.scaling_refine(item, &entry);

        if refine != 0 && sum != 0 && !NO_SCALE_SLOT.contains(&id) {
            return refine_scale_with_mask(item_id, refine, sum, entry.equip_mask);
        }

        sum
    }

    pub fn total(&self, item: &Item, id: u8) -> i32 {
        let Some((_, entry)) = self.lookup(item) else {
            return 0;
        };

        if gated_out_by_grade(&entry, id) {
            return 0;
        }

        let id = alias(item, &entry, id);

        let mut sum = self.preload(&entry, id);

        if id != 0x4E {
            sum += template_sum(&entry, id);
        }

        if let Some(v) = self.pet_effect_override(item, &entry, id, sum, true) {
            return v;
        }

        if entry.item_type == item_type::MOUNT {
            return self.mount_table_effect(&entry, id).unwrap_or(sum);
        }

        sum + slot_sum(item, id)
    }

    pub fn total_scaled(&self, item: &Item, id: u8) -> i32 {
        let Some((item_id, entry)) = self.lookup(item) else {
            return 0;
        };

        if gated_out_by_grade(&entry, id) {
            return 0;
        }

        let id = alias(item, &entry, id);

        let mut sum = self.preload(&entry, id);

        if id != 0x4E {
            sum += template_sum(&entry, id);
        }

        if let Some(v) = self.pet_effect_override(item, &entry, id, sum, true) {
            return v;
        }

        if entry.item_type == item_type::MOUNT {
            return self.mount_table_effect(&entry, id).unwrap_or(sum);
        }

        sum += slot_sum(item, id);

        if id == effect::TRADE {
            return sum;
        }

        let refine = if item_id < SCALE_MIN_ID {
            0
        } else {
            self.scaling_refine(item, &entry)
        };

        if refine != 0 && sum > 0 && !NO_SCALE_TOTAL.contains(&id) {
            return refine_scale_with_mask(item_id, refine, sum, entry.equip_mask);
        }

        post_scale(id, refine, sum)
    }

    fn lookup(&self, item: &Item) -> Option<(i32, Entry)> {
        let id = i32::from(item.index as i16);

        if (id as u32).wrapping_sub(1) > (MAX_ID - 1) as u32 {
            return None;
        }

        if EXCLUDED_IDS.contains(&id) {
            return None;
        }

        self.items
            .get(usize::try_from(id).ok()?)
            .map(|entry| (id, entry))
    }

    fn preload(&self, entry: &Entry, id: u8) -> i32 {
        match id {
            effect::REQ_LEVEL => i32::from(entry.req_level),
            effect::REQ_STR => i32::from(entry.req_str),
            effect::REQ_INT => i32::from(entry.req_int),
            effect::REQ_DEX => i32::from(entry.req_dex),
            effect::REQ_CON => i32::from(entry.req_con),
            effect::EQUIP_MASK => entry.equip_mask as i32,
            _ => 0,
        }
    }

    fn scaling_refine(&self, item: &Item, entry: &Entry) -> i32 {
        let mut refine = refine_level(item, self.items) as i32;

        if refine > 8 && entry.equip_mask & 0xF00 != 0 {
            refine += 1;
        }

        refine
    }

    fn pet_effect_override(
        &self,
        item: &Item,
        entry: &Entry,
        id: u8,
        sum: i32,
        bonus: bool,
    ) -> Option<i32> {
        if !matches!(
            entry.item_type,
            item_type::PET_STAGE1 | item_type::PET_STAGE2
        ) {
            return None;
        }

        let pet = PetState::of(item);

        match id {
            effect::PET_PROGRESS => return Some(i32::from(pet.progress)),
            effect::PET_MATURITY => return Some(i32::from(pet.maturity)),
            effect::PET_VITALITY => return Some(i32::from(pet.vitality)),
            effect::PET_FOOD => return Some(i32::from(pet.food)),
            effect::PET_QUALITY => return Some(i32::from(pet.quality)),
            _ => {}
        }

        let index = i32::from(entry.mount_index);

        if entry.item_type != item_type::PET_STAGE2 || index < 2 || pet.progress == 0 {
            return Some(sum);
        }

        if index >= crate::data::mount::RECORDS as i32 {
            return Some(0);
        }

        let Some(row) = self.mounts.get(index as usize) else {
            return self.mount_table_effect(entry, id).or(Some(sum));
        };

        let mut value = match id {
            effect::DAMAGE => (i32::from(pet.maturity) + 20) * row.atk / 100,
            effect::MAGIC_ATTACK => (i32::from(pet.maturity) + 15) * row.def / 100,
            _ => return self.mount_table_effect(entry, id).or(Some(sum)),
        };

        if bonus && pet.quality != 0 {
            value = (i32::from(pet.quality) + 100) * value / 100;
        }

        Some(value)
    }

    fn mount_table_effect(&self, entry: &Entry, id: u8) -> Option<i32> {
        let index = usize::try_from(entry.mount_index).ok()?;

        if index >= crate::data::mount::RECORDS {
            return Some(0);
        }

        let row = self.mounts.get(index)?;

        match id {
            effect::DAMAGE => Some(row.atk),
            effect::MAGIC_ATTACK => Some(row.def),
            effect::EVASION => Some(row.evasion),
            effect::ALL_RESIST => Some(row.all_resistance),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PetState {
    pub progress: u16,
    pub maturity: u8,
    pub vitality: u8,
    pub food: u8,
    pub quality: u8,
}

impl PetState {
    pub fn of(item: &Item) -> PetState {
        PetState {
            progress: u16::from(item.effects[0].id) | (u16::from(item.effects[0].value) << 8),
            maturity: item.effects[1].id,
            vitality: item.effects[1].value,
            food: item.effects[2].id,
            quality: item.effects[2].value,
        }
    }
}

fn template_sum(entry: &Entry, id: u8) -> i32 {
    let want = i16::from(id);

    entry
        .effect
        .iter()
        .filter(|e| e.id == want)
        .map(|e| adjust(id, i32::from(e.value)))
        .sum()
}

fn slot_sum(item: &Item, id: u8) -> i32 {
    item.effects
        .iter()
        .filter(|e| e.id == id)
        .map(|e| adjust(id, i32::from(e.value)))
        .sum()
}

fn adjust(id: u8, value: i32) -> i32 {
    if id == effect::ATTACK_SPEED && value == 1 {
        100
    } else {
        value
    }
}

fn alias(item: &Item, entry: &Entry, id: u8) -> u8 {
    let has = |c: u8| item.effects[1].id == c || item.effects[2].id == c;

    match id {
        effect::CRITICAL if has(b'G') => effect::CRITICAL_ALT,
        effect::DAMAGE if entry.equip_mask == 0x20 && has(b'I') => effect::DAMAGE_BOOT,
        effect::MAX_HP_RATE if has(b'E') => effect::MAX_HP_RATE_ALT,
        effect::MAX_MANA_RATE if has(b'F') => effect::MAX_MANA_RATE_ALT,
        effect::DEFENSE_ALT if has(b'H') => effect::DEFENSE_ALT2,
        _ => id,
    }
}

fn gated_out_by_grade(entry: &Entry, id: u8) -> bool {
    matches!(id, effect::DAMAGE_GRADED | effect::MAGIC_ATTACK_GRADED)
        && !(0x29..=0x32).contains(&entry.grade)
}

fn post_scale(id: u8, refine: i32, sum: i32) -> i32 {
    if id == effect::MOVE_SPEED {
        let clamped = if sum >= 3 { 2 } else { sum };

        if clamped < 1 {
            return clamped;
        }

        return if refine > 8 { clamped + 1 } else { clamped };
    }

    if matches!(id, effect::PVP_DAMAGE | effect::PVP_ARMOR) {
        return sum & 0xFF;
    }

    if matches!(id, effect::HP_RECOVERY | effect::MANA_RECOVERY) {
        return refine * sum;
    }

    sum
}

pub fn refine_scale_with_mask(item_id: i32, refine: i32, value: i32, mask: u32) -> i32 {
    if (6493..=6499).contains(&item_id) {
        let level = if refine > 9 && mask & 0xf00 != 0 {
            refine - 1
        } else {
            refine
        };

        let pct = [
            420i32, 423, 428, 431, 436, 439, 442, 447, 454, 460, 471, 480, 491, 501, 514,
        ][level.clamp(1, 15) as usize - 1];

        return pct.wrapping_mul(value) / 100;
    }

    base_refine_scale(item_id, refine, value)
}

pub fn refine_scale(item_id: i32, refine: i32, value: i32) -> i32 {
    refine_scale_with_mask(item_id, refine, value, 0)
}

fn base_refine_scale(item_id: i32, refine: i32, value: i32) -> i32 {
    let pct = if is_special_family(item_id) {
        let ladder = if is_ladder_a_family(item_id) {
            &LADDER_A
        } else {
            &LADDER_B
        };

        ladder_at(ladder, refine)
    } else if matches!(item_id, 0xDB5 | 0xDB6) {
        ladder_at(&LADDER_MANTLE, refine)
    } else if (0x195D..=0x1963).contains(&item_id) {
        let refine = if matches!(item_id, 0x195D | 0x1962 | 0x1963) && refine > 9 {
            refine - 1
        } else {
            refine
        };

        ladder_at(&LADDER_CAPE, refine)
    } else if refine < 11 {
        return (refine + 10) * value / 10;
    } else {
        ladder_at(&LADDER_HIGH, refine)
    };

    pct * value / 100
}

fn ladder_at(ladder: &[i32], refine: i32) -> i32 {
    usize::try_from(refine - 1)
        .ok()
        .and_then(|k| ladder.get(k))
        .copied()
        .unwrap_or(refine)
}

pub const LADDER_MANTLE: [i32; 15] = [
    220, 230, 240, 250, 260, 270, 280, 290, 300, 310, 320, 340, 360, 380, 400,
];

pub const LADDER_CAPE: [i32; 15] = [
    165, 168, 171, 174, 177, 180, 181, 183, 187, 189, 193, 195, 198, 201, 205,
];

pub const LADDER_HIGH: [i32; 16] = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 220, 250, 280, 320, 370, 420];

pub const LADDER_A: [i32; 15] = [
    400, 405, 410, 415, 420, 425, 435, 445, 455, 465, 475, 495, 515, 535, 560,
];

pub const LADDER_B: [i32; 15] = [
    230, 250, 270, 290, 310, 330, 350, 370, 380, 390, 410, 440, 470, 510, 560,
];

pub fn is_special_family(item_id: i32) -> bool {
    const WINDOWS: [(i32, i32); 15] = [
        (0x4DA, 0x4E1),
        (0x561, 0x568),
        (0x5F7, 0x5FE),
        (0x68D, 0x694),
        (0x6B3, 0x6B4),
        (0xE19, 0xE1C),
        (0xE2D, 0xE30),
        (0xE41, 0xE44),
        (0xE55, 0xE58),
        (0xE69, 0xE6C),
        (0xE7D, 0xE80),
        (0xE99, 0xEA0),
        (0xEB9, 0xEBC),
        (0xECD, 0xED0),
        (0x1942, 0x1951),
    ];

    WINDOWS.iter().any(|&(lo, hi)| (lo..=hi).contains(&item_id))
}

pub fn is_ladder_a_family(item_id: i32) -> bool {
    (0x1942..=0x1951).contains(&item_id)
}

pub mod quality {

    pub const NEUTRAL: u32 = 0xFFAA_AAFF;

    pub const PLAIN: u32 = 0xFF99_EE99;

    pub const GOOD: u32 = 0xFFFF_FFAA;

    pub const BEST: u32 = 0xFFFF_AA00;

    pub fn rank(color: u32) -> u8 {
        match color {
            NEUTRAL => 1,
            PLAIN => 2,
            GOOD => 3,
            BEST => 4,
            _ => 0,
        }
    }
}

pub fn quality_color(equip_mask: u32, id: u8, value: i32) -> u32 {
    fn step(value: i32, at: i32) -> Option<u32> {
        (value >= at).then_some(if value == at {
            quality::GOOD
        } else {
            quality::BEST
        })
    }

    fn band(value: i32, from: i32, to: i32) -> Option<u32> {
        (value > from).then_some(if value > to {
            quality::BEST
        } else {
            quality::GOOD
        })
    }

    let color = if equip_mask < 0x40 {
        match id {
            effect::MAGIC_ATTACK | effect::MAGIC_ATTACK_GRADED => {
                if equip_mask == 0x02 {
                    band(value, 0x0B, 0x0E)
                } else {
                    step(value, 0x06)
                }
            }
            effect::CRITICAL | effect::CRITICAL_ALT => band(value, 0x31, 0x3B),
            effect::ATTACK_SPEED | 0x4A => step(value, 0x0C),
            effect::DEFENSE | effect::DEFENSE_ALT | effect::DEFENSE_ALT2 => {
                step(value, if equip_mask == 0x10 { 0x1E } else { 0x0F })
            }
            effect::DAMAGE | effect::DAMAGE_BOOT | effect::DAMAGE_GRADED => {
                if equip_mask == 0x20 {
                    band(value, 0x17, 0x1E)
                } else {
                    step(value, 0x12)
                }
            }
            0x04 | effect::MAX_HP_RATE | effect::MAX_HP_RATE_ALT => step(value, 0x28),
            _ => None,
        }
    } else if matches!(equip_mask, 0x40 | 0x80 | 0xC0) {
        match id {
            effect::DAMAGE | effect::DAMAGE_BOOT | effect::DAMAGE_GRADED => band(value, 0x2C, 0x36),
            effect::MAGIC_ATTACK | effect::MAGIC_ATTACK_GRADED => band(value, 0x13, 0x18),
            effect::ATTACK_SPEED => band(value, 0x14, 0x18),
            0x4A => band(value, 0x14, 0x18),
            _ => None,
        }
    } else {
        None
    };

    color.unwrap_or(quality::PLAIN)
}

impl Effects<'_> {
    pub fn name_color(&self, item: &Item) -> u32 {
        let mask = self.total_scaled(item, effect::EQUIP_MASK) as u32;

        let mut best = quality::NEUTRAL;

        let mut best_id: Option<u8> = None;

        for row in ROWS {
            let id = match row.id {
                effect::MAX_HP_RATE => effect::MAX_HP_RATE_ALT,
                effect::MAX_MANA_RATE => effect::MAX_MANA_RATE_ALT,
                other => other,
            };

            let aliased = matches!(id, effect::CRITICAL | effect::DEFENSE_ALT)
                || (mask == 0x20 && id == effect::DAMAGE);

            let (scaled, raw) = if aliased {
                (self.total_scaled(item, id), self.total(item, id))
            } else {
                (self.slot_scaled(item, id), self.slot(item, id))
            };

            let color = quality_color(mask, id, raw);

            if best_id != Some(id) {
                if raw != 0 && quality::rank(best) < quality::rank(color) {
                    best = color;

                    best_id = Some(id);
                }
            } else {
                best = quality_color(mask, id, scaled);
            }
        }

        best
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Wearer {
    pub body: i32,
    pub level: i32,
    pub strength: i32,
    pub intelligence: i32,
    pub dexterity: i32,
    pub condition: i32,
    pub learned_skills: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Requirements {
    pub class: bool,
    pub level: bool,
    pub strength: bool,
    pub intelligence: bool,
    pub dexterity: bool,
    pub condition: bool,
}

impl Default for Requirements {
    fn default() -> Requirements {
        Requirements {
            class: true,
            level: true,
            strength: true,
            intelligence: true,
            dexterity: true,
            condition: true,
        }
    }
}

impl Requirements {
    pub fn by_row(&self, row: usize) -> Option<bool> {
        Some(match row {
            0 => self.class,
            1 => self.level,
            2 => self.strength,
            3 => self.intelligence,
            4 => self.dexterity,
            5 => self.condition,
            _ => return None,
        })
    }

    pub fn all_met(&self) -> bool {
        self.class
            && self.level
            && self.strength
            && self.intelligence
            && self.dexterity
            && self.condition
    }
}

impl Effects<'_> {
    pub fn requirements(&self, item: &Item, wearer: &Wearer) -> Requirements {
        let mut out = Requirements::default();

        let Some((_, entry)) = self.lookup(item) else {
            return out;
        };

        let sub = wearer.body.rem_euclid(10);

        let mask = self.total_scaled(item, effect::EQUIP_MASK);

        let class_mask = self.total_scaled(item, effect::CLASS);

        if class_mask >> (wearer.body.div_euclid(10) & 0x1F) & 1 == 0 {
            if sub < 6 || !matches!(mask, 0x40 | 0x80 | 0xC0) {
                out.class = false;
            }
        }

        let mut clear_class = false;

        match self.total_scaled(item, effect::RESTRICTION) {
            3 => {
                clear_class = sub < 6 || wearer.learned_skills & 0x4000_0000 == 0;
            }
            1 => {
                if sub > 5 {
                    return out;
                }

                clear_class = true;
            }
            2 => {
                if sub >= 6 {
                    out.class = false;

                    return out;
                }

                return self.stat_requirements(item, wearer, out, entry.grade);
            }
            _ => {
                if wearer.body < 0x20 && sub != 1 {
                    clear_class = mask == 2;
                }
            }
        }

        if clear_class {
            out.class = false;
        }

        if sub > 5 {
            return out;
        }

        self.stat_requirements(item, wearer, out, entry.grade)
    }

    fn stat_requirements(
        &self,
        item: &Item,
        wearer: &Wearer,
        mut out: Requirements,
        grade: i16,
    ) -> Requirements {
        out.level &= wearer.level >= self.total_scaled(item, effect::REQ_LEVEL);

        out.strength &= wearer.strength >= self.total_scaled(item, effect::REQ_STR);

        out.intelligence &= wearer.intelligence >= self.total_scaled(item, effect::REQ_INT);

        out.dexterity &= wearer.dexterity >= self.total_scaled(item, effect::REQ_DEX);

        out.condition &= wearer.condition >= self.total_scaled(item, effect::REQ_CON);

        let grade = i32::from(grade);

        if grade.rem_euclid(10) > 8 && grade < 0x28 {
            out.class = false;
        }

        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Trade {
    Free,
    Never,
    Bound,
    Once,
}

impl Trade {
    pub fn strdef(self) -> Option<u16> {
        match self {
            Trade::Free => None,
            Trade::Never => Some(1611),
            Trade::Bound => Some(1612),
            Trade::Once => Some(1613),
        }
    }

    pub fn code(self) -> i32 {
        match self {
            Trade::Free => 0,
            Trade::Never => 0x6F,
            Trade::Bound => 2,
            Trade::Once => 3,
        }
    }
}

impl Effects<'_> {
    pub fn trade(&self, item: &Item) -> Trade {
        let Some((id, entry)) = self.lookup(item) else {
            return Trade::Free;
        };

        let refine = refine_level(item, self.items) as i32;

        if self.total_scaled(item, effect::USE_KIND) == 0x10 && refine > 9 {
            return Trade::Never;
        }

        if entry.item_type == item_type::PET_STAGE2
            && (item.appearance != 0 || item.effects[1].id > 0xB4)
        {
            return Trade::Never;
        }

        if (is_bound_family(id) || is_extended_bound_family(id)) && refine < 11 {
            return Trade::Never;
        }

        if (0xF3C..4000).contains(&id) && item.effects[0].value == 0 && item.effects[1].value == 0 {
            return Trade::Never;
        }

        let mut found = Trade::Free;

        for e in &item.effects {
            if e.id != effect::TRADE || e.value == 0 {
                continue;
            }

            match e.value {
                3 => return Trade::Once,
                2 => return Trade::Bound,
                _ if entry.item_type == item_type::PET_STAGE2 => {}
                _ => found = Trade::Never,
            }
        }

        if found != Trade::Free {
            return found;
        }

        for e in entry.effect.iter() {
            if e.id != i16::from(effect::TRADE) || e.value == 0 {
                continue;
            }

            match e.value {
                3 => return Trade::Once,
                2 => return Trade::Bound,
                _ => found = Trade::Never,
            }
        }

        found
    }
}

pub fn is_bound_family(item_id: i32) -> bool {
    const WINDOWS: [(i32, i32); 5] = [
        (0x4CE, 0x4D1),
        (0x555, 0x558),
        (0x5EB, 0x5EE),
        (0x681, 0x684),
        (0x6B1, 0x6B1),
    ];

    WINDOWS.iter().any(|&(lo, hi)| (lo..=hi).contains(&item_id))
}

pub fn is_extended_bound_family(item_id: i32) -> bool {
    const WINDOWS: [(i32, i32); 9] = [
        (0xE15, 0xE18),
        (0xE29, 0xE2C),
        (0xE3D, 0xE40),
        (0xE51, 0xE54),
        (0xE65, 0xE68),
        (0xE79, 0xE7C),
        (0xE91, 0xE98),
        (0xEB5, 0xEB8),
        (0xEC9, 0xECC),
    ];

    WINDOWS.iter().any(|&(lo, hi)| (lo..=hi).contains(&item_id))
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TierBonus {
    pub piercing: i32,
    pub absorb: i32,
    pub drop_rate: i32,
    pub exp: i32,
}

pub fn tier_bonus(entry: &Entry, refine: u32, uncompressed: u32) -> TierBonus {
    let mut out = TierBonus::default();

    let stepped = i32::try_from(refine).unwrap_or(0);

    let stepped = if stepped < 9 { stepped } else { stepped + 1 } + 10;

    match entry.tier {
        5 => out.drop_rate = 8,
        6 => out.piercing = 40,
        7 => out.exp = 2,
        8 => out.absorb = 40,
        9 => out.piercing = stepped,
        10 => out.absorb = stepped,
        20 => out.piercing = stepped * 15 / 10,
        21 => out.absorb = stepped * 15 / 10,
        22 => {
            out.piercing = stepped * 15 / 10;

            out.absorb = out.piercing;
        }
        25 => out.piercing = stepped * 30 / 10,
        26 => {
            out.piercing = stepped * 30 / 10;

            out.absorb = out.piercing + 15;
        }
        _ => {}
    }

    let Some(over) = uncompressed.checked_sub(10) else {
        return out;
    };

    let over = i32::try_from(over).unwrap_or(0);

    let which = over % 4 + 1;

    let step = over / 4 + 1;

    let base = if entry.scale_pct == 0 {
        40
    } else {
        i32::from(entry.scale_pct)
    };

    match which {
        1 => out.drop_rate += step.min(1) * 8,
        3 => out.exp += step.min(1) * 2,
        2 | 4 => {
            let amount = base * ladder(step) / 100;

            if which == 2 {
                out.piercing += amount;
            } else {
                out.absorb += amount;
            }
        }
        _ => {}
    }

    out
}

fn ladder(step: i32) -> i32 {
    match step {
        2 => 200,
        3 => 300,
        4 => 450,
        5 => 600,
        6 => 800,
        _ => 100,
    }
}

pub fn stack_size(item: &Item) -> i32 {
    let value = i32::from(item.appearance as i16);

    if (1..=500).contains(&value) { value } else { 1 }
}
