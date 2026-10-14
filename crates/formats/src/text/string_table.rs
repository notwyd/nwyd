use super::codepage::{self, Codepage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StringTable {
    entries: Vec<String>,
}

impl StringTable {
    pub fn get(&self, id: usize) -> &str {
        self.entries.get(id).map(String::as_str).unwrap_or("")
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn populated(&self) -> usize {
        self.entries.iter().filter(|s| !s.is_empty()).count()
    }
}

fn lines(bytes: &[u8]) -> impl Iterator<Item = &[u8]> {
    bytes
        .split(|&b| b == b'\n')
        .map(|l| l.strip_suffix(b"\r").unwrap_or(l))
}

pub mod quest {

    use super::{Codepage, StringTable, codepage, lines, parse_id_and_token};

    pub const SLOTS: usize = 2000;

    pub const MAX_ID: usize = 1999;

    pub const MAX_LEN: usize = 125;

    pub fn parse(bytes: &[u8], cp: Codepage) -> StringTable {
        let mut entries = vec![String::new(); SLOTS];

        for line in lines(bytes) {
            if line.first().is_none_or(|&b| b == b'/') {
                continue;
            }

            let Some((id, token)) = parse_id_and_token(line) else {
                continue;
            };

            if !(1..=MAX_ID as i64).contains(&id) || token.len() > MAX_LEN {
                continue;
            }

            entries[id as usize] = codepage::decode(token, cp);
        }

        StringTable { entries }
    }
}

pub mod ui {

    use super::{Codepage, StringTable, codepage, lines, parse_id_and_token};

    pub const SLOTS: usize = 440;

    pub const MAX_ID: usize = 439;

    pub const MAX_LEN: usize = 63;

    pub fn parse(bytes: &[u8], cp: Codepage) -> StringTable {
        let mut entries = vec![String::new(); SLOTS];

        let mut carried: Vec<u8> = Vec::new();

        for line in lines(bytes) {
            let Some((id, token)) = parse_id_and_token(line) else {
                continue;
            };

            if !token.is_empty() {
                carried = token.to_vec();
            }

            if !(0..=MAX_ID as i64).contains(&id) || carried.len() > MAX_LEN {
                continue;
            }

            entries[id as usize] = codepage::decode(&carried, cp);
        }

        StringTable { entries }
    }
}

pub fn parse_token_list(bytes: &[u8], slots: usize, max_len: usize, cp: Codepage) -> Vec<String> {
    bytes
        .split(|b| b.is_ascii_whitespace())
        .filter(|t| !t.is_empty() && t.len() <= max_len)
        .take(slots)
        .map(|t| codepage::decode(t, cp))
        .collect()
}

pub const EFFECT_STRING_SLOTS: usize = 54;

pub const EFFECT_SUBSTRING_SLOTS: usize = 10;

pub const EFFECT_STRING_MAX_LEN: usize = 23;

fn parse_id_and_token(line: &[u8]) -> Option<(i64, &[u8])> {
    let start = line.iter().position(|b| !b.is_ascii_whitespace())?;

    let line = &line[start..];

    let end = line
        .iter()
        .enumerate()
        .take_while(|(i, b)| b.is_ascii_digit() || (*i == 0 && (**b == b'-' || **b == b'+')))
        .map(|(i, _)| i + 1)
        .last()?;

    let id: i64 = std::str::from_utf8(&line[..end]).ok()?.parse().ok()?;

    let token = line[end..]
        .split(|b| b.is_ascii_whitespace())
        .find(|t| !t.is_empty())
        .unwrap_or(b"");

    Some((id, token))
}
