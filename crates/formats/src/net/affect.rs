use crate::error::{Kind, Result};

const KIND: Kind = Kind::Affect;

pub const AFFECTS: usize = 32;

pub const DETAIL_LEN: usize = 8;

pub const AFFECT_LEN: usize = 2;

pub const NO_DETAILS: [Detail; AFFECTS] = [Detail {
    kind: 0,
    level: 0,
    value: 0,
    time_8s: 0,
}; AFFECTS];

pub const NO_AFFECTS: [Affect; AFFECTS] = [Affect {
    time_8s: 0,
    kind: 0,
}; AFFECTS];

pub const DETAIL_OFF: usize = 0;

pub const PAYLOAD_LEN: usize = AFFECTS * DETAIL_LEN;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]

pub struct Affect {
    pub time_8s: u8,
    pub kind: u8,
}

impl Affect {
    pub fn at(run: &[u8], slot: usize) -> Affect {
        let off = slot * AFFECT_LEN;

        Affect {
            time_8s: run.get(off).copied().unwrap_or(0),
            kind: run.get(off + 1).copied().unwrap_or(0),
        }
    }

    pub fn all(run: &[u8]) -> [Affect; AFFECTS] {
        core::array::from_fn(|slot| Affect::at(run, slot))
    }

    pub fn is_set(&self) -> bool {
        self.kind != 0
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]

pub struct Detail {
    pub kind: u8,
    pub level: u8,
    pub value: u16,
    pub time_8s: i32,
}

impl Detail {
    pub fn is_set(&self) -> bool {
        self.kind != 0
    }

    pub fn bit(&self, bit: u32) -> bool {
        self.value as u8 & (1 << bit) != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Affects {
    pub details: [Detail; AFFECTS],
    pub tail: Vec<u8>,
}

impl Default for Affects {
    fn default() -> Affects {
        Affects {
            details: [Detail::default(); AFFECTS],
            tail: Vec::new(),
        }
    }
}

impl Affects {
    pub fn apply(&self, into: &mut [Detail; AFFECTS]) {
        *into = self.details;
    }

    pub fn apply_hide_flags(&self, flags: &mut Flags) {
        flags.set(Flags::HIDE_A, 0);

        flags.set(Flags::HIDE_D, 0);

        for detail in self.details.iter().filter(|d| d.kind == Flags::HIDE_KIND) {
            for (bit, off) in Flags::HIDE_BITS {
                if detail.bit(bit) {
                    flags.set(off, 1);
                }
            }
        }
    }
}

pub fn parse(payload: &[u8]) -> Result<Affects> {
    let mut details = [Detail::default(); AFFECTS];

    for (slot, detail) in details.iter_mut().enumerate() {
        let off = DETAIL_OFF + slot * DETAIL_LEN;

        *detail = Detail {
            kind: crate::util::byte_at(payload, off, KIND)?,
            level: crate::util::byte_at(payload, off + 1, KIND)?,
            value: crate::util::u16_at(payload, off + 2, KIND)?,
            time_8s: crate::util::u32_at(payload, off + 4, KIND)? as i32,
        };
    }

    Ok(Affects {
        details,
        tail: payload[PAYLOAD_LEN.min(payload.len())..].to_vec(),
    })
}

impl Affects {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; PAYLOAD_LEN];

        for (slot, detail) in self.details.iter().enumerate() {
            let off = DETAIL_OFF + slot * DETAIL_LEN;

            out[off] = detail.kind;

            out[off + 1] = detail.level;

            out[off + 2..off + 4].copy_from_slice(&detail.value.to_le_bytes());

            out[off + 4..off + 8].copy_from_slice(&detail.time_8s.to_le_bytes());
        }

        out.extend_from_slice(&self.tail);

        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Flags {
    raw: [u8; Flags::LEN],
}

impl Default for Flags {
    fn default() -> Flags {
        Flags {
            raw: [0; Flags::LEN],
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]

pub struct Wearer {
    pub class: i32,
    pub part0_mesh: u16,
}

impl Flags {
    pub const BASE: usize = 0xB2C;

    pub const LAST: usize = 0xB5A;

    pub const LEN: usize = Flags::LAST - Flags::BASE + 1;

    pub const WEAPON_GLOW: usize = 0xB2E;

    pub const AURA: usize = 0xB34;

    pub const AURA_SUPPRESS: usize = 0xB35;

    pub const TRAIL: usize = 0xB38;

    pub const INVISIBLE: usize = 0xB3E;

    pub const STICKY: usize = 0xB3F;

    pub const WEAPON_EFFECT: usize = 0xB41;

    pub const WEAPON_SPARK: usize = 0xB54;

    pub const HIDE_A: usize = 0xB4C;

    pub const HIDE_B: usize = 0xB4D;

    pub const HIDE_C: usize = 0xB4F;

    pub const HIDE_D: usize = 0xB50;

    pub const HIDE_KIND: u8 = 8;

    pub const HIDE_BITS: [(u32, usize); 4] = [
        (0, Flags::HIDE_A),
        (1, Flags::HIDE_B),
        (3, Flags::HIDE_C),
        (4, Flags::HIDE_D),
    ];

    pub const PAIR: usize = 0xB45;

    pub const WORD: usize = 0xB48;

    pub const TABLE: [(u8, usize); 29] = [
        (0x01, 0xB2F),
        (0x02, 0xB2D),
        (0x03, 0xB31),
        (0x04, 0xB32),
        (0x06, 0xB44),
        (0x07, 0xB35),
        (0x09, 0xB38),
        (0x0A, 0xB3C),
        (0x0B, 0xB36),
        (0x0D, 0xB2E),
        (0x0F, 0xB3A),
        (0x11, 0xB37),
        (0x12, 0xB53),
        (0x13, 0xB52),
        (0x14, 0xB2C),
        (0x15, 0xB54),
        (0x16, 0xB3B),
        (0x17, 0xB57),
        (0x18, 0xB58),
        (0x19, 0xB3D),
        (0x1A, 0xB3F),
        (0x1B, 0xB41),
        (0x1C, 0xB3E),
        (0x1D, 0xB5A),
        (0x1E, 0xB42),
        (0x1F, 0xB56),
        (0x20, 0xB39),
        (0x25, 0xB59),
        (0x27, 0xB43),
    ];

    pub const CLEARED: [(usize, usize); 15] = [
        (0xB2C, 4),
        (0xB31, 2),
        (0xB34, 4),
        (0xB38, 4),
        (0xB3C, 2),
        (0xB3E, 1),
        (0xB40, 2),
        (0xB42, 2),
        (0xB51, 1),
        (0xB52, 2),
        (0xB54, 1),
        (0xB56, 4),
        (0xB48, 4),
        (0xB45, 2),
        (0xB44, 1),
    ];

    pub const STICKY_CLASS: i32 = 0x22;

    pub const STICKY_CLASS_ALT: i32 = 0x15;

    pub const STICKY_MESH_ALT: u16 = 10;

    pub fn build(
        affects: &[Affect; AFFECTS],
        details: &[Detail; AFFECTS],
        wearer: &Wearer,
    ) -> Flags {
        let mut flags = Flags::default();

        flags.rebuild(affects, details, wearer);

        flags
    }

    pub fn get(&self, offset: usize) -> u8 {
        self.raw[offset - Flags::BASE]
    }

    pub fn set(&mut self, offset: usize, value: u8) {
        self.raw[offset - Flags::BASE] = value;
    }

    pub fn raw(&self) -> &[u8; Flags::LEN] {
        &self.raw
    }

    pub fn invisible(&self) -> bool {
        self.get(Flags::INVISIBLE) == 1
    }

    pub fn rebuild(
        &mut self,
        affects: &[Affect; AFFECTS],
        details: &[Detail; AFFECTS],
        wearer: &Wearer,
    ) {
        for (offset, len) in Flags::CLEARED {
            for byte in 0..len {
                self.set(offset + byte, 0);
            }
        }

        let sticks = wearer.class == Flags::STICKY_CLASS
            || (wearer.class == Flags::STICKY_CLASS_ALT
                && wearer.part0_mesh == Flags::STICKY_MESH_ALT);

        if !sticks {
            self.set(Flags::STICKY, 0);
        }

        for (slot, affect) in affects.iter().enumerate() {
            match affect.kind {
                0x28 | 0x2D => {
                    self.set(Flags::PAIR, 1);

                    self.set(Flags::PAIR + 1, 1);

                    self.set_word(Flags::WORD, 1);
                }
                0x05 => self.set_word(Flags::WORD, 1),
                Flags::HIDE_KIND => {
                    if details[slot].bit(0) {
                        self.set(Flags::HIDE_A, 1);
                    }
                }
                0x2C => self.set(Flags::PAIR, 1),
                kind => {
                    if let Some((_, offset)) = Flags::TABLE.iter().find(|(k, _)| *k == kind) {
                        self.set(*offset, 1);
                    }
                }
            }
        }
    }

    fn set_word(&mut self, offset: usize, value: i32) {
        let bytes = value.to_le_bytes();

        for (byte, &value) in bytes.iter().enumerate() {
            self.set(offset + byte, value);
        }
    }
}

pub const AFFECT_SOURCE_SKILL: [i32; 55] = [
    -1, 1, 41, 16, 165, 119, 105, 19, 161, 44, 51, 43, 84, 11, 3, 45, 71, 5, 46, 76, 40, 77, 37,
    54, 13, 53, 89, 75, 95, 102, 163, 85, 47, 96, 164, 125, 92, 81, 87, 162, 96, 26, 121, 129, 99,
    114, 130, 147, 140, 169, 177, 178, 179, 180, 0,
];

pub const SOURCE_SKILL_REAL: usize = 54;

pub const SOURCE_SKILL_MAX_KIND: u8 = 0x36;

pub const NO_SOURCE_SKILL_MIN: i32 = 0x99;

pub const ICON_ONLY_KINDS: [u8; 10] = [4, 8, 30, 34, 39, 49, 50, 51, 52, 53];

pub const NO_AFFECT_SKILL: u8 = 0x09;

pub const MORPH_FAMILY: [u8; 5] = [0x40, 0x42, 0x44, 0x46, 0x47];

pub const MORPH_CODE: u8 = 0x47;

pub fn source_skill(kind: u8) -> Option<i32> {
    AFFECT_SOURCE_SKILL.get(usize::from(kind)).copied()
}

pub fn source_matches(kind: u8, code: u8) -> bool {
    let want = if MORPH_FAMILY.contains(&code) {
        MORPH_CODE
    } else {
        code
    };

    source_skill(kind) == Some(i32::from(want))
}

pub const ICON_SHEET: &str = "UI/NewAmul.wyt";

pub const ICON_SHEET_ON: &str = "UI/NewAmulOn.wyt";

pub const ICON_SIZE: i32 = 0x20;

pub const ICON_COLUMNS: i32 = 16;

pub fn remap(n: i32) -> i32 {
    match n {
        0x69..=0x98 => n + 0x17,
        0x99..=0xA8 => n - 0x29,
        n => n,
    }
}

pub fn cell_of(n: i32) -> [i32; 4] {
    let column = (n % ICON_COLUMNS).wrapping_mul(ICON_SIZE);

    let row = (n / ICON_COLUMNS).wrapping_mul(ICON_SIZE);

    [
        column,
        row,
        column.wrapping_add(ICON_SIZE),
        row.wrapping_add(ICON_SIZE),
    ]
}

pub fn icon_number(kind: u8) -> Option<i32> {
    Some(remap(source_skill(kind)?))
}

pub fn icon_cell(kind: u8) -> Option<[i32; 4]> {
    Some(cell_of(icon_number(kind)?))
}

pub fn countdown(seconds: i32) -> String {
    match seconds {
        s if s < 1 => EXPIRED_CAPTION.to_string(),
        s if s < 601 => format!("{s}"),
        s if s < 3601 => format!("{}M", s / 60),
        s if s < 86401 => format!("{}H", s / 3600),
        s => format!("{}dD", s / 86400),
    }
}

pub const EXPIRED_CAPTION: &str = "0";

pub const COUNTDOWN_COLOURS: [u32; 7] = [
    0xFF00_0000,
    0xFFFF_00FF,
    0xFFFF_AA00,
    0xFFFF_FF00,
    0xFF00_FFFF,
    0xFFAA_AAFF,
    0xFFFF_FFFF,
];

pub const COUNTDOWN_THRESHOLDS: [i32; 7] = [120, 240, 360, 3360, 3656, 5936, 7375];

pub fn countdown_colour(slot: usize, seconds: i32) -> u32 {
    if slot != 0 {
        return 0xFFFF_FFFF;
    }

    let seat = COUNTDOWN_THRESHOLDS
        .iter()
        .position(|&limit| seconds < limit);

    seat.map_or(0xFFFF_FFFF, |seat| COUNTDOWN_COLOURS[seat])
}

pub const BLINK_SECONDS: i32 = 11;

pub const BLINK_PERIOD_MS: u32 = 200;

pub const BLINK_COLOUR: u32 = 0x0000_0000;

pub const DISABLED_COLOUR: u32 = 0x44FF_FFFF;

pub const ENABLED_COLOUR: u32 = 0xFFFF_FFFF;

pub fn blinking_off(seconds: i32, now_ms: u32) -> bool {
    seconds < BLINK_SECONDS && (now_ms / BLINK_PERIOD_MS) & 1 == 1
}

pub fn seconds_left(detail: &Detail, now_ms: u32, stamped_ms: u32) -> i32 {
    let elapsed = now_ms.wrapping_sub(stamped_ms) / 1000;

    detail
        .time_8s
        .saturating_mul(8)
        .saturating_sub(elapsed as i32)
}
