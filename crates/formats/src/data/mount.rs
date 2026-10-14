use super::envelope::{self, Integrity};
use crate::error::{Kind, Result};

pub const RECORDS: usize = 500;

pub const DATA_RECORD_LEN: usize = 140;

pub const DATA_BODY_LEN: usize = RECORDS * DATA_RECORD_LEN;

pub const DATA_LEN: usize = DATA_BODY_LEN + 4;

pub const VISUAL_RECORD_LEN: usize = 28;

pub const VISUAL_BODY_LEN: usize = RECORDS * VISUAL_RECORD_LEN;

pub const VISUAL_LEN: usize = VISUAL_BODY_LEN + 4;

pub const ACCESSORY_BASE: usize = 100;

pub const GRID_COLS: usize = 10;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MountData {
    pub index: u32,
    pub c02: i32,
    pub c03: i32,
    pub c06: i32,
    pub c04: i32,
    pub max_maturity: i32,
    pub c07: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    pub evasion: i32,
    pub all_resistance: i32,
    pub c13: i32,
    pub c14: i32,
    pub c55: i32,
    pub grid: [[i16; GRID_COLS]; 4],
}

impl MountData {
    pub fn base_max_hp(&self) -> i32 {
        self.max_hp
    }

    pub fn accessory(&self, index: usize) -> Option<i16> {
        let k = index.checked_sub(ACCESSORY_BASE)?;

        self.grid[0].get(k).copied()
    }

    pub fn accessory_slot(&self, item_id: i16) -> Option<usize> {
        self.grid[0].iter().position(|&id| id == item_id)
    }

    pub fn accessory_available(&self, k: usize) -> bool {
        self.grid[3].get(k).is_some_and(|&v| v != 0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MountVisual {
    pub kind: i16,
    pub grid: [[i16; 3]; 3],
    pub scale: f32,
    pub visual_kind: i16,
    pub v14: i16,
}

impl MountVisual {
    pub fn is_populated(&self) -> bool {
        self.kind != 0
    }

    pub fn is_disabled(&self) -> bool {
        self.grid[2][0] == -1
    }
}

fn i32_at(rec: &[u8], off: usize) -> i32 {
    i32::from_le_bytes([rec[off], rec[off + 1], rec[off + 2], rec[off + 3]])
}

fn i16_at(rec: &[u8], off: usize) -> i16 {
    i16::from_le_bytes([rec[off], rec[off + 1]])
}

fn decode_data(rec: &[u8]) -> MountData {
    let mut grid = [[0i16; GRID_COLS]; 4];

    for (r, row) in grid.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            *cell = i16_at(rec, 0x3C + (r * GRID_COLS + c) * 2);
        }
    }

    MountData {
        index: i32_at(rec, 0x00) as u32,
        c02: i32_at(rec, 0x04),
        c03: i32_at(rec, 0x08),
        c06: i32_at(rec, 0x0C),
        c04: i32_at(rec, 0x10),
        max_maturity: i32_at(rec, 0x14),
        c07: i32_at(rec, 0x18),
        max_hp: i32_at(rec, 0x1C),
        atk: i32_at(rec, 0x20),
        def: i32_at(rec, 0x24),
        evasion: i32_at(rec, 0x28),
        all_resistance: i32_at(rec, 0x2C),
        c13: i32_at(rec, 0x30),
        c14: i32_at(rec, 0x34),
        c55: i32_at(rec, 0x38),
        grid,
    }
}

fn decode_visual(rec: &[u8]) -> MountVisual {
    let mut grid = [[0i16; 3]; 3];

    for (r, row) in grid.iter_mut().enumerate() {
        for (c, cell) in row.iter_mut().enumerate() {
            *cell = i16_at(rec, 0x02 + (r * 3 + c) * 2);
        }
    }

    MountVisual {
        kind: i16_at(rec, 0x00),
        grid,
        scale: f32::from_le_bytes(rec[0x14..0x18].try_into().expect("4 bytes")),
        visual_kind: i16_at(rec, 0x18),
        v14: i16_at(rec, 0x1A),
    }
}

pub fn parse_data(bytes: &[u8]) -> Result<Vec<MountData>> {
    let body = envelope::open(bytes, DATA_BODY_LEN, Integrity::TrailerB, Kind::MountData)?;

    Ok(body
        .chunks_exact(DATA_RECORD_LEN)
        .map(decode_data)
        .collect())
}

pub fn parse_visual(bytes: &[u8]) -> Result<Vec<MountVisual>> {
    let body = envelope::open(
        bytes,
        VISUAL_BODY_LEN,
        Integrity::TrailerB,
        Kind::MountDataV,
    )?;

    Ok(body
        .chunks_exact(VISUAL_RECORD_LEN)
        .map(decode_visual)
        .collect())
}

pub fn visual_for_mesh(table: &[MountVisual], mesh: i16) -> Option<&MountVisual> {
    usize::try_from(mesh).ok().and_then(|i| table.get(i))
}

pub const BASE_MAX_HP_FALLBACK: i32 = 7000;

pub const MATURITY_SCALE_MIN: i32 = 0x82;

pub const MATURITY_SCALE_CAP: i32 = 64_000;

pub const MATURITY_SCALE_DEFAULT_PCT: i32 = 10;

pub const MATURITY_SCALE_PCT: [(i32, i32); 7] = [
    (0x0E, 20),
    (0x0F, 30),
    (0x10, 40),
    (0x11, 50),
    (0x12, 65),
    (0x13, 80),
    (0x14, 100),
];

pub fn maturity_scale(base: i32, maturity: i32) -> i32 {
    if maturity < MATURITY_SCALE_MIN {
        return base;
    }

    let decade = maturity / 10;

    let pct = MATURITY_SCALE_PCT
        .iter()
        .find(|&&(d, _)| d == decade)
        .map_or(MATURITY_SCALE_DEFAULT_PCT, |&(_, pct)| pct);

    let scaled = base.wrapping_add(pct.wrapping_mul(base) / 100);

    scaled.min(MATURITY_SCALE_CAP)
}

pub type FeedGroup = (&'static [(i16, i16)], &'static [i16]);

pub const FEED_TABLE: [FeedGroup; 18] = [
    (&[(0x91A, 0x91A), (0x938, 0x938)], &[0x974]),
    (&[(0x91B, 0x91B), (0x939, 0x939)], &[0x975, 0xD28]),
    (&[(0x91C, 0x91C), (0x93A, 0x93A)], &[0x976, 0xD29]),
    (&[(0x91D, 0x91D), (0x93B, 0x93B)], &[0x977, 0xD2A]),
    (&[(0x91E, 0x91E), (0x93C, 0x93C)], &[0x978, 0xD2B]),
    (&[(0x91F, 0x91F), (0x93D, 0x93D)], &[0x979, 0xD2C]),
    (
        &[(0x920, 0x929), (0x93E, 0x947), (0xB90, 0xB91)],
        &[0x97A, 0xD2D],
    ),
    (&[(0x92A, 0x92A), (0x948, 0x948)], &[0x984, 0xD37]),
    (&[(0x92B, 0x92B), (0x949, 0x949)], &[0x985, 0xD38]),
    (&[(0x92D, 0x92D), (0x94B, 0x94B)], &[0x97B, 0xD2E]),
    (&[(0x92E, 0x92E), (0x94C, 0x94C)], &[0x97C, 0xD2F]),
    (&[(0x92F, 0x931), (0x94D, 0x94F)], &[0x97D, 0xD30]),
    (&[(0x932, 0x934), (0x950, 0x952)], &[0x97E, 0xD31]),
    (&[(0x935, 0x935), (0x953, 0x953)], &[0x97A, 0xD2D]),
    (&[(0x936, 0x936), (0x954, 0x954)], &[0x97D, 0xD30]),
    (&[(0x94A, 0x94A), (0x92C, 0x92C)], &[0xD89, 0x986]),
    (&[(0x955, 0x955), (0x937, 0x937)], &[0xD8A, 0x987]),
    (&[(0xB93, 0xB93), (0xB94, 0xB94)], &[0x173F, 0x1740]),
];

pub const FEED_UNREADABLE_MOUNTS: [i16; 2] = [0xB90, 0xB91];

pub const FEED_ITEM_KIND: i32 = 0x0F;

pub fn feeds(mount_id: i16, food_id: i16) -> bool {
    FEED_TABLE.iter().any(|(mounts, foods)| {
        mounts
            .iter()
            .any(|&(low, high)| (low..=high).contains(&mount_id))
            && foods.contains(&food_id)
    })
}
