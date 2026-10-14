use super::{MipLevel, PixelFormat, Texture};
use crate::error::{Error, Kind, Result};
use crate::util::u32_at;

pub const MAGIC: &[u8; 4] = b"WS10";

const DDS_TO_WYS: usize = 1;

const DDS_HEADER_LEN: usize = 128;

const PAYLOAD_START: usize = DDS_HEADER_LEN + DDS_TO_WYS;

const OFF_SIZE: usize = 0x04;

const OFF_HEIGHT: usize = 0x0C;

const OFF_WIDTH: usize = 0x10;

const OFF_DEPTH: usize = 0x18;

const OFF_MIPMAP_COUNT: usize = 0x1C;

const OFF_PF_FLAGS: usize = 0x50;

const OFF_FOURCC: usize = 0x54;

const OFF_CAPS2: usize = 0x70;

const DDPF_FOURCC: u32 = 0x4;

fn validate_levels(width: u32, height: u32, levels: u32) -> Result<()> {
    let max_levels = u32::BITS - width.max(height).leading_zeros();

    if levels == 0 || levels > max_levels {
        return Err(Error::Wys(format!(
            "{levels} mip levels exceed the chain for {width}x{height}"
        )));
    }

    Ok(())
}

fn mip_chain(
    width: u32,
    height: u32,
    block_bytes: usize,
    levels: u32,
) -> Result<(Vec<MipLevel>, usize)> {
    validate_levels(width, height, levels)?;

    let mut mips = Vec::with_capacity(levels as usize);

    let mut offset = 0usize;

    for i in 0..levels {
        let w = (width >> i).max(1);

        let h = (height >> i).max(1);

        let len = (w.div_ceil(4) as usize)
            .checked_mul(h.div_ceil(4) as usize)
            .and_then(|blocks| blocks.checked_mul(block_bytes))
            .ok_or_else(|| Error::Wys("mip byte size exceeds the address space".into()))?;

        let end = offset
            .checked_add(len)
            .ok_or_else(|| Error::Wys("mip chain exceeds the address space".into()))?;

        mips.push(MipLevel {
            width: w,
            height: h,
            offset,
            len,
        });

        offset = end;
    }

    Ok((mips, offset))
}

pub fn decode(wys: &[u8]) -> Result<Texture> {
    if !wys.starts_with(MAGIC) {
        let mut found = [0u8; 4];

        let n = wys.len().min(4);

        found[..n].copy_from_slice(&wys[..n]);

        return Err(Error::BadMagic {
            kind: Kind::Wys,
            expected: "WS10",
            found,
        });
    }

    decode_surface(wys, DDS_TO_WYS)
}

pub fn decode_dds(dds: &[u8]) -> Result<Texture> {
    if !dds.starts_with(b"DDS ") {
        let mut found = [0; 4];

        let n = dds.len().min(4);

        found[..n].copy_from_slice(&dds[..n]);

        return Err(Error::BadMagic {
            kind: Kind::Wys,
            expected: "DDS ",
            found,
        });
    }

    if u32_at(dds, 76, Kind::Wys)? != 32 {
        return Err(Error::Wys(
            "raw DDS pixel-format header size must be 32".into(),
        ));
    }

    decode_surface(dds, 0)
}

fn decode_surface(wys: &[u8], header_shift: usize) -> Result<Texture> {
    let dds_u32 = |offset| u32_at(wys, offset + header_shift, Kind::Wys);

    let header_size = dds_u32(OFF_SIZE)?;

    if header_size != 124 {
        return Err(Error::Wys(format!(
            "DDS_HEADER.dwSize is {header_size}, expected 124"
        )));
    }

    let pf_flags = dds_u32(OFF_PF_FLAGS)?;

    if header_shift != 0 && pf_flags & DDPF_FOURCC == 0 {
        return Err(Error::Wys(format!(
            "ddspf.dwFlags is {pf_flags:#x}, expected DDPF_FOURCC ({DDPF_FOURCC:#x}); \
             WYS requires a block-compressed payload"
        )));
    }

    let width = dds_u32(OFF_WIDTH)?;

    let height = dds_u32(OFF_HEIGHT)?;

    if width == 0 || height == 0 {
        return Err(Error::Wys(format!(
            "degenerate dimensions {width}x{height}"
        )));
    }

    let depth = dds_u32(OFF_DEPTH)?;

    let caps2 = dds_u32(OFF_CAPS2)?;

    if depth > 1 || caps2 != 0 {
        return Err(Error::Wys(format!(
            "unsupported DDS resource layout: depth {depth}, caps2 {caps2:#x}; expected a 2D texture"
        )));
    }

    let levels = dds_u32(OFF_MIPMAP_COUNT)?.max(1);

    validate_levels(width, height, levels)?;

    decode_payload(
        wys,
        header_shift,
        width,
        height,
        levels,
        payload_start(wys, header_shift)?,
    )
}

fn payload_start(bytes: &[u8], shift: usize) -> Result<usize> {
    let palette = shift == 0 && u32_at(bytes, OFF_PF_FLAGS, Kind::Wys)? & 0x20 != 0;

    Ok(DDS_HEADER_LEN + shift + if palette { 1024 } else { 0 })
}

fn decode_payload(
    wys: &[u8],
    header_shift: usize,
    width: u32,
    height: u32,
    levels: u32,
    payload_start: usize,
) -> Result<Texture> {
    let dds_u32 = |offset| u32_at(wys, offset + header_shift, Kind::Wys);

    let pf_flags = dds_u32(OFF_PF_FLAGS)?;

    if header_shift == 0 && pf_flags & DDPF_FOURCC == 0 {
        return super::dds_rgb::decode(wys, width, height, levels, payload_start);
    }

    let codec = if header_shift == 0 {
        let code = dds_u32(OFF_FOURCC)?;

        if matches!(code, 36 | 82 | 110..=116) {
            return super::dds_rgb::decode_numeric(wys, width, height, levels, code, payload_start);
        }

        if code == 117 {
            return super::dds_rgb::decode_vector(wys, width, height, levels, code, payload_start);
        }

        use super::dds_rgb::{Packed, decode_packed};

        match code.to_le_bytes() {
            [b'R', b'G', b'B', b'G'] => {
                return decode_packed(wys, width, height, levels, Packed::Rgbg, payload_start);
            }
            [b'G', b'R', b'G', b'B'] => {
                return decode_packed(wys, width, height, levels, Packed::Grgb, payload_start);
            }
            [b'U', b'Y', b'V', b'Y'] => {
                return decode_packed(wys, width, height, levels, Packed::Uyvy, payload_start);
            }
            [b'Y', b'U', b'Y', b'2'] => {
                return decode_packed(wys, width, height, levels, Packed::Yuy2, payload_start);
            }
            [b'D', b'X', b'T', n @ b'1'..=b'5'] => n - b'0',
            code => {
                return Err(Error::Wys(format!("unsupported raw DDS FourCC {code:?}")));
            }
        }
    } else {
        match crate::util::byte_at(wys, OFF_FOURCC + header_shift, Kind::Wys)? {
            b'2' => 1,
            _ => 3,
        }
    };

    let format = if codec == 1 {
        PixelFormat::Bc1RgbaUnorm
    } else {
        PixelFormat::Bc2RgbaUnorm
    };

    let block_bytes = format
        .block_bytes()
        .expect("wys always decodes to a block-compressed format");

    let (mips, expected) = mip_chain(width, height, block_bytes, levels)?;

    let payload = wys.get(payload_start..).ok_or(Error::Truncated {
        kind: Kind::Wys,
        offset: payload_start,
        needed: expected,
        len: wys.len(),
    })?;

    if payload.len() < expected {
        return Err(Error::Truncated {
            kind: Kind::Wys,
            offset: payload_start,
            needed: expected,
            len: wys.len(),
        });
    }

    let texture = Texture {
        width,
        height,
        format,
        mips,
        data: payload[..expected].to_vec(),
        source_len: payload_start + expected,
    };

    if header_shift == 0 && (matches!(codec, 2 | 4 | 5) || width % 4 != 0 || height % 4 != 0) {
        super::dxt::expand(texture, codec)
    } else {
        Ok(texture)
    }
}

pub fn decode_cube(bytes: &[u8]) -> Result<super::CubeTexture> {
    let shift = if bytes.starts_with(MAGIC) {
        1
    } else if bytes.starts_with(b"DDS ") {
        0
    } else {
        return Err(Error::Wys("cube input is not DDS or WYS".into()));
    };

    let word = |at| u32_at(bytes, at + shift, Kind::Wys);

    if word(OFF_SIZE)? != 124 || word(76)? != 32 {
        return Err(Error::Wys("invalid cube DDS header size".into()));
    }

    let size = word(OFF_WIDTH)?;

    if size == 0
        || word(OFF_HEIGHT)? != size
        || word(OFF_DEPTH)? > 1
        || word(8)? & 0x800000 != 0
        || word(OFF_CAPS2)? & !0x200 != 0xfc00
    {
        return Err(
            Error::Wys(
                "cube requires square faces and all six legacy face flags; native loading rejects partial cubes and volumes"
                    .into(),
            ),
        );
    }

    if shift != 0 && word(OFF_PF_FLAGS)? & DDPF_FOURCC == 0 {
        return Err(Error::Wys(
            "WYS cube requires a block-compressed payload".into(),
        ));
    }

    let levels = word(OFF_MIPMAP_COUNT)?.max(1);

    validate_levels(size, size, levels)?;

    let budget = (0..levels).try_fold(0u64, |sum, level| {
        let edge = u64::from((size >> level).max(1));

        sum.checked_add(edge.checked_mul(edge)?.checked_mul(24)?)
    });

    if budget.is_none_or(|bytes| bytes > 256 * 1024 * 1024) {
        return Err(Error::Wys(
            "cube exceeds the 256 MiB decoded resource budget".into(),
        ));
    }

    let mut cursor = payload_start(bytes, shift)?;

    let mut faces = Vec::with_capacity(6);

    for _ in 0..6 {
        let mut face = decode_payload(bytes, shift, size, size, levels, cursor)?;

        let end = face.source_len;

        face.source_len = end - cursor;

        cursor = end;

        if size % 4 != 0 {
            face = match face.format {
                PixelFormat::Bc1RgbaUnorm => super::dxt::expand(face, 1)?,
                PixelFormat::Bc2RgbaUnorm => super::dxt::expand(face, 3)?,
                PixelFormat::Rgba8Unorm => face,
            };
        }

        faces.push(face);
    }

    Ok(super::CubeTexture {
        faces: faces.try_into().expect("six decoded faces"),
        source_len: cursor,
    })
}

pub fn to_dds(wys: &[u8]) -> Result<Vec<u8>> {
    if !wys.starts_with(MAGIC) {
        let mut found = [0u8; 4];

        let n = wys.len().min(4);

        found[..n].copy_from_slice(&wys[..n]);

        return Err(Error::BadMagic {
            kind: Kind::Wys,
            expected: "WS10",
            found,
        });
    }

    if wys.len() < PAYLOAD_START {
        return Err(Error::Truncated {
            kind: Kind::Wys,
            offset: 0,
            needed: PAYLOAD_START,
            len: wys.len(),
        });
    }

    let mut dds = wys[DDS_TO_WYS..].to_vec();

    dds[0..3].copy_from_slice(b"DDS");

    let fourcc = if dds[OFF_FOURCC] == b'2' {
        b"DXT1"
    } else {
        b"DXT3"
    };

    dds[OFF_FOURCC..OFF_FOURCC + 4].copy_from_slice(fourcc);

    Ok(dds)
}
