use super::envelope::{self, Integrity};
use crate::error::{Kind, Result};

pub const RECORDS: usize = 248;

pub const RECORD_LEN: usize = 0x68;

pub const BODY_LEN: usize = RECORDS * RECORD_LEN;

pub const LEN: usize = BODY_LEN + 4;

pub const CHECKSUM: i32 = 0x001A_45C7;

pub const ANIM_A: usize = 0x30;

pub const ANIM_B: usize = 0x38;

pub const ANIM_COLUMN_LEN: usize = 6;

pub const CH02_MODEL: u32 = 1;

pub const ANIM_PADDING: [usize; 4] = [0x36, 0x37, 0x3E, 0x3F];

pub const fn anim_base(model: u32, mounted: bool) -> usize {
    let column = if model == CH02_MODEL { ANIM_B } else { ANIM_A };

    column + if mounted { 3 } else { 0 }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct Skill {
    pub skill_points: i32,
    pub target_shape: i32,
    pub cost: i32,
    pub cooldown_secs: i32,
    pub range: i32,
    pub damage_kind: i32,
    pub base_damage: i32,
    pub c09: i32,
    pub c10: i32,
    pub c11: i32,
    pub damage_coefficient: i32,
    pub recast_coefficient: i32,
    pub anim_a: [u8; 6],
    pub anim_b: [u8; 6],
    pub c16: i32,
    pub c17: i32,
    pub offensive: i32,
    pub max_targets: i32,
    pub c20: i32,
    pub c21: i32,
    pub passive: i32,
    pub c23: i32,
    pub id: i32,
    pub c24: i32,
}

impl Skill {
    pub fn is_populated(&self) -> bool {
        self.id != 0
    }

    pub fn anim_window(&self, model: u32, mounted: bool) -> [u8; 4] {
        let column = if model == CH02_MODEL {
            &self.anim_b
        } else {
            &self.anim_a
        };

        let start = if mounted { 3 } else { 0 };

        let mut out = [0u8; 4];

        for (slot, byte) in out.iter_mut().enumerate() {
            *byte = column.get(start + slot).copied().unwrap_or(0);
        }

        out
    }

    pub fn is_selectable(&self) -> bool {
        self.target_shape == 0 || self.target_shape == 2
    }

    pub fn cost_at(&self, level: i32, reduce_pct: i32) -> i32 {
        let scaled = (level / 2 + 100).wrapping_mul(self.cost) / 100;

        scaled.wrapping_mul(100 - reduce_pct) / 100
    }

    pub fn range(&self) -> i32 {
        self.range
    }

    pub fn recast_coeff(&self) -> i32 {
        self.recast_coefficient
    }

    pub fn recast_ms(&self, mastery: i32) -> i32 {
        let mastery = mastery.min(MASTERY_CAP);

        self.recast_coefficient
            .wrapping_mul(mastery)
            .wrapping_mul(100)
    }

    pub fn max_targets(&self) -> i32 {
        self.max_targets
    }

    pub fn damage(&self) -> i32 {
        self.base_damage
    }

    pub fn property(&self) -> i32 {
        self.c16.max(self.c17)
    }

    pub fn is_passive(&self) -> bool {
        self.passive == 1
    }

    pub fn skill_points(&self) -> i32 {
        self.skill_points
    }
}

pub const MASTERY_CAP: i32 = 0x140;

pub const DIRECT_CODE_MAX: u8 = 0x68;

pub const SECOND_WINDOW_DELTA: usize = 0x5F;

pub fn wire_skill_row(code: u8) -> usize {
    let code = usize::from(code);

    if code > usize::from(DIRECT_CODE_MAX) {
        code + SECOND_WINDOW_DELTA
    } else {
        code
    }
}

pub const SCROLL_IDS: std::ops::RangeInclusive<i32> = 0x29A..=0x29F;

pub const SCROLL_SKILL_BASE: i32 = 0x23A;

pub fn scroll_skill(id: i32) -> Option<usize> {
    SCROLL_IDS
        .contains(&id)
        .then(|| (id - SCROLL_SKILL_BASE) as usize)
}

pub const PROPERTIES: [usize; 6] = [116, 117, 118, 119, 120, 481];

pub const POISON_SKILL: i32 = 0x5C;

pub fn is_skill_book(id: i32) -> bool {
    (id as u32).wrapping_sub(5000) < 0x69 || (id as u32).wrapping_sub(0x1518) <= 0x2F
}

pub fn skill_index(id: i32) -> i32 {
    if id >= 0x1518 {
        return id - 0x1450;
    }

    if id >= 5000 { id - 5000 } else { id }
}

pub const GROUPS: [usize; 12] = [107, 108, 109, 110, 111, 112, 113, 114, 115, 133, 134, 135];

pub fn skill_group(id: i32) -> Option<usize> {
    let index = if id >= 0x1518 {
        (id - 0x1518) / 4
    } else {
        skill_group_by_eight_raw(id)
    };

    usize::try_from(index).ok().filter(|&i| i < GROUPS.len())
}

pub fn skill_group_by_eight(id: i32) -> Option<usize> {
    usize::try_from(skill_group_by_eight_raw(id))
        .ok()
        .filter(|&i| i < GROUPS.len())
}

fn skill_group_by_eight_raw(id: i32) -> i32 {
    (id - 5000) / 8
}

#[derive(Debug, Clone)]

pub struct SkillData {
    skills: Vec<Skill>,
}

fn i32_at(rec: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([rec[off], rec[off + 1], rec[off + 2], rec[off + 3]])
}

fn decode(rec: &[u8]) -> Skill {
    Skill {
        skill_points: i32_at(rec, 0x00),
        target_shape: i32_at(rec, 0x04),
        cost: i32_at(rec, 0x08),
        cooldown_secs: i32_at(rec, 0x0C),
        range: i32_at(rec, 0x10),
        damage_kind: i32_at(rec, 0x14),
        base_damage: i32_at(rec, 0x18),
        c09: i32_at(rec, 0x1C),
        c10: i32_at(rec, 0x20),
        c11: i32_at(rec, 0x24),
        damage_coefficient: i32_at(rec, 0x28),
        recast_coefficient: i32_at(rec, 0x2C),
        anim_a: rec[ANIM_A..ANIM_A + ANIM_COLUMN_LEN]
            .try_into()
            .expect("6 bytes"),
        anim_b: rec[ANIM_B..ANIM_B + ANIM_COLUMN_LEN]
            .try_into()
            .expect("6 bytes"),
        c16: i32_at(rec, 0x40),
        c17: i32_at(rec, 0x44),
        offensive: i32_at(rec, 0x48),
        max_targets: i32_at(rec, 0x4C),
        c20: i32_at(rec, 0x50),
        c21: i32_at(rec, 0x54),
        passive: i32_at(rec, 0x58),
        c23: i32_at(rec, 0x5C),
        id: i32_at(rec, 0x60),
        c24: i32_at(rec, 0x64),
    }
}

pub fn parse(bytes: &[u8]) -> Result<SkillData> {
    let body = envelope::open(
        bytes,
        BODY_LEN,
        Integrity::ConstantA(CHECKSUM),
        Kind::SkillData,
    )?;

    Ok(SkillData {
        skills: body.chunks_exact(RECORD_LEN).map(decode).collect(),
    })
}

impl SkillData {
    pub fn get(&self, id: usize) -> Option<&Skill> {
        self.skills.get(id)
    }

    pub fn for_wire_code(&self, code: u8) -> Option<&Skill> {
        self.get(wire_skill_row(code))
    }

    pub fn len(&self) -> usize {
        self.skills.len()
    }

    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &Skill)> {
        self.skills.iter().enumerate()
    }

    pub fn populated(&self) -> usize {
        self.skills.iter().filter(|s| s.is_populated()).count()
    }

    pub fn from_plain(body: Vec<u8>) -> SkillData {
        assert_eq!(
            body.len(),
            BODY_LEN,
            "a hand-built table is still the full array"
        );

        SkillData {
            skills: body.chunks_exact(RECORD_LEN).map(decode).collect(),
        }
    }

    pub fn misnumbered(&self) -> Vec<usize> {
        self.skills
            .iter()
            .enumerate()
            .filter(|(i, s)| s.is_populated() && s.id != *i as i32 + 1)
            .map(|(i, _)| i)
            .collect()
    }
}
