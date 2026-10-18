use super::{MipLevel, PixelFormat, Texture};
use crate::error::{Error, Result};

fn rgb565(word: u16) -> [u8; 4] {
    let r = ((word >> 11) & 31) as u8;

    let g = ((word >> 5) & 63) as u8;

    let b = (word & 31) as u8;

    [
        (r << 3) | (r >> 2),
        (g << 2) | (g >> 4),
        (b << 3) | (b >> 2),
        255,
    ]
}

fn block(bytes: &[u8], codec: u8) -> [[u8; 4]; 16] {
    let colour = if codec == 1 { bytes } else { &bytes[8..] };

    let c0 = u16::from_le_bytes([colour[0], colour[1]]);

    let c1 = u16::from_le_bytes([colour[2], colour[3]]);

    let mut palette = [rgb565(c0), rgb565(c1), [0; 4], [0; 4]];

    let opaque = codec != 1 || c0 > c1;

    for (channel, (a, b)) in palette[0].into_iter().zip(palette[1]).take(3).enumerate() {
        let a = a as u16;

        let b = b as u16;

        palette[2][channel] = if opaque {
            ((2 * a + b + 1) / 3) as u8
        } else {
            ((a + b) / 2) as u8
        };

        palette[3][channel] = if opaque {
            ((a + 2 * b + 1) / 3) as u8
        } else {
            0
        };
    }

    palette[2][3] = 255;

    palette[3][3] = if opaque { 255 } else { 0 };

    let indices = u32::from_le_bytes(colour[4..8].try_into().unwrap());

    let mut alpha = [0u8; 8];

    let mut alpha_indices = 0u64;

    if codec >= 4 {
        let a = bytes[0] as u16;

        let b = bytes[1] as u16;

        alpha[0] = a as u8;

        alpha[1] = b as u8;

        let denominator = if a > b { 7 } else { 5 };

        for i in 1..denominator {
            alpha[i as usize + 1] =
                (((denominator - i) * a + i * b + denominator / 2) / denominator) as u8;
        }

        if a <= b {
            alpha[6] = 0;

            alpha[7] = 255;
        }

        for (i, &byte) in bytes[2..8].iter().enumerate() {
            alpha_indices |= (byte as u64) << (i * 8);
        }
    }

    std::array::from_fn(|i| {
        let mut rgba = palette[((indices >> (i * 2)) & 3) as usize];

        if matches!(codec, 2 | 3) {
            rgba[3] = ((bytes[i / 2] >> ((i % 2) * 4)) & 15) * 17;
        }

        if codec >= 4 {
            rgba[3] = alpha[((alpha_indices >> (i * 3)) & 7) as usize];
        }

        if matches!(codec, 2 | 4) {
            let a = rgba[3] as u16;

            for c in &mut rgba[..3] {
                *c = (*c as u16 * 255 + a / 2)
                    .checked_div(a)
                    .unwrap_or(0)
                    .min(255) as u8;
            }
        }

        rgba
    })
}

pub(super) fn pixel(bytes: &[u8], codec: u8, x: u32, y: u32) -> [u8; 4] {
    block(bytes, codec)[(y * 4 + x) as usize]
}

pub(super) fn expand(source: Texture, codec: u8) -> Result<Texture> {
    if !(1..=5).contains(&codec) {
        return Err(Error::Wys("unsupported DXT codec".into()));
    }

    let stride = if codec == 1 { 8 } else { 16 };

    let mut mips = Vec::new();

    let mut total = 0usize;

    for m in &source.mips {
        let len = (m.width as usize)
            .checked_mul(m.height as usize)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(|| Error::Wys("expanded DXT mip exceeds the address space".into()))?;

        let next = total
            .checked_add(len)
            .ok_or_else(|| Error::Wys("expanded DXT chain overflows".into()))?;

        let stored = (m.width.div_ceil(4) as usize)
            .checked_mul(m.height.div_ceil(4) as usize)
            .and_then(|n| n.checked_mul(stride))
            .ok_or_else(|| Error::Wys("DXT block count overflows".into()))?;

        if m.len != stored
            || m.offset
                .checked_add(m.len)
                .is_none_or(|end| end > source.data.len())
        {
            return Err(Error::Wys("truncated DXT mip".into()));
        }

        mips.push(MipLevel {
            width: m.width,
            height: m.height,
            offset: total,
            len,
        });

        total = next;
    }

    let mut data = Vec::new();

    data.try_reserve_exact(total)
        .map_err(|_| Error::Wys("expanded DXT allocation failed".into()))?;

    data.resize(total, 0);

    for (stored, m) in source.mips.iter().zip(&mips) {
        let bw = m.width.div_ceil(4) as usize;

        for (index, bytes) in source.data[stored.offset..stored.offset + stored.len]
            .chunks_exact(stride)
            .enumerate()
        {
            let bx = index % bw;

            let by = index / bw;

            let decoded = block(bytes, codec);

            for (i, rgba) in decoded.iter().enumerate() {
                let x = bx * 4 + i % 4;

                let y = by * 4 + i / 4;

                if x < m.width as usize && y < m.height as usize {
                    let at = m.offset + (y * m.width as usize + x) * 4;

                    data[at..at + 4].copy_from_slice(rgba);
                }
            }
        }
    }

    Ok(Texture {
        width: source.width,
        height: source.height,
        format: PixelFormat::Rgba8Unorm,
        mips,
        data,
        source_len: source.source_len,
    })
}
