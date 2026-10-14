use crate::error::{Error, Kind, Result};

pub const OBJECT_COUNT: usize = 4096;

pub const MASK_DIM: usize = 16;

pub const MASK_BYTES: usize = MASK_DIM * MASK_DIM;

pub const PAYLOAD_LEN: usize = OBJECT_COUNT * MASK_BYTES;

pub const FILE_LEN: usize = PAYLOAD_LEN + 4;

pub const USED_DIM: usize = 15;

pub const MASK_CENTRE: usize = 7;

pub const KEY: [u8; 170] = [
    0xb1, 0xdd, 0xb0, 0xad, 0xbb, 0xea, 0xc3, 0xa3, 0xbe, 0xc6, 0xb0, 0xa1, 0xc0, 0xda, 0xc0, 0xcf,
    0xb8, 0xb8, 0xc0, 0xcc, 0xc3, 0xb5, 0xba, 0xc0, 0xba, 0xbc, 0xbc, 0xf6, 0xb7, 0xcf, 0xbe, 0xc6,
    0xb8, 0xa7, 0xb4, 0xe4, 0xb0, 0xed, 0xbd, 0xc5, 0xba, 0xf1, 0xc7, 0xcf, 0xb1, 0xb8, 0xb3, 0xaa,
    0xbf, 0xec, 0xb8, 0xae, 0xb3, 0xaa, 0xb6, 0xf3, 0xc1, 0xc1, 0xc0, 0xba, 0xb3, 0xaa, 0xb6, 0xf3,
    0xbb, 0xf5, 0xb3, 0xaa, 0xb6, 0xf3, 0xc0, 0xc7, 0xbe, 0xee, 0xb8, 0xb0, 0xc0, 0xcc, 0xb4, 0xc2,
    0xc0, 0xcf, 0xc2, 0xef, 0xc0, 0xcf, 0xbe, 0xee, 0xb3, 0xb3, 0xb4, 0xcf, 0xb4, 0xd9, 0xc0, 0xe1,
    0xb2, 0xd9, 0xb7, 0xaf, 0xb1, 0xe2, 0xbe, 0xf8, 0xb4, 0xc2, 0xb3, 0xaa, 0xb6, 0xf3, 0xbf, 0xec,
    0xb8, 0xae, 0xb3, 0xaa, 0xb6, 0xf3, 0xc1, 0xc1, 0xc0, 0xba, 0xb3, 0xaa, 0xb6, 0xf3, 0xb9, 0xab,
    0xb1, 0xc3, 0xc8, 0xad, 0xb9, 0xab, 0xb1, 0xc3, 0xc8, 0xad, 0xbf, 0xec, 0xb8, 0xae, 0xb3, 0xaa,
    0xb6, 0xf3, 0xb2, 0xc9, 0xbb, 0xef, 0xc3, 0xb5, 0xb8, 0xae, 0xb0, 0xad, 0xbb, 0xea, 0xbf, 0xa1,
    0xbf, 0xec, 0xb8, 0xae, 0xb3, 0xaa, 0xb6, 0xf3, 0xb2, 0xc9,
];

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct ObjectMasks {
    data: Vec<u8>,
}

fn err(message: impl Into<String>) -> Error {
    Error::Structure {
        kind: Kind::ObjectBin,
        message: format!("object.bin: {}", message.into()),
    }
}

pub fn decrypt(cipher: &mut [u8]) {
    for (i, b) in cipher.iter_mut().enumerate() {
        *b = b.wrapping_sub(KEY[i % KEY.len()]).wrapping_sub(i as u8);
    }
}

pub fn checksum(cipher: &[u8]) -> u32 {
    #[inline]

    fn trunc_mod4(v: i8) -> i32 {
        (v as i32) % 4
    }

    let mut total: i32 = 0;

    let mut off = 0usize;

    let mut k3: i32 = 0;

    for _ in 0..4096 {
        let mut k4: i32 = 0;

        for _ in 0..16 {
            let (mut a, mut c, mut e) = (0i32, 0i32, 0i32);

            for i in [0usize, 4, 8, 12] {
                let bias = k3 + k4;

                total = total.wrapping_add(trunc_mod4(cipher[off + i] as i8) + bias);

                a = a.wrapping_add(trunc_mod4(cipher[off + i + 1] as i8) + bias);

                c = c.wrapping_add(trunc_mod4(cipher[off + i + 2] as i8) + bias);

                e = e.wrapping_add(trunc_mod4(cipher[off + i + 3] as i8) + bias);
            }

            total = total.wrapping_add(e).wrapping_add(c).wrapping_add(a);

            off += 16;

            k4 += 2;
        }

        k3 += 5;
    }

    total as u32
}

pub fn parse(bytes: &[u8]) -> Result<ObjectMasks> {
    if bytes.len() != FILE_LEN {
        return Err(err(format!(
            "is {} bytes, expected {FILE_LEN} ({OBJECT_COUNT} x {MASK_BYTES} + 4)",
            bytes.len()
        )));
    }

    let (payload, trailer) = bytes.split_at(PAYLOAD_LEN);

    let expected = u32::from_le_bytes(trailer.try_into().expect("split at len-4"));

    let actual = checksum(payload);

    if actual != expected {
        return Err(err(format!(
            "checksum mismatch: file says {expected}, computed {actual}"
        )));
    }

    let mut data = payload.to_vec();

    decrypt(&mut data);

    Ok(ObjectMasks { data })
}

impl ObjectMasks {
    pub fn mask(&self, id: usize) -> Option<&[u8]> {
        if id >= OBJECT_COUNT {
            return None;
        }

        self.data.get(id * MASK_BYTES..(id + 1) * MASK_BYTES)
    }

    pub fn cell(&self, id: usize, row: usize, col: usize) -> Option<u8> {
        if row >= MASK_DIM || col >= MASK_DIM {
            return None;
        }

        self.mask(id).map(|m| m[row * MASK_DIM + col])
    }

    pub fn is_empty(&self, id: usize) -> bool {
        self.mask(id).is_none_or(|m| m.iter().all(|&b| b == 0))
    }

    pub fn populated_ids(&self) -> impl Iterator<Item = usize> + '_ {
        (0..OBJECT_COUNT).filter(|&id| !self.is_empty(id))
    }
}

pub fn parse_mask_text(text: &[u8]) -> Vec<(usize, Vec<u8>)> {
    let text = String::from_utf8_lossy(text);

    let mut out = Vec::new();

    let mut lines = text.lines().peekable();

    while let Some(line) = lines.next() {
        let line = line.trim();

        let Some(rest) = line.strip_prefix('#') else {
            continue;
        };

        let Ok(id) = rest.trim().parse::<usize>() else {
            continue;
        };

        let mut mask = vec![0u8; MASK_BYTES];

        for row in 0..USED_DIM {
            let Some(row_text) = lines.peek() else {
                break;
            };

            let row_text = row_text.trim();

            if row_text.len() < USED_DIM || !row_text.bytes().all(is_mask_char) {
                break;
            }

            for (col, ch) in row_text.bytes().take(USED_DIM).enumerate() {
                mask[row * MASK_DIM + col] = decode_mask_char(ch);
            }

            lines.next();
        }

        out.push((id, mask));
    }

    out
}

fn is_mask_char(c: u8) -> bool {
    c.is_ascii_digit() || c.is_ascii_lowercase() || c == b'X'
}

fn decode_mask_char(c: u8) -> u8 {
    match c {
        b'0'..=b'9' => c - b'0',
        b'a'..=b'z' => c - b'a' + 10,
        b'X' => IMPASSABLE,
        _ => 0,
    }
}

use super::IMPASSABLE;
