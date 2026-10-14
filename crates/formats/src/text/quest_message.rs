use super::codepage::{self, Codepage};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub id: i32,
    pub color: u32,
    pub text: String,
}

pub fn parse(bytes: &[u8], codepage: Codepage) -> Vec<Row> {
    bytes
        .split(|b| *b == b'\n')
        .take(100)
        .filter_map(|line| {
            let line = line.strip_suffix(b"\r").unwrap_or(line);

            let header = std::str::from_utf8(line.get(..10)?).ok()?;

            let mut words = header.split_ascii_whitespace();

            let id = words.next()?.parse::<i32>().ok()?;

            let color = u32::from_str_radix(words.next()?.trim_start_matches("0x"), 16).ok()?;

            if id <= 0 {
                return None;
            }

            let text = if line.get(10) == Some(&b' ') {
                codepage::decode(&line[10..], codepage)
            } else {
                String::new()
            };

            Some(Row { id, color, text })
        })
        .collect()
}

impl Row {
    pub fn arrival(&self, body_digit: i32) -> (String, u32) {
        if (self.color == 0x00ffaaaa && body_digit > 5) || (self.id > 350 && body_digit < 6) {
            (String::new(), 0xff000000)
        } else {
            (self.text.clone(), self.color | 0xff000000)
        }
    }
}
