use std::collections::HashMap;

#[derive(Debug, Clone, Default)]

pub struct CharMap {
    entries: HashMap<u32, u16>,
    selected: Option<(u16, u16)>,
}

impl CharMap {
    pub fn glyph(&self, code_point: u32) -> u16 {
        self.entries.get(&code_point).copied().unwrap_or(0)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn selected(&self) -> Option<(u16, u16)> {
        self.selected
    }

    pub fn parse(font: &[u8], index: u32) -> CharMap {
        let Some(offset) = sfnt_offset(font, index) else {
            return CharMap::default();
        };

        let Some(cmap) = table(font, offset, b"cmap") else {
            return CharMap::default();
        };

        let Some((platform, encoding, sub)) = select(font, cmap) else {
            return CharMap::default();
        };

        CharMap {
            entries: subtable(font, sub).unwrap_or_default(),
            selected: Some((platform, encoding)),
        }
    }
}

fn u16_at(data: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(data.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(data: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(data.get(at..at + 4)?.try_into().ok()?))
}

fn sfnt_offset(font: &[u8], index: u32) -> Option<usize> {
    if font.get(..4)? == b"ttcf" {
        let count = u32_at(font, 8)?;

        if index >= count {
            return None;
        }

        return Some(u32_at(font, 12 + 4 * index as usize)? as usize);
    }

    (index == 0).then_some(0)
}

fn table(font: &[u8], directory: usize, tag: &[u8; 4]) -> Option<usize> {
    let count = u16_at(font, directory + 4)? as usize;

    for i in 0..count {
        let record = directory + 12 + 16 * i;

        if font.get(record..record + 4)? == tag {
            return Some(u32_at(font, record + 8)? as usize);
        }
    }

    None
}

const PREFERENCE: [(u16, u16); 8] = [
    (3, 10),
    (3, 1),
    (0, 6),
    (0, 4),
    (0, 3),
    (0, 2),
    (0, 1),
    (0, 0),
];

fn select(font: &[u8], cmap: usize) -> Option<(u16, u16, usize)> {
    let count = u16_at(font, cmap + 2)? as usize;

    let mut found: Vec<(u16, u16, usize)> = Vec::with_capacity(count);

    for i in 0..count {
        let record = cmap + 4 + 8 * i;

        let platform = u16_at(font, record)?;

        let encoding = u16_at(font, record + 2)?;

        let offset = u32_at(font, record + 4)? as usize;

        found.push((platform, encoding, cmap + offset));
    }

    PREFERENCE
        .iter()
        .find_map(|(p, e)| found.iter().find(|(fp, fe, _)| fp == p && fe == e).copied())
}

fn subtable(font: &[u8], at: usize) -> Option<HashMap<u32, u16>> {
    match u16_at(font, at)? {
        0 => format0(font, at),
        4 => format4(font, at),
        6 => format6(font, at),
        12 => format12(font, at),
        _ => Some(HashMap::new()),
    }
}

fn format0(font: &[u8], at: usize) -> Option<HashMap<u32, u16>> {
    let ids = font.get(at + 6..at + 6 + 256)?;

    Some(
        ids.iter()
            .enumerate()
            .filter(|(_, g)| **g != 0)
            .map(|(c, g)| (c as u32, u16::from(*g)))
            .collect(),
    )
}

fn format4(font: &[u8], at: usize) -> Option<HashMap<u32, u16>> {
    let seg_x2 = u16_at(font, at + 6)? as usize;

    let segments = seg_x2 / 2;

    let ends = at + 14;

    let starts = ends + seg_x2 + 2;

    let deltas = starts + seg_x2;

    let ranges = deltas + seg_x2;

    let mut out = HashMap::new();

    for i in 0..segments {
        let end = u16_at(font, ends + 2 * i)?;

        let start = u16_at(font, starts + 2 * i)?;

        if start > end {
            continue;
        }

        let delta = u16_at(font, deltas + 2 * i)?;

        let range = u16_at(font, ranges + 2 * i)? as usize;

        for code in start..=end {
            if code == 0xFFFF {
                continue;
            }

            let glyph = if range == 0 {
                code.wrapping_add(delta)
            } else {
                let slot = ranges + 2 * i + range + 2 * (code - start) as usize;

                match u16_at(font, slot)? {
                    0 => 0,
                    g => g.wrapping_add(delta),
                }
            };

            if glyph != 0 {
                out.insert(u32::from(code), glyph);
            }
        }
    }

    Some(out)
}

fn format6(font: &[u8], at: usize) -> Option<HashMap<u32, u16>> {
    let first = u16_at(font, at + 6)? as u32;

    let count = u16_at(font, at + 8)? as usize;

    let mut out = HashMap::with_capacity(count);

    for i in 0..count {
        let glyph = u16_at(font, at + 10 + 2 * i)?;

        if glyph != 0 {
            out.insert(first + i as u32, glyph);
        }
    }

    Some(out)
}

fn format12(font: &[u8], at: usize) -> Option<HashMap<u32, u16>> {
    let groups = u32_at(font, at + 12)? as usize;

    let mut out = HashMap::new();

    for i in 0..groups {
        let group = at + 16 + 12 * i;

        let start = u32_at(font, group)?;

        let end = u32_at(font, group + 4)?;

        let glyph = u32_at(font, group + 8)?;

        if end < start || end - start > 0x11_0000 {
            continue;
        }

        for code in start..=end {
            let id = glyph + (code - start);

            if id != 0 && id <= u32::from(u16::MAX) {
                out.insert(code, id as u16);
            }
        }
    }

    Some(out)
}
