use super::{MipLevel, PixelFormat, Texture};
use crate::error::{Error, Kind, Result};
use crate::util::u32_at;

fn channel(pixel: u32, mask: u32, missing: u8) -> u8 {
    if mask == 0 {
        return missing;
    }

    let shift = mask.trailing_zeros();

    let max = (mask >> shift) as u64;

    ((((pixel & mask) >> shift) as u64 * 255 + max / 2) / max) as u8
}

pub(super) fn decode(
    bytes: &[u8],
    width: u32,
    height: u32,
    levels: u32,
    start: usize,
) -> Result<Texture> {
    let flags = u32_at(bytes, 80, Kind::Wys)?;

    let bits = u32_at(bytes, 88, Kind::Wys)?;

    if flags & 0x20 != 0 {
        return decode_palette(bytes, width, height, levels, flags, bits, start);
    }

    if flags & (0x40000 | 0x80000) != 0 {
        let masks = [
            u32_at(bytes, 92, Kind::Wys)?,
            u32_at(bytes, 96, Kind::Wys)?,
            u32_at(bytes, 100, Kind::Wys)?,
            u32_at(bytes, 104, Kind::Wys)?,
        ];

        let code = match (flags, bits, masks) {
            (0x80000, 16, [0xff, 0xff00, 0, 0]) => 60,
            (0x40000, 16, [0x1f, 0x3e0, 0xfc00, 0]) => 61,
            (0x40000, 32, [0xff, 0xff00, 0xff0000, 0]) => 62,
            (0x80000, 32, [0xff, 0xff00, 0xff0000, 0xff000000]) => 63,
            (0x80000, 32, [0xffff, 0xffff0000, 0, 0]) => 64,
            (0x80001, 32, [0x3ff00000, 0xffc00, 0x3ff, 0xc0000000]) => 67,
            _ => return Err(Error::Wys("unsupported vector DDS pixel layout".into())),
        };

        return decode_vector(bytes, width, height, levels, code, start);
    }

    if !matches!(bits, 8 | 16 | 24 | 32) {
        return Err(Error::Wys(format!("unsupported DDS bit count {bits}")));
    }

    let depth = flags == 0x400;

    let mode = if depth {
        0x20000
    } else {
        flags & (0x40 | 0x20000 | 2)
    };

    if !matches!(mode, 0x40 | 0x20000 | 2) || (!depth && flags & !(0x40 | 0x20000 | 3) != 0) {
        return Err(Error::Wys(format!(
            "unsupported uncompressed DDS flags {flags:#x}"
        )));
    }

    let mut masks = [
        u32_at(bytes, 92, Kind::Wys)?,
        u32_at(bytes, 96, Kind::Wys)?,
        u32_at(bytes, 100, Kind::Wys)?,
        if flags & 3 != 0 {
            u32_at(bytes, 104, Kind::Wys)?
        } else {
            0
        },
    ];

    if depth {
        if bits != 16 || masks != [0, 0xffff, 0, 0] || u32_at(bytes, 104, Kind::Wys)? != 0 {
            return Err(Error::Wys("invalid D16 DDS pixel layout".into()));
        }

        masks = [0xffff, 0, 0, 0];
    }

    let mut occupied = 0u32;

    for mask in masks {
        if mask == 0 {
            continue;
        }

        let shifted = mask >> mask.trailing_zeros();

        if (mask as u64) >> bits != 0
            || shifted & (shifted.wrapping_add(1)) != 0
            || mask & occupied != 0
        {
            return Err(Error::Wys(
                "DDS masks overlap, are noncontiguous or exceed their pixel".into(),
            ));
        }

        occupied |= mask;
    }

    if (mode == 2 && (masks[..3] != [0; 3] || masks[3] == 0))
        || (mode != 2 && masks[0] == 0)
        || (mode == 0x20000 && (masks[1] != 0 || masks[2] != 0))
        || (flags & 1 != 0 && masks[3] == 0)
    {
        return Err(Error::Wys(
            "DDS channel masks do not describe the declared format".into(),
        ));
    }

    let bpp = bits as usize / 8;

    let (mips, source_end, total) = layout_at(bytes, width, height, levels, bpp, start)?;

    let mut data = Vec::new();

    data.try_reserve_exact(total)
        .map_err(|_| Error::Wys("expanded DDS allocation failed".into()))?;

    for px in bytes[start..source_end].chunks_exact(bpp) {
        let mut word = [0; 4];

        word[..bpp].copy_from_slice(px);

        let word = u32::from_le_bytes(word);

        let r = channel(word, masks[0], 255);

        let g = if mode == 0x20000 {
            r
        } else {
            channel(word, masks[1], 255)
        };

        let b = if mode == 0x20000 {
            r
        } else {
            channel(word, masks[2], 255)
        };

        data.extend_from_slice(&[r, g, b, channel(word, masks[3], 255)]);
    }

    Ok(Texture {
        width,
        height,
        format: PixelFormat::Rgba8Unorm,
        mips,
        data,
        source_len: source_end,
    })
}

fn layout_at(
    bytes: &[u8],
    width: u32,
    height: u32,
    levels: u32,
    bpp: usize,
    start: usize,
) -> Result<(Vec<MipLevel>, usize, usize)> {
    let mut source_end = start;

    let mut total = 0usize;

    let mut mips = Vec::new();

    for level in 0..levels {
        let w = (width >> level).max(1);

        let h = (height >> level).max(1);

        let pixels = (w as usize)
            .checked_mul(h as usize)
            .ok_or_else(|| Error::Wys("DDS pixel count exceeds the address space".into()))?;

        let stored = pixels
            .checked_mul(bpp)
            .ok_or_else(|| Error::Wys("DDS pixel bytes overflow".into()))?;

        let len = pixels
            .checked_mul(4)
            .ok_or_else(|| Error::Wys("expanded DDS pixel bytes overflow".into()))?;

        source_end = source_end
            .checked_add(stored)
            .ok_or_else(|| Error::Wys("DDS mip chain overflows".into()))?;

        mips.push(MipLevel {
            width: w,
            height: h,
            offset: total,
            len,
        });

        total = total
            .checked_add(len)
            .ok_or_else(|| Error::Wys("expanded DDS mip chain overflows".into()))?;
    }

    if source_end > bytes.len() {
        return Err(Error::Truncated {
            kind: Kind::Wys,
            offset: start,
            needed: source_end - start,
            len: bytes.len(),
        });
    }

    Ok((mips, source_end, total))
}

fn decode_palette(
    bytes: &[u8],
    width: u32,
    height: u32,
    levels: u32,
    flags: u32,
    bits: u32,
    start: usize,
) -> Result<Texture> {
    let alpha = flags == 0x21;

    if !matches!((flags, bits), (0x20, 8) | (0x21, 16))
        || [
            u32_at(bytes, 92, Kind::Wys)?,
            u32_at(bytes, 96, Kind::Wys)?,
            u32_at(bytes, 100, Kind::Wys)?,
        ] != [0; 3]
        || u32_at(bytes, 104, Kind::Wys)? != if alpha { 0xff00 } else { 0 }
    {
        return Err(Error::Wys("invalid palette DDS pixel layout".into()));
    }

    let palette = bytes.get(128..1152).ok_or(Error::Truncated {
        kind: Kind::Wys,
        offset: 128,
        needed: 1024,
        len: bytes.len(),
    })?;

    let bpp = if alpha { 2 } else { 1 };

    let (mips, source_end, total) = layout_at(bytes, width, height, levels, bpp, start)?;

    let mut data = Vec::new();

    data.try_reserve_exact(total)
        .map_err(|_| Error::Wys("expanded palette DDS allocation failed".into()))?;

    for pixel in bytes[start..source_end].chunks_exact(bpp) {
        let at = usize::from(pixel[0]) * 4;

        let entry = &palette[at..at + 4];

        data.extend_from_slice(&[
            entry[0],
            entry[1],
            entry[2],
            if alpha { pixel[1] } else { entry[3] },
        ]);
    }

    Ok(Texture {
        width,
        height,
        format: PixelFormat::Rgba8Unorm,
        mips,
        data,
        source_len: source_end,
    })
}

pub(super) fn decode_vector(
    bytes: &[u8],
    width: u32,
    height: u32,
    levels: u32,
    code: u32,
    start: usize,
) -> Result<Texture> {
    let bpp = match code {
        60 | 61 | 117 => 2,
        62 | 63 | 64 | 67 => 4,
        _ => return Err(Error::Wys(format!("unsupported vector DDS format {code}"))),
    };

    let (mips, source_end, total) = layout_at(bytes, width, height, levels, bpp, start)?;

    let mut data = Vec::new();

    data.try_reserve_exact(total)
        .map_err(|_| Error::Wys("expanded vector DDS allocation failed".into()))?;

    let signed = |word: u32, shift: u32, bits: u32| {
        let value = ((word >> shift) << (32 - bits)) as i32 >> (32 - bits);

        (value as f32 / ((1u32 << (bits - 1)) - 1) as f32).max(-1.0)
    };

    let display = |value: f32| ((value + 1.0) * 127.5).round() as u8;

    for pixel in bytes[start..source_end].chunks_exact(bpp) {
        let mut word = [0; 4];

        word[..bpp].copy_from_slice(pixel);

        let word = u32::from_le_bytes(word);

        let rgba = match code {
            61 => [
                display(signed(word, 0, 5)),
                display(signed(word, 5, 5)),
                255,
                channel(word, 0xfc00, 255),
            ],
            62 => [
                display(signed(word, 0, 8)),
                display(signed(word, 8, 8)),
                255,
                pixel[2],
            ],
            63 => [
                display(signed(word, 0, 8)),
                display(signed(word, 8, 8)),
                display(signed(word, 16, 8)),
                display(signed(word, 24, 8)),
            ],
            64 => [
                display(signed(word, 0, 16)),
                display(signed(word, 16, 16)),
                255,
                255,
            ],
            67 => [
                display(signed(word, 0, 10)),
                display(signed(word, 10, 10)),
                display(signed(word, 20, 10)),
                channel(word, 0xc0000000, 255),
            ],
            _ => {
                let u = signed(word, 0, 8);

                let v = signed(word, 8, 8);

                let w = if code == 117 {
                    (1.0 - u * u - v * v).max(0.0).sqrt()
                } else {
                    1.0
                };

                [display(u), display(v), display(w), 255]
            }
        };

        data.extend_from_slice(&rgba);
    }

    Ok(Texture {
        width,
        height,
        format: PixelFormat::Rgba8Unorm,
        mips,
        data,
        source_len: source_end,
    })
}

pub(super) fn decode_numeric(
    bytes: &[u8],
    width: u32,
    height: u32,
    levels: u32,
    code: u32,
    start: usize,
) -> Result<Texture> {
    let (channels, component_bytes) = match code {
        36 | 110 | 113 => (4, 2),
        111 => (1, 2),
        112 => (2, 2),
        82 | 114 => (1, 4),
        115 => (2, 4),
        116 => (4, 4),
        _ => return Err(Error::Wys(format!("unsupported numeric DDS FourCC {code}"))),
    };

    let bpp = channels * component_bytes;

    let (mips, source_end, total) = layout_at(bytes, width, height, levels, bpp, start)?;

    let mut data = Vec::new();

    data.try_reserve_exact(total)
        .map_err(|_| Error::Wys("expanded DDS allocation failed".into()))?;

    for pixel in bytes[start..source_end].chunks_exact(bpp) {
        let mut rgba = [255; 4];

        for (channel, bytes) in rgba.iter_mut().zip(pixel.chunks_exact(component_bytes)) {
            let value = match code {
                36 => u16::from_le_bytes([bytes[0], bytes[1]]) as f32 / 65535.0,
                110 => {
                    ((i16::from_le_bytes([bytes[0], bytes[1]]) as f32 / 32767.0).max(-1.0) + 1.0)
                        * 0.5
                }
                111..=113 => half(u16::from_le_bytes([bytes[0], bytes[1]])),
                _ => f32::from_le_bytes(bytes.try_into().expect("four-byte float chunk")),
            };

            if !value.is_finite() {
                return Err(Error::Wys("non-finite DDS float component".into()));
            }

            *channel = (value.clamp(0.0, 1.0) * 255.0).round() as u8;
        }

        data.extend_from_slice(&rgba);
    }

    Ok(Texture {
        width,
        height,
        format: PixelFormat::Rgba8Unorm,
        mips,
        data,
        source_len: source_end,
    })
}

fn half(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;

    let exponent = (bits >> 10) & 31;

    let fraction = bits & 1023;

    if exponent == 0 {
        let magnitude = f32::from(fraction) / 16777216.0;

        if sign == 0 { magnitude } else { -magnitude }
    } else {
        let exponent = if exponent == 31 {
            255
        } else {
            u32::from(exponent) + 112
        };

        f32::from_bits(sign | (exponent << 23) | (u32::from(fraction) << 13))
    }
}

#[derive(Clone, Copy)]

pub(super) enum Packed {
    Rgbg,
    Grgb,
    Uyvy,
    Yuy2,
}

fn yuv(y: u8, u: u8, v: u8) -> [u8; 4] {
    let c = i32::from(y) - 16;

    let d = i32::from(u) - 128;

    let e = i32::from(v) - 128;

    let channel = |n: i32| ((n + 128) >> 8).clamp(0, 255) as u8;

    [
        channel(298 * c + 409 * e),
        channel(298 * c - 100 * d - 208 * e),
        channel(298 * c + 516 * d),
        255,
    ]
}

pub(super) fn decode_packed(
    bytes: &[u8],
    width: u32,
    height: u32,
    levels: u32,
    format: Packed,
    start: usize,
) -> Result<Texture> {
    let mut source_end = start;

    let mut total = 0usize;

    let mut mips = Vec::new();

    for level in 0..levels {
        let w = (width >> level).max(1);

        let h = (height >> level).max(1);

        let stored = (w.div_ceil(2) as usize)
            .checked_mul(4)
            .and_then(|pitch| pitch.checked_mul(h as usize))
            .ok_or_else(|| Error::Wys("packed DDS mip bytes overflow".into()))?;

        let len = (w as usize)
            .checked_mul(h as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| Error::Wys("expanded packed DDS mip bytes overflow".into()))?;

        source_end = source_end
            .checked_add(stored)
            .ok_or_else(|| Error::Wys("packed DDS mip chain overflows".into()))?;

        mips.push(MipLevel {
            width: w,
            height: h,
            offset: total,
            len,
        });

        total = total
            .checked_add(len)
            .ok_or_else(|| Error::Wys("expanded packed DDS mip chain overflows".into()))?;
    }

    if source_end > bytes.len() {
        return Err(Error::Truncated {
            kind: Kind::Wys,
            offset: start,
            needed: source_end - start,
            len: bytes.len(),
        });
    }

    let mut data = Vec::new();

    data.try_reserve_exact(total)
        .map_err(|_| Error::Wys("expanded packed DDS allocation failed".into()))?;

    let mut source = start;

    for mip in &mips {
        for _ in 0..mip.height {
            for pair in 0..mip.width.div_ceil(2) {
                let p = &bytes[source..source + 4];

                source += 4;

                let pixels = match format {
                    Packed::Rgbg => [[p[0], p[1], p[2], 255], [p[0], p[3], p[2], 255]],
                    Packed::Grgb => [[p[1], p[0], p[3], 255], [p[1], p[2], p[3], 255]],
                    Packed::Uyvy => [yuv(p[1], p[0], p[2]), yuv(p[3], p[0], p[2])],
                    Packed::Yuy2 => [yuv(p[0], p[1], p[3]), yuv(p[2], p[1], p[3])],
                };

                data.extend_from_slice(&pixels[0]);

                if pair * 2 + 1 < mip.width {
                    data.extend_from_slice(&pixels[1]);
                }
            }
        }
    }

    Ok(Texture {
        width,
        height,
        format: PixelFormat::Rgba8Unorm,
        mips,
        data,
        source_len: source_end,
    })
}
