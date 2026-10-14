use crate::error::{Error, Kind, Result};

pub const LEN: usize = 1_000;

pub const MAX_ENTRIES: usize = 100;

pub const RECORD_LEN: usize = 10;

pub const ENTITY_ID_BASE: i32 = 0x3A99;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Entry {
    pub x: i16,
    pub y: i16,
    pub item_id: i16,
    pub direction: i16,
    pub b8: i16,
}

pub fn deobfuscate(buf: &mut [u8]) {
    for b in buf {
        *b = !*b;
    }
}

pub fn parse(bytes: &[u8]) -> Result<Vec<Entry>> {
    if bytes.len() != LEN {
        return Err(Error::structure(
            Kind::InitItem,
            format!("expected exactly {LEN} bytes, found {}", bytes.len()),
        ));
    }

    let mut buf = bytes.to_vec();

    deobfuscate(&mut buf);

    let mut out = Vec::new();

    for i in 0..MAX_ENTRIES {
        let r = &buf[i * RECORD_LEN..(i + 1) * RECORD_LEN];

        let f = |o: usize| i16::from_le_bytes([r[o], r[o + 1]]);

        let entry = Entry {
            x: f(0),
            y: f(2),
            item_id: f(4),
            direction: f(6),
            b8: f(8),
        };

        if entry.x < 1 {
            break;
        }

        out.push(entry);
    }

    Ok(out)
}

pub fn to_bytes(entries: &[Entry]) -> Vec<u8> {
    let mut out = vec![0u8; LEN];

    for (i, e) in entries.iter().take(MAX_ENTRIES).enumerate() {
        let r = &mut out[i * RECORD_LEN..(i + 1) * RECORD_LEN];

        for (o, v) in [
            (0, e.x),
            (2, e.y),
            (4, e.item_id),
            (6, e.direction),
            (8, e.b8),
        ] {
            r[o..o + 2].copy_from_slice(&v.to_le_bytes());
        }
    }

    deobfuscate(&mut out);

    out
}
