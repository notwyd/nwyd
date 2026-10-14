use super::ITEM_SLOTS;
use crate::text::{Codepage, codepage};

pub const LINES: usize = 9;

pub const LINES_PER_RECORD: usize = LINES + 1;

pub const LINE_LEN: usize = 0x80;

pub const RECORD_LEN: usize = 0x14 + LINES * LINE_LEN;

pub const BUFFER_LEN: usize = ITEM_SLOTS * RECORD_LEN;

pub const MIX_RECORDS: usize = 200;

pub const MIX_RECORD_LEN: usize = 0x924;

pub const MISSION_RECORDS: usize = 5_000;

pub const TEXT_COLUMN: usize = 9;

pub const COLOR_DIGITS: usize = 6;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Help {
    pub color: [u16; LINES],
    pub color_full: [u32; LINES],
    pub color_override: [u32; LINES],
    pub count: u16,
    pub line: [String; LINES],
}

impl Help {
    pub fn argb(&self, index: usize) -> u32 {
        let full = self.color_override[index];

        0xff00_0000
            | if full != 0 {
                full
            } else {
                i32::from(self.color[index] as i16) as u32
            }
    }

    pub fn is_populated(&self) -> bool {
        self.count != 0 || self.line.iter().any(|l| !l.is_empty())
    }

    pub fn lines(&self) -> impl Iterator<Item = (&str, u16)> + '_ {
        self.line
            .iter()
            .zip(self.color)
            .filter(|(l, _)| !l.is_empty())
            .map(|(l, c)| (l.as_str(), c))
    }
}

fn lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes
        .split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r".as_ref()).unwrap_or(l))
}

fn parse_color(line: &[u8]) -> (u16, u32) {
    let digits = &line[..line.len().min(COLOR_DIGITS)];

    let end = digits
        .iter()
        .position(|b| !b.is_ascii_hexdigit())
        .unwrap_or(digits.len());

    let full = digits[..end].iter().fold(0u32, |acc, &b| {
        acc * 16 + (b as char).to_digit(16).unwrap_or(0)
    });

    (full as u16, full)
}

fn parse_header(line: &[u8]) -> Option<(i64, u16)> {
    let mut it = line
        .split(|b| b.is_ascii_whitespace())
        .filter(|f| !f.is_empty());

    let id = std::str::from_utf8(it.next()?).ok()?.parse::<i64>().ok()?;

    let count = it
        .next()
        .and_then(|f| std::str::from_utf8(f).ok())
        .and_then(|s| s.parse::<i64>().ok())
        .unwrap_or(0);

    Some((id, count as u16))
}

pub fn parse(bytes: &[u8], codepage: Codepage) -> Vec<Help> {
    parse_with(bytes, ITEM_SLOTS, codepage).0
}

pub fn parse_mix(bytes: &[u8], codepage: Codepage) -> (Vec<Help>, Vec<String>) {
    parse_named(bytes, MIX_RECORDS, codepage)
}

pub fn parse_mission(bytes: &[u8], codepage: Codepage) -> (Vec<Help>, Vec<String>) {
    parse_named(bytes, MISSION_RECORDS, codepage)
}

fn parse_named(bytes: &[u8], slots: usize, codepage: Codepage) -> (Vec<Help>, Vec<String>) {
    let mut table = vec![Help::default(); slots];

    let mut names = vec![String::new(); slots];

    let mut it = lines(bytes);

    for _ in 0..slots {
        let Some(header) = it.next() else {
            break;
        };

        let rest = header
            .iter()
            .position(|b| b.is_ascii_whitespace())
            .map(|i| &header[i..])
            .unwrap_or(&[]);

        let name = &header[..header.len() - rest.len()];

        let parsed = parse_header(rest);

        let mut record = Help::default();

        read_body(&mut it, &mut record, codepage);

        if let Some((id, count)) = parsed
            && (0..slots as i64).contains(&id)
        {
            record.count = count;

            names[id as usize] = codepage::decode(name, codepage);

            table[id as usize] = record;
        }
    }

    (table, names)
}

fn read_body<'a>(it: &mut impl Iterator<Item = &'a [u8]>, record: &mut Help, codepage: Codepage) {
    for i in 0..LINES {
        let Some(line) = it.next() else {
            return;
        };

        let (truncated, full) = parse_color(line);

        record.color[i] = truncated;

        record.color_full[i] = full;

        if matches!(line.get(TEXT_COLUMN - 1), Some(b'\t' | b' ')) {
            let text = &line[TEXT_COLUMN..];

            let n = text.len().min(LINE_LEN - 1);

            record.line[i] = codepage::decode(&text[..n], codepage);
        }
    }
}

fn parse_with(bytes: &[u8], slots: usize, codepage: Codepage) -> (Vec<Help>, usize) {
    let mut table = vec![Help::default(); slots];

    let mut it = lines(bytes);

    let mut groups = 0;

    for _ in 0..slots {
        let Some(header) = it.next() else {
            break;
        };

        groups += 1;

        let parsed = parse_header(header);

        let mut record = Help::default();

        read_body(&mut it, &mut record, codepage);

        if let Some((id, count)) = parsed
            && (0..slots as i64).contains(&id)
        {
            record.count = count;

            table[id as usize] = record;
        }
    }

    (table, groups)
}

pub fn overlay_colors(table: &mut [Help], bytes: &[u8]) {
    let mut lines = Vec::new();

    let mut at = 0;

    while at < bytes.len() {
        let start = at;

        while at < bytes.len() && !matches!(bytes[at], b'\r' | b'\n') {
            at += 1;
        }

        lines.push(&bytes[start..at]);

        if bytes.get(at) == Some(&b'\r') {
            at += 1;
        }

        if bytes.get(at) == Some(&b'\n') {
            at += 1;
        }
    }

    for record in lines.chunks(10) {
        let id = record[0]
            .iter()
            .take_while(|b| b.is_ascii_digit())
            .fold(0u32, |v, b| {
                v.wrapping_mul(10).wrapping_add(u32::from(*b - b'0'))
            });

        let Some(help) = table.get_mut(id as usize) else {
            continue;
        };

        for (index, line) in record.iter().skip(1).enumerate() {
            let (narrow, full) = parse_color(line);

            help.color_override[index] = full;

            if narrow == 0 {
                help.color[index] = 0x0101;
            }
        }
    }
}
