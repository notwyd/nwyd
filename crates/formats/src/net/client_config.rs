use std::collections::BTreeMap;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Update(pub Vec<(String, String)>);

pub fn parse(bytes: &[u8]) -> Update {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());

    let text = crate::text::codepage::decode(&bytes[..end], crate::text::Codepage::Windows1252);

    Update(
        text.split('\n')
            .filter_map(|line| line.split_once('='))
            .filter(|(key, _)| !key.is_empty())
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect(),
    )
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Config(BTreeMap<String, String>);

impl Config {
    pub fn apply(&mut self, update: &Update) {
        for (key, value) in &update.0 {
            if key == "__reset" {
                self.0.clear();
            } else {
                self.0.insert(key.clone(), value.clone());
            }
        }
    }

    pub fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(String::as_str)
    }

    pub fn integer(&self, key: &str) -> i64 {
        self.get(key).map_or(0, integer)
    }
}

pub fn integer(text: &str) -> i64 {
    let text = text.trim_start_matches(|c: char| c.is_ascii_whitespace());

    let (negative, digits) = match text.as_bytes().first() {
        Some(b'-') => (true, &text[1..]),
        Some(b'+') => (false, &text[1..]),
        _ => (false, text),
    };

    let mut value = 0i64;

    for byte in digits.bytes().take_while(u8::is_ascii_digit) {
        value = value
            .saturating_mul(10)
            .saturating_sub(i64::from(byte - b'0'));
    }

    if negative {
        value
    } else {
        value.saturating_neg()
    }
}

pub fn normalize_chat(frame: &mut [u8], config: &Config) -> Option<usize> {
    let opcode = u16::from_le_bytes(frame.get(4..6)?.try_into().ok()?);

    let (offset, channel) = match opcode {
        0x333 if frame.len() >= 140 => (12, false),
        0x334 if frame.len() >= 156 => (28, frame[12] == 0),
        _ => return None,
    };

    let bytes = &mut frame[offset..offset + 128];

    let prefix = if channel {
        match bytes[0] {
            b'@' | b'-' => 1 + usize::from(bytes[1] == bytes[0]),
            b'=' => 1,
            _ => 0,
        }
    } else {
        0
    };

    let mut output = Vec::with_capacity(128);

    for &byte in bytes.iter().take_while(|&&b| b != 0) {
        if byte == b'>' {
            if let Some(end) = output.iter().rposition(|&b| b != b' ')
                && output[end] == b']'
            {
                output.remove(end);

                if let Some(start) = output[..end].iter().rposition(|&b| b == b'[') {
                    output.remove(start);
                }

                continue;
            }
        }

        output.push(if byte < 32 { b' ' } else { byte });
    }

    let limit = usize::try_from(config.integer("chatmaxlen"))
        .ok()
        .filter(|&n| n > 0);

    let truncated = limit.filter(|&n| n.saturating_add(prefix) < output.len());

    output.truncate(truncated.map_or(127, |n| n + prefix).min(127));

    bytes.fill(0);

    bytes[..output.len()].copy_from_slice(&output);

    truncated
}

pub fn chat_limit_notice(template: &str, limit: usize) -> Option<String> {
    use crate::text::{Codepage, codepage};

    if template.is_empty() || template.starts_with('þ') {
        return None;
    }

    let mut chars = template.chars();

    let mut out = String::new();

    let mut substitutions = 0;

    while let Some(c) = chars.next() {
        if c != '%' {
            out.push(c);

            continue;
        }

        match chars.next()? {
            '%' => out.push('%'),
            's' => {
                substitutions += 1;

                out.push_str(&limit.to_string());
            }
            _ => return None,
        }
    }

    if substitutions != 1 {
        return None;
    }

    let bytes = codepage::encode(&out, Codepage::Windows1252);

    Some(codepage::decode(
        &bytes[..bytes.len().min(127)],
        Codepage::Windows1252,
    ))
}
