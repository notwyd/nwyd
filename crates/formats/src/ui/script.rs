use crate::error::{Error, Kind, Result};

pub const TRIM: &[char] = &[' ', '\t', '\r', '\n'];

pub const LINE_SEPARATOR: char = '\n';

pub const COMMENT: &str = "//";

pub const TOKEN_DELIMS: &[char] = &['\t', ' '];

pub const VALUE_DELIMS: &[char] = &['\t', '\n', ' '];

pub const BOM: [u8; 2] = [0xFF, 0xFE];

pub const LINE_BUFFER: usize = 0x400;

pub fn decode(kind: Kind, bytes: &[u8]) -> Result<Vec<char>> {
    if !bytes.starts_with(&BOM) {
        let mut found = [0u8; 4];

        let n = bytes.len().min(4);

        found[..n].copy_from_slice(&bytes[..n]);

        return Err(Error::BadMagic {
            kind,
            expected: "FF FE (UTF-16LE BOM)",
            found,
        });
    }

    let body = &bytes[BOM.len()..];

    if !body.len().is_multiple_of(2) {
        return Err(Error::structure(
            kind,
            format!("odd UTF-16 payload length {}", body.len()),
        ));
    }

    let units = body
        .chunks_exact(2)
        .map(|c| u16::from_le_bytes([c[0], c[1]]));

    char::decode_utf16(units)
        .collect::<std::result::Result<Vec<char>, _>>()
        .map_err(|e| Error::structure(kind, format!("invalid UTF-16: {e}")))
}

pub struct Reader<'a> {
    chars: &'a [char],
    pos: usize,
}

impl<'a> Reader<'a> {
    pub fn new(chars: &'a [char]) -> Self {
        Reader { chars, pos: 0 }
    }

    pub fn at_end(&self) -> bool {
        self.pos >= self.chars.len()
    }

    pub fn read_line(&mut self) -> Option<String> {
        if self.at_end() {
            return None;
        }

        while self.pos < self.chars.len() && TRIM.contains(&self.chars[self.pos]) {
            self.pos += 1;
        }

        let mut line = String::new();

        while self.pos < self.chars.len() {
            let c = self.chars[self.pos];

            self.pos += 1;

            if c == LINE_SEPARATOR {
                break;
            }

            line.push(c);
        }

        while line.ends_with(TRIM) {
            line.pop();
        }

        Some(line)
    }

    pub fn skip_to(&mut self, set: &[char]) -> bool {
        while self.pos < self.chars.len() {
            let c = self.chars[self.pos];

            self.pos += 1;

            if set.contains(&c) {
                return true;
            }
        }

        false
    }
}

pub fn is_pane_comment(line: &str) -> bool {
    line.starts_with(COMMENT)
}

pub fn strip_guimat_comment(line: &str) -> &str {
    let line = match line.find(COMMENT) {
        Some(i) => &line[..i],
        None => line,
    };

    line.trim_matches(TRIM)
}

pub fn split_token_and_remainder(line: &str) -> Option<(&str, &str)> {
    let start = line.find(|c| !TOKEN_DELIMS.contains(&c))?;

    let line = &line[start..];

    match line.find(TOKEN_DELIMS) {
        Some(i) => Some((&line[..i], &line[i + 1..])),
        None => Some((line, "")),
    }
}

pub fn split_all(line: &str, delims: &[char]) -> Vec<String> {
    line.split(|c| delims.contains(&c))
        .filter(|t| !t.is_empty())
        .map(str::to_string)
        .collect()
}

pub fn parse_integer_prefix_or_zero(s: &str) -> i32 {
    let s = s.trim_start_matches(|c: char| c.is_ascii_whitespace());

    let (negative, digits) = match s.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, s.strip_prefix('+').unwrap_or(s)),
    };

    let mut acc: i32 = 0;

    for c in digits.chars().take_while(char::is_ascii_digit) {
        acc = acc
            .wrapping_mul(10)
            .wrapping_add((c as u8 - b'0') as i32 * if negative { -1 } else { 1 });
    }

    acc
}

pub fn argb(value: &str) -> u32 {
    let n = split_all(value, VALUE_DELIMS);

    let at = |i: usize| {
        n.get(i)
            .map(|s| parse_integer_prefix_or_zero(s))
            .unwrap_or(0) as u32
            & 0xFF
    };

    at(0) << 24 | at(1) << 16 | at(2) << 8 | at(3)
}

pub fn ints4(value: &str) -> [i32; 4] {
    let n = split_all(value, VALUE_DELIMS);

    std::array::from_fn(|i| {
        n.get(i)
            .map(|s| parse_integer_prefix_or_zero(s))
            .unwrap_or(0)
    })
}

fn scan_i32(token: &str) -> Option<i32> {
    let body = token.strip_prefix(['-', '+']).unwrap_or(token);

    body.starts_with(|c: char| c.is_ascii_digit())
        .then(|| parse_integer_prefix_or_zero(token))
}

pub fn scan_ints(value: &str, out: &mut [i32]) -> usize {
    let mut written = 0;

    for (slot, token) in out.iter_mut().zip(split_all(value, VALUE_DELIMS)) {
        let Some(n) = scan_i32(&token) else {
            break;
        };

        *slot = n;

        written += 1;
    }

    written
}

pub fn parse_float_prefix_or_zero(value: &str) -> f32 {
    scan_float(value).unwrap_or(0.0)
}

pub fn scan_float(value: &str) -> Option<f32> {
    let s = value.trim_start_matches(|c: char| c.is_ascii_whitespace());

    let end = s
        .char_indices()
        .take_while(|(i, c)| {
            c.is_ascii_digit()
                || *c == '.'
                || ((*c == '-' || *c == '+') && *i == 0)
                || ((*c == 'e' || *c == 'E') && *i > 0)
                || ((*c == '-' || *c == '+')
                    && matches!(s.as_bytes().get(i.wrapping_sub(1)), Some(b'e' | b'E')))
        })
        .map(|(i, c)| i + c.len_utf8())
        .last()
        .unwrap_or(0);

    s[..end].parse::<f32>().ok()
}
