use super::codepage::{self, Codepage};

pub const MAX_LINES: usize = 0x230;

pub const PAGE_ROWS: usize = 10;

pub const COORD_WIDTH: usize = 9;

pub const GUARD_BYTE: usize = 6;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub colour: u32,
    pub zone: String,
    pub name: String,
    pub x: u16,
    pub y: u16,
    pub coords: String,
}

pub fn parse(bytes: &[u8], codepage: Codepage) -> Vec<Row> {
    let mut rows = Vec::new();

    for line in bytes.split(|&b| b == b'\n').take(MAX_LINES) {
        let line = line.strip_suffix(b"\r").unwrap_or(line);

        let mut line: Vec<u8> = line
            .iter()
            .map(|&b| if b == b'_' { b' ' } else { b })
            .collect();

        if line.get(GUARD_BYTE) != Some(&b' ') {
            continue;
        }

        let Some(colour) = hex6(&line) else {
            continue;
        };

        if line.len() < COORD_WIDTH {
            continue;
        }

        let tail = line.split_off(line.len() - COORD_WIDTH);

        let Some((x, y)) = two_numbers(&tail) else {
            continue;
        };

        let Some(close) = line.iter().position(|&b| b == b']') else {
            continue;
        };

        let zone = codepage::decode(&line[GUARD_BYTE..=close], codepage)
            .trim()
            .to_string();

        let name = codepage::decode(&line[close + 1..], codepage)
            .trim()
            .to_string();

        let coords = codepage::decode(&tail, codepage).trim().to_string();

        rows.push(Row {
            colour,
            zone,
            name,
            x,
            y,
            coords,
        });
    }

    rows
}

pub fn page(rows: &[Row], page: usize) -> &[Row] {
    let start = page * PAGE_ROWS;

    let end = (start + PAGE_ROWS).min(rows.len());

    rows.get(start..end).unwrap_or(&[])
}

fn hex6(line: &[u8]) -> Option<u32> {
    let head = line.get(..GUARD_BYTE)?;

    let text = std::str::from_utf8(head).ok()?;

    u32::from_str_radix(text.trim(), 16).ok()
}

fn two_numbers(tail: &[u8]) -> Option<(u16, u16)> {
    let text = std::str::from_utf8(tail).ok()?;

    let mut parts = text.split_ascii_whitespace();

    let x = parts.next()?.parse().ok()?;

    let y = parts.next()?.parse().ok()?;

    parts.next().is_none().then_some((x, y))
}
