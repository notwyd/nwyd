use crate::text::Codepage;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub item: u32,
    pub slot: u32,
    pub icon: u16,
    pub text: Vec<u8>,
    pub codepage: Codepage,
}

fn space(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n' | 11 | 12)
}

fn number(line: &[u8], at: &mut usize) -> Option<u32> {
    while *at < line.len() && space(line[*at]) {
        *at += 1;
    }

    let mut negative = false;

    if *at < line.len() && matches!(line[*at], b'+' | b'-') {
        negative = line[*at] == b'-';

        *at += 1;
    }

    let start = *at;

    let mut value = 0u32;

    while *at < line.len() && line[*at].is_ascii_digit() {
        value = value
            .wrapping_mul(10)
            .wrapping_add((line[*at] - b'0') as u32);

        *at += 1;
    }

    if *at == start {
        None
    } else {
        Some(if negative {
            value.wrapping_neg()
        } else {
            value
        })
    }
}

pub fn parse(data: &[u8]) -> Vec<Entry> {
    parse_with_codepage(data, Codepage::Windows1252)
}

pub fn parse_with_codepage(data: &[u8], codepage: Codepage) -> Vec<Entry> {
    let mut result = Vec::new();

    for physical in data.split(|&b| b == b'\n' || b == b'\r') {
        let line = &physical[..physical
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(physical.len())];

        let mut at = 0;

        while at < line.len() && space(line[at]) {
            at += 1;
        }

        let start = at;

        while at < line.len() && !space(line[at]) && at - start < 15 {
            at += 1;
        }

        if &line[start..at] != b"M-ITEM" {
            continue;
        }

        let Some(item) = number(line, &mut at) else {
            continue;
        };

        let Some(slot) = number(line, &mut at) else {
            continue;
        };

        let Some(icon) = number(line, &mut at) else {
            continue;
        };

        if item >= 5000 || slot >= 8 || icon > 65535 {
            continue;
        }

        at = 0;

        for _ in 0..4 {
            while at < line.len() && matches!(line[at], b' ' | b'\t') {
                at += 1;
            }

            while at < line.len() && !matches!(line[at], b' ' | b'\t') {
                at += 1;
            }
        }

        while at < line.len() && matches!(line[at], b' ' | b'\t') {
            at += 1;
        }

        let mut text = line[at..].iter().copied().take(47).collect::<Vec<_>>();

        while text.last().is_some_and(|b| matches!(b, b' ' | b'\t')) {
            text.pop();
        }

        result.push(Entry {
            item,
            slot,
            icon: icon as u16,
            text,
            codepage,
        });
    }

    result
}

pub fn overlay(table: &mut std::collections::BTreeMap<(u32, u32), Entry>, bytes: &[u8]) {
    overlay_with_codepage(table, bytes, Codepage::Windows1252);
}

pub fn overlay_with_codepage(
    table: &mut std::collections::BTreeMap<(u32, u32), Entry>,
    bytes: &[u8],
    codepage: Codepage,
) {
    for row in parse_with_codepage(bytes, codepage) {
        table.insert((row.item, row.slot), row);
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use crate::text::codepage::decode;
    use std::collections::BTreeMap;

    #[test]
    fn existing_entrypoints_keep_windows_1252_and_raw_text() {
        let bytes = b"M-ITEM 12 0 21 Pre\xE7o \x80\r\n";

        let rows = parse(bytes);

        assert_eq!(rows, parse_with_codepage(bytes, Codepage::Windows1252));

        assert_eq!(rows[0].text, b"Pre\xE7o \x80");

        assert_eq!(rows[0].codepage, Codepage::Windows1252);

        assert_eq!(decode(&rows[0].text, rows[0].codepage), "Preço €");

        let mut table = BTreeMap::new();

        overlay(&mut table, bytes);

        assert_eq!(table[&(12, 0)], rows[0]);
    }

    #[test]
    fn mixed_overlays_preserve_each_entry_source_codepage() {
        let mut table = BTreeMap::new();

        overlay(
            &mut table,
            b"M-ITEM 12 0 21 Pre\xE7o\nM-ITEM 12 1 22 Original\n",
        );

        overlay_with_codepage(
            &mut table,
            b"M-ITEM 12 1 23 Za\xBF\xF3\xB3\xE6\n",
            Codepage::Windows1250,
        );

        overlay_with_codepage(
            &mut table,
            b"M-ITEM 12 2 24 \xC7\xD1\xB1\xB9\xBE\xEE\n",
            Codepage::Windows949,
        );

        assert_eq!(table.len(), 3);

        for (slot, codepage, text, icon) in [
            (0, Codepage::Windows1252, "Preço", 21),
            (1, Codepage::Windows1250, "Zażółć", 23),
            (2, Codepage::Windows949, "한국어", 24),
        ] {
            let entry = &table[&(12, slot)];

            assert_eq!(entry.codepage, codepage);

            assert_eq!(decode(&entry.text, entry.codepage), text);

            assert_eq!(entry.icon, icon);
        }

        overlay(&mut table, b"M-ITEM 12 1 25 Ma\xE7\xE3\n");

        let replaced = &table[&(12, 1)];

        assert_eq!(replaced.codepage, Codepage::Windows1252);

        assert_eq!(decode(&replaced.text, replaced.codepage), "Maçã");

        assert_eq!(table[&(12, 2)].codepage, Codepage::Windows949);
    }
}
