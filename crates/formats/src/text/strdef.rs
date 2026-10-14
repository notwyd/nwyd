use super::codepage::{self, Codepage};

pub const RECORDS: usize = 2000;

pub const RECORD_LEN: usize = 128;

pub const LEN: usize = RECORDS * RECORD_LEN;

pub const PAD: u8 = 0xFE;

const OBFUSCATION_KEY: u8 = 0x5A;

pub fn checksum(buf: &[u8]) -> i32 {
    let mut acc: i32 = 0;

    for (i, &b) in buf.iter().enumerate() {
        let c = b as i8 as i32;

        let (extra, add_third) = match i % 7 {
            0 => (c / 2, true),
            1 => (c ^ 0xff, true),
            2 => (c.wrapping_mul(3), true),
            3 => (c.wrapping_mul(2), true),
            4 => (-(c / 7), true),
            5 => (-c, false),
            _ => (0, true),
        };

        acc = acc.wrapping_add(extra);

        if add_third {
            acc = acc.wrapping_add(c / 3);
        }
    }

    acc
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrDef {
    entries: Vec<String>,
    obfuscated: bool,
    codepage: Codepage,
}

pub fn parse(bytes: &[u8], codepage: Codepage) -> StrDef {
    let mut buf = vec![0u8; LEN];

    let n = bytes.len().min(LEN);

    buf[..n].copy_from_slice(&bytes[..n]);

    let trailer = bytes
        .get(LEN..LEN + 4)
        .map(|t| i32::from_le_bytes(t.try_into().expect("4 bytes")))
        .unwrap_or(0);

    let obfuscated = checksum(&buf) == trailer;

    if obfuscated {
        for b in &mut buf {
            *b ^= OBFUSCATION_KEY;
        }
    }

    let entries = buf
        .chunks_exact(RECORD_LEN)
        .map(|r| {
            let end = r.iter().position(|&b| b == 0).unwrap_or(RECORD_LEN);

            codepage::decode(&r[..end], codepage)
        })
        .collect();

    StrDef {
        entries,
        obfuscated,
        codepage,
    }
}

impl StrDef {
    pub fn get(&self, id: usize) -> &str {
        self.entries.get(id).map(String::as_str).unwrap_or("")
    }

    pub fn entries(&self) -> &[String] {
        &self.entries
    }

    pub fn populated(&self) -> usize {
        self.entries.iter().filter(|s| !s.is_empty()).count()
    }

    pub fn was_obfuscated(&self) -> bool {
        self.obfuscated
    }

    pub fn codepage(&self) -> Codepage {
        self.codepage
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = vec![0u8; LEN];

        for (i, s) in self.entries.iter().enumerate().take(RECORDS) {
            if s.is_empty() {
                continue;
            }

            let encoded = codepage::encode(s, self.codepage);

            let start = i * RECORD_LEN;

            let n = encoded.len().min(RECORD_LEN - 1);

            out[start..start + n].copy_from_slice(&encoded[..n]);

            out[start + n] = 0;

            for b in &mut out[start + n + 1..start + RECORD_LEN] {
                *b = PAD;
            }
        }

        out
    }
}
