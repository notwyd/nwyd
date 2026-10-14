use crate::error::{Error, Kind, Result};

pub const LEN: usize = 8_192;

pub const WORDS: usize = LEN / 4;

pub const CHECKSUM: i32 = 0x238B_755C_u32 as i32;

pub const GUARDED_SCENE: i32 = 30_000;

pub const EMPTY: i32 = 1_024_000;

pub fn checksum(words: &[i32]) -> i32 {
    let mut sum: i32 = 0;

    let mut k: i32 = 8;

    let mut base: i32 = 8;

    let mut p = 0usize;

    loop {
        let (mut a, mut b, mut c) = (0i32, 0i32, 0i32);

        for _ in 0..8 {
            let w = |i: usize| words.get(i).copied().unwrap_or(0);

            sum = sum.wrapping_add(-0x20 + w(p).wrapping_add(k).wrapping_mul(4));

            c = c.wrapping_add(-0x10 + w(p + 1).wrapping_add(k).wrapping_mul(4));

            a = a.wrapping_add(w(p + 2).wrapping_add(k).wrapping_mul(4));

            b = b.wrapping_add(0x10 + w(p + 3).wrapping_add(k).wrapping_mul(4));

            p += 4;

            k = k.wrapping_add(0x10);
        }

        sum = sum.wrapping_add(a).wrapping_add(b).wrapping_add(c);

        k = base + 2;

        if k > 0x87 {
            break;
        }

        base = k;
    }

    sum
}

pub fn parse(bytes: &[u8]) -> Result<Vec<i32>> {
    if bytes.len() < LEN {
        return Err(Error::Truncated {
            kind: Kind::CData,
            offset: 0,
            needed: LEN,
            len: bytes.len(),
        });
    }

    if bytes.len() > LEN {
        return Err(Error::structure(
            Kind::CData,
            format!("{} bytes, expected exactly {LEN}", bytes.len()),
        ));
    }

    Ok(bytes
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes(c.try_into().expect("chunk is 4 bytes")))
        .collect())
}

pub fn verify(bytes: &[u8]) -> Result<()> {
    let words = parse(bytes)?;

    let got = checksum(&words);

    if got != CHECKSUM {
        return Err(Error::structure(
            Kind::CData,
            format!("checksum mismatch: expected {CHECKSUM:#010x}, computed {got:#010x}"),
        ));
    }

    Ok(())
}
