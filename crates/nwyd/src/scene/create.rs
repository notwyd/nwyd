use crate::ui::PanelUi;
use anyhow::Result;
use assets::GameDir;
use formats::text::codepage::{self, Codepage};
use formats::ui::pane::ElementType;
use ui::{Strings, text};

pub const CLASS_STATS: [[i32; 6]; 4] = [
    [12, 4, 7, 13, 80, 45],
    [5, 15, 5, 11, 60, 65],
    [11, 11, 9, 5, 70, 55],
    [8, 9, 13, 6, 75, 60],
];

pub const STATS_SHOWN: usize = 4;

pub const GAUGE_SCALE: i32 = 10;

pub const PANE_SCALE: [f32; 4] = [1.8, 1.6, 0.8, 0.6];

pub const CLASS_STRDEF: [usize; 4] = super::sel_info::CLASS_STRDEF;

pub const DESC_STEMS: [&str; 4] = [
    "chardesctrans",
    "chardescfoema",
    "chardescbeast",
    "chardeschunter",
];

pub const ELEMENT_CLASS: (&str, ElementType) = ("Class", ElementType::Static);

pub const ELEMENT_DESC: (&str, ElementType) = ("Desc", ElementType::Static);

pub const ELEMENT_STAT: (&str, ElementType) = ("Stat%d", ElementType::Static);

pub const ELEMENT_STATI: (&str, ElementType) = ("Stati%d", ElementType::Image);

pub const PANE_EFFECTS: [u32; 4] = [0x61A, 0x61B, 0x61C, 0x61D];

pub const PICK_ACTION: [usize; 4] = [5, 9, 8, 8];

pub const PICK_SOUND: i64 = 0x39;

pub const HUNTER_SOUND: i64 = 0x86;

pub const HUNTER_DOLL: usize = 3;

#[derive(Debug, Clone, PartialEq)]

pub struct Pane {
    pub class: String,
    pub desc: String,
    pub stat: [String; 4],
    pub gauge_width: [i32; 4],
    pub gauge_height: GaugeHeight,
    pub scale: f32,
    pub action: usize,
    pub sounds: Vec<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct GaugeHeight(pub i32);

pub fn fill(
    doll: usize,
    descriptions: &[String; 4],
    stati0_height: GaugeHeight,
    strings: &Strings,
) -> Option<Pane> {
    let row = CLASS_STATS.get(doll)?;

    Some(Pane {
        class: strings.translate(&format!("ss_{}", CLASS_STRDEF[doll])),
        desc: descriptions[doll].clone(),
        stat: std::array::from_fn(|i| row[i].to_string()),
        gauge_width: std::array::from_fn(|i| row[i] * GAUGE_SCALE),
        gauge_height: stati0_height,
        scale: PANE_SCALE[doll],
        action: PICK_ACTION[doll],
        sounds: match doll == HUNTER_DOLL {
            true => vec![PICK_SOUND, HUNTER_SOUND],
            false => vec![PICK_SOUND],
        },
    })
}

pub fn descriptions(game: &GameDir, _codepage: Codepage) -> Result<[String; 4]> {
    let mut out = std::array::from_fn(|_| String::new());

    for (slot, stem) in DESC_STEMS.iter().enumerate() {
        if let Ok((bytes, codepage)) = crate::ui::read_language(game, &format!("{stem}.txt")) {
            out[slot] = text::decode_colored_text_records(&bytes, codepage);
        }
    }

    Ok(out)
}

pub fn gauge_height(ui: &PanelUi) -> GaugeHeight {
    let (name, kind) = ELEMENT_STATI;

    let height = ui
        .panel
        .element(&name.replace("%d", "0"), kind)
        .map(|e| e.height)
        .unwrap_or_default();

    GaugeHeight(height)
}

pub fn bind(ui: &mut PanelUi) {
    let PanelUi {
        panel, resource, ..
    } = ui;

    for (name, kind) in [ELEMENT_CLASS, ELEMENT_DESC] {
        panel.element_or_create(name, kind, resource);
    }

    for i in 0..4 {
        for (name, kind) in [ELEMENT_STAT, ELEMENT_STATI] {
            panel.element_or_create(&name.replace("%d", &i.to_string()), kind, resource);
        }
    }
}

pub fn apply(pane: &Pane, ui: &mut PanelUi) {
    let PanelUi {
        panel, resource, ..
    } = ui;

    panel
        .element_or_create(ELEMENT_CLASS.0, ELEMENT_CLASS.1, resource)
        .set_display(&pane.class);

    panel
        .element_or_create(ELEMENT_DESC.0, ELEMENT_DESC.1, resource)
        .set_display(&pane.desc);

    for i in 0..STATS_SHOWN {
        let stat = ELEMENT_STAT.0.replace("%d", &i.to_string());

        panel
            .element_or_create(&stat, ELEMENT_STAT.1, resource)
            .set_display(&pane.stat[i]);

        let gauge = ELEMENT_STATI.0.replace("%d", &i.to_string());

        let element = panel.element_or_create(&gauge, ELEMENT_STATI.1, resource);

        element.width = pane.gauge_width[i];

        element.height = pane.gauge_height.0;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum NameReject {
    TooShort,
    TooLong,
    Invalid,
}

impl NameReject {
    pub fn strdef(self) -> usize {
        match self {
            NameReject::TooShort => 15,
            NameReject::TooLong => 16,
            NameReject::Invalid => 17,
        }
    }
}

pub const NAME_MIN: usize = 4;

pub const NAME_MAX: usize = 12;

pub const VALIDATOR_LEN: std::ops::RangeInclusive<usize> = 4..=15;

pub fn charset_ok(name: &[u8]) -> bool {
    if !VALIDATOR_LEN.contains(&name.len()) {
        return false;
    }

    let mut i = 0;

    while i < name.len() {
        let c = name[i];

        if c & 0x80 != 0 {
            i += 1;

            match name.get(i) {
                None | Some(0) => return false,
                Some(_) => {}
            }
        } else if !c.is_ascii_alphanumeric() && c != b'-' {
            return false;
        }

        i += 1;
    }

    true
}

pub fn printable_ok(name: &[u8], cp: Codepage) -> bool {
    if !VALIDATOR_LEN.contains(&name.len()) {
        return false;
    }

    codepage::decode(name, cp)
        .chars()
        .all(|c| c as u32 >= 0x21 && !(0x80..=0xA0).contains(&(c as u32)))
}

pub fn filter(name: &mut [u8]) -> bool {
    for byte in name.iter_mut() {
        if *byte & 0x80 != 0 {
            *byte = b' ';
        } else if !byte.is_ascii_alphanumeric() && *byte != b'-' {
            return false;
        }
    }

    true
}

pub fn contains_reserved_name_marker(name: &[u8]) -> bool {
    let limit = name.len().saturating_sub(2);

    (0..limit).any(|i| name[i] == 0xA1 && name[i + 1] == 0xA1)
}

pub const RESERVED_FILE: &[u8] = b"curse.bin";

pub const RESERVED_LEN: usize = 0x2000;

pub const RESERVED_STRIDE: usize = 0x20;

pub const RESERVED_REPLACEMENT: usize = 0x10;

#[derive(Debug, Clone, Default, PartialEq, Eq)]

pub struct Reserved {
    pub rows: Vec<(Vec<u8>, Vec<u8>)>,
}

impl Reserved {
    pub fn parse(bytes: &[u8]) -> Reserved {
        let mut rows = Vec::new();

        for record in bytes
            .chunks_exact(RESERVED_STRIDE)
            .take(RESERVED_LEN / RESERVED_STRIDE)
        {
            let field = |from: usize| -> Vec<u8> {
                record[from..from + RESERVED_REPLACEMENT]
                    .iter()
                    .map(|b| !b)
                    .take_while(|b| *b != 0)
                    .collect()
            };

            let word = field(0);

            if word.is_empty() {
                break;
            }

            rows.push((word, field(RESERVED_REPLACEMENT)));
        }

        Reserved { rows }
    }

    pub fn load(game: &GameDir) -> Reserved {
        match game.read(RESERVED_FILE) {
            Ok(bytes) => Reserved::parse(&bytes),
            Err(_) => Reserved::default(),
        }
    }

    pub fn substitute(&self, name: &mut Vec<u8>, codepage: Codepage) {
        for _ in 0..RESCAN_LIMIT {
            let hit = self.rows.iter().find_map(|(word, replacement)| {
                let at = find(name, word)?;

                on_character_boundary(name, at, codepage).then_some((at, word, replacement))
            });

            let Some((at, word, replacement)) = hit else {
                return;
            };

            name.splice(at..at + word.len(), replacement.iter().copied());
        }
    }
}

pub const RESCAN_LIMIT: usize = 64;

fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    match needle.is_empty() {
        true => Some(0),
        false => haystack.windows(needle.len()).position(|w| w == needle),
    }
}

pub fn on_character_boundary(name: &[u8], at: usize, codepage: Codepage) -> bool {
    let lead = |b: u8| match codepage {
        Codepage::Windows949
        | Codepage::Windows950
        | Codepage::Windows936
        | Codepage::Unsupported(949 | 950 | 936) => (0x81..=0xFE).contains(&b),
        _ => false,
    };

    let next = match name.get(at) {
        Some(&b) if lead(b) && at + 1 < name.len() => 2,
        Some(_) => 1,
        None => return true,
    };

    let end = at + next;

    let mut i = 0;

    let mut begin = 0;

    while i < end {
        begin = i;

        i += match lead(name[i]) && i + 1 < name.len() {
            true => 2,
            false => 1,
        };
    }

    let prev = end - begin;

    !(next == 1 && prev == 2) && !(next == 2 && prev == 1)
}

pub fn check_name(
    name: &[u8],
    codepage: Codepage,
    reserved: &Reserved,
) -> Result<Vec<u8>, NameReject> {
    if name.len() < NAME_MIN {
        return Err(NameReject::TooShort);
    }

    if name.len() > NAME_MAX {
        return Err(NameReject::TooLong);
    }

    if !charset_ok(name) || !printable_ok(name, codepage) {
        return Err(NameReject::Invalid);
    }

    let mut filtered = name.to_vec();

    if !filter(&mut filtered) {
        return Err(NameReject::Invalid);
    }

    reserved.substitute(&mut filtered, codepage);

    if contains_reserved_name_marker(&filtered) {
        return Err(NameReject::Invalid);
    }

    Ok(filtered)
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Pressed {
    Send(Box<[u8; CREATE_LEN]>),
    BadName(NameReject),
    NoClass,
    NoSlot,
}

pub const NO_CLASS_STRDEF: usize = 14;

pub const SLOTS: usize = 4;

pub fn pressed(
    typed: &str,
    doll: Option<usize>,
    free_slot: Option<usize>,
    codepage: Codepage,
    reserved: &Reserved,
) -> Pressed {
    let name = match check_name(&codepage::encode(typed, codepage), codepage, reserved) {
        Ok(name) => name,
        Err(reject) => return Pressed::BadName(reject),
    };

    let Some(doll) = doll.filter(|d| *d < CLASS_STATS.len()) else {
        return Pressed::NoClass;
    };

    let Some(slot) = free_slot.filter(|s| *s < SLOTS) else {
        return Pressed::NoSlot;
    };

    Pressed::Send(Box::new(create_packet(slot as u32, &name, doll as u32)))
}

pub const CREATE_OPCODE: u16 = 0x20F;

pub const CREATE_LEN: usize = 0x24;

pub fn create_packet(slot: u32, name: &[u8], doll: u32) -> [u8; CREATE_LEN] {
    let mut out = [0u8; CREATE_LEN];

    out[0x04..0x06].copy_from_slice(&CREATE_OPCODE.to_le_bytes());

    out[0x0C..0x10].copy_from_slice(&slot.to_le_bytes());

    let n = name.len().min(15);

    out[0x10..0x10 + n].copy_from_slice(&name[..n]);

    out[0x20..0x24].copy_from_slice(&doll.to_le_bytes());

    out
}

#[cfg(test)]

mod language_tests {

    use super::*;

    #[test]

    fn multibyte_name_boundaries_do_not_split_encoded_characters() {
        for (cp, name) in [
            (Codepage::Windows949, "한국"),
            (Codepage::Windows950, "中文"),
            (Codepage::Windows936, "中文"),
        ] {
            let bytes = codepage::encode(name, cp);

            assert_eq!(bytes.len(), 4);

            for at in [0, 2, 4] {
                assert!(on_character_boundary(&bytes, at, cp), "{cp:?} {at}");
            }

            for at in [1, 3] {
                assert!(!on_character_boundary(&bytes, at, cp), "{cp:?} {at}");
            }
        }
    }
}
