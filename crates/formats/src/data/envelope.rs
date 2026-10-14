use crate::error::{Error, Kind, Result};

pub const XOR_KEY: u8 = 0x5A;

pub fn xor_5a(buf: &mut [u8]) {
    for b in buf {
        *b ^= XOR_KEY;
    }
}

pub fn nine_step_checksum(buf: &[u8]) -> i32 {
    let mut acc: i32 = 0;

    for (i, &b) in buf.iter().enumerate() {
        let c = b as i8 as i32;

        let (extra, add_fifth) = match i % 9 {
            0 => (c.wrapping_mul(2), true),
            1 => (c ^ 0xff, true),
            2 => (c / 3, true),
            3 => (c.wrapping_mul(2), true),
            4 => (-(c ^ 0x5a), true),
            5 => (-c, false),
            _ => (0, true),
        };

        acc = acc.wrapping_add(extra);

        if add_fifth {
            acc = acc.wrapping_add(c / 5);
        }
    }

    acc
}

pub fn seven_step_checksum(buf: &[u8]) -> i32 {
    crate::text::strdef::checksum(buf)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Integrity {
    ConstantA(i32),
    TrailerB,
    IgnoredTrailer,
    None,
}

#[derive(Debug, Clone, Copy)]
pub struct Split<'a> {
    pub body: &'a [u8],
    pub trailer: Option<i32>,
}

pub fn split(bytes: &[u8], body_len: usize, kind: Kind) -> Result<Split<'_>> {
    let body = bytes.get(..body_len).ok_or(Error::Truncated {
        kind,
        offset: 0,
        needed: body_len,
        len: bytes.len(),
    })?;

    let trailer = bytes
        .get(body_len..body_len + 4)
        .map(|t| i32::from_le_bytes(t.try_into().expect("slice is 4 bytes")));

    Ok(Split { body, trailer })
}

pub fn open(bytes: &[u8], body_len: usize, integrity: Integrity, kind: Kind) -> Result<Vec<u8>> {
    let Split { body, trailer } = split(bytes, body_len, kind)?;

    match integrity {
        Integrity::ConstantA(expected) => {
            let got = nine_step_checksum(body);

            if got != expected {
                return Err(Error::structure(
                    kind,
                    format!("checksum A mismatch: expected {expected:#010x}, computed {got:#010x}"),
                ));
            }
        }
        Integrity::TrailerB => {
            let Some(expected) = trailer else {
                return Err(Error::Truncated {
                    kind,
                    offset: body_len,
                    needed: 4,
                    len: bytes.len(),
                });
            };

            let got = seven_step_checksum(body);

            if got != expected {
                return Err(Error::structure(
                    kind,
                    format!("checksum B mismatch: trailer {expected:#010x}, computed {got:#010x}"),
                ));
            }
        }
        Integrity::IgnoredTrailer | Integrity::None => {}
    }

    let mut out = body.to_vec();

    xor_5a(&mut out);

    Ok(out)
}
