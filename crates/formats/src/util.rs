use crate::error::{Error, Kind, Result};

pub fn u32_at(buf: &[u8], off: usize, kind: Kind) -> Result<u32> {
    let end = off.checked_add(4).ok_or(Error::Truncated {
        kind,
        offset: off,
        needed: 4,
        len: buf.len(),
    })?;

    let bytes = buf.get(off..end).ok_or(Error::Truncated {
        kind,
        offset: off,
        needed: 4,
        len: buf.len(),
    })?;

    Ok(u32::from_le_bytes(
        bytes.try_into().expect("slice is 4 bytes"),
    ))
}

pub fn u16_at(buf: &[u8], off: usize, kind: Kind) -> Result<u16> {
    let end = off.checked_add(2).ok_or(Error::Truncated {
        kind,
        offset: off,
        needed: 2,
        len: buf.len(),
    })?;

    let bytes = buf.get(off..end).ok_or(Error::Truncated {
        kind,
        offset: off,
        needed: 2,
        len: buf.len(),
    })?;

    Ok(u16::from_le_bytes(
        bytes.try_into().expect("slice is 2 bytes"),
    ))
}

pub fn byte_at(buf: &[u8], off: usize, kind: Kind) -> Result<u8> {
    buf.get(off).copied().ok_or(Error::Truncated {
        kind,
        offset: off,
        needed: 1,
        len: buf.len(),
    })
}
