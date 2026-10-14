use super::{ZONE_CELLS, ZONE_GRID};
use crate::error::{Error, Kind, Result};
use crate::text::codepage::{self, Codepage};

pub const SLOTS: usize = 256;

pub const RECORD_LEN: usize = 0x34;

pub const NAME_LEN: usize = 0x20;

pub const COLORS: [u32; 4] = [0xFFFF_FFFF, 0xFF44_AA44, 0xFF55_55FF, 0xFFAA_00FF];

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Label {
    pub x: i32,
    pub y: i32,
    pub ofs_x: i32,
    pub ofs_y: i32,
    pub color: u32,
    pub raw_name: Vec<u8>,
}

impl Label {
    pub fn kind(&self) -> Option<u8> {
        COLORS
            .iter()
            .position(|&c| c == self.color)
            .map(|i| i as u8 + 1)
    }

    pub fn name(&self, codepage: Codepage) -> String {
        codepage::decode(&self.raw_name, codepage)
    }

    pub fn zone(&self) -> Option<(usize, usize)> {
        let (x, y) = (usize::try_from(self.x).ok()?, usize::try_from(self.y).ok()?);

        let (zx, zy) = (x / ZONE_CELLS, y / ZONE_CELLS);

        (zx < ZONE_GRID && zy < ZONE_GRID).then_some((zx, zy))
    }

    pub fn is_blank(&self) -> bool {
        *self == Label::default()
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Minimap {
    slots: Vec<Label>,
    written: usize,
    truncated: bool,
}

impl Minimap {
    pub fn labels(&self) -> &[Label] {
        &self.slots[..self.written]
    }

    pub fn slots(&self) -> &[Label] {
        &self.slots
    }

    pub fn len(&self) -> usize {
        self.written
    }

    pub fn is_empty(&self) -> bool {
        self.written == 0
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }

    pub fn zones(&self) -> Vec<(usize, usize)> {
        let mut v: Vec<_> = self.labels().iter().filter_map(Label::zone).collect();

        v.sort_unstable();

        v.dedup();

        v
    }

    fn push(&mut self, label: Label) -> bool {
        if self.written == SLOTS {
            self.truncated = true;

            return false;
        }

        self.slots[self.written] = label;

        self.written += 1;

        true
    }

    fn blank() -> Minimap {
        Minimap {
            slots: vec![Label::default(); SLOTS],
            written: 0,
            truncated: false,
        }
    }
}

fn color_for(kind: i32, previous: u32) -> u32 {
    match kind {
        1..=4 => COLORS[kind as usize - 1],
        _ => previous,
    }
}

pub fn parse(bytes: &[u8]) -> Result<Minimap> {
    let mut out = Minimap::blank();

    let mut previous_color = 0;

    for (n, line) in bytes.split(|&b| b == b'\n').enumerate() {
        let line = strip_cr(line);

        if line.iter().all(|b| b.is_ascii_whitespace()) {
            continue;
        }

        let mut fields = split_whitespace(line);

        let mut next = |what: &str| -> Result<i32> {
            let tok = fields
                .next()
                .ok_or_else(|| bad(n, format!("missing {what}")))?;

            std::str::from_utf8(tok)
                .ok()
                .and_then(|s| s.parse::<i32>().ok())
                .ok_or_else(|| bad(n, format!("{what}: {}", String::from_utf8_lossy(tok))))
        };

        let x = next("x")?;

        let y = next("y")?;

        let kind = next("type")?;

        let ofs_x = next("ofs_x")?;

        let ofs_y = next("ofs_y")?;

        let rest: Vec<&[u8]> = fields.collect();

        let raw_name = rest.join(&b' ');

        let color = color_for(kind, previous_color);

        previous_color = color;

        if !out.push(Label {
            x,
            y,
            ofs_x,
            ofs_y,
            color,
            raw_name,
        }) {
            break;
        }
    }

    Ok(out)
}

pub fn parse_whitespace_records(bytes: &[u8]) -> Minimap {
    let mut out = Minimap::blank();

    let mut s = Scanner { bytes, pos: 0 };

    let mut kind = 0i32;

    for slot in 0..SLOTS {
        let mut label = Label::default();

        let mut assigned = 0;

        let mut eof = false;

        for field in 0..6 {
            match field {
                2 => match s.int() {
                    Some(v) => kind = v,
                    None => break,
                },
                5 => match s.token() {
                    Some(v) => label.raw_name = v,
                    None => break,
                },
                _ => {
                    let Some(v) = s.int() else {
                        if assigned == 0 && s.at_end() {
                            eof = true;
                        }

                        break;
                    };

                    match field {
                        0 => label.x = v,
                        1 => label.y = v,
                        3 => label.ofs_x = v,
                        _ => label.ofs_y = v,
                    }
                }
            }

            assigned += 1;
        }

        if eof && assigned == 0 {
            break;
        }

        let previous = out.slots[slot].color;

        label.color = color_for(kind, previous);

        out.slots[slot] = label;

        out.written = slot + 1;
    }

    out
}

struct Scanner<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Scanner<'_> {
    fn at_end(&self) -> bool {
        self.bytes[self.pos..]
            .iter()
            .all(|b| b.is_ascii_whitespace())
    }

    fn skip_space(&mut self) {
        while self
            .bytes
            .get(self.pos)
            .is_some_and(u8::is_ascii_whitespace)
        {
            self.pos += 1;
        }
    }

    fn int(&mut self) -> Option<i32> {
        self.skip_space();

        let start = self.pos;

        let mut end = self.pos;

        if matches!(self.bytes.get(end), Some(b'+' | b'-')) {
            end += 1;
        }

        let digits = end;

        while self.bytes.get(end).is_some_and(u8::is_ascii_digit) {
            end += 1;
        }

        if end == digits {
            self.pos = start;

            return None;
        }

        let v = std::str::from_utf8(&self.bytes[start..end])
            .ok()?
            .parse()
            .ok();

        if v.is_some() {
            self.pos = end;
        }

        v
    }

    fn token(&mut self) -> Option<Vec<u8>> {
        self.skip_space();

        let start = self.pos;

        while self
            .bytes
            .get(self.pos)
            .is_some_and(|b| !b.is_ascii_whitespace())
        {
            self.pos += 1;
        }

        (self.pos > start).then(|| self.bytes[start..self.pos].to_vec())
    }
}

fn strip_cr(line: &[u8]) -> &[u8] {
    match line.split_last() {
        Some((b'\r', head)) => head,
        _ => line,
    }
}

fn split_whitespace(line: &[u8]) -> impl Iterator<Item = &[u8]> {
    line.split(|b| b.is_ascii_whitespace())
        .filter(|t| !t.is_empty())
}

fn bad(line: usize, message: String) -> Error {
    Error::structure(Kind::Minimap, format!("line {}: {message}", line + 1))
}
