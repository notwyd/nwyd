use super::{MipLevel, PixelFormat, Texture};
use crate::error::{Error, Kind, Result};
use crate::util::{byte_at, u16_at};

pub const MAGIC: &[u8; 4] = b"WT10";

const TGA_START: usize = 4;

const TGA_HEADER_LEN: usize = 18;

const IMAGE_TYPE_TRUECOLOR: u8 = 2;

const IMAGE_TYPE_GRAYSCALE: u8 = 3;

const IMAGE_TYPE_RLE_TRUECOLOR: u8 = 10;

const DESCRIPTOR_TOP_ORIGIN: u8 = 0x20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TgaHeader {
    pub id_length: u8,
    pub colormap_type: u8,
    pub image_type: u8,
    pub cmap_first: u16,
    pub cmap_len: u16,
    pub cmap_bpp: u8,
    pub x_origin: u16,
    pub y_origin: u16,
    pub width: u16,
    pub height: u16,
    pub bits_per_pixel: u8,
    pub descriptor: u8,
}

impl TgaHeader {
    fn parse(wyt: &[u8]) -> Result<TgaHeader> {
        Self::parse_at(wyt, TGA_START)
    }

    fn parse_at(wyt: &[u8], start: usize) -> Result<TgaHeader> {
        let b = |off: usize| byte_at(wyt, start + off, Kind::Wyt);

        let s = |off: usize| u16_at(wyt, start + off, Kind::Wyt);

        Ok(TgaHeader {
            id_length: b(0)?,
            colormap_type: b(1)?,
            image_type: b(2)?,
            cmap_first: s(3)?,
            cmap_len: s(5)?,
            cmap_bpp: b(7)?,
            x_origin: s(8)?,
            y_origin: s(10)?,
            width: s(12)?,
            height: s(14)?,
            bits_per_pixel: b(16)?,
            descriptor: b(17)?,
        })
    }

    pub fn top_origin(&self) -> bool {
        self.descriptor & DESCRIPTOR_TOP_ORIGIN != 0
    }

    fn bytes_per_pixel(&self) -> usize {
        self.bits_per_pixel as usize / 8
    }
}

#[inline]
const fn expand5(v: u8) -> u8 {
    (v << 3) | (v >> 2)
}

#[inline]
fn to_rgba(src: &[u8], bpp: usize, grayscale: bool) -> [u8; 4] {
    match (bpp, grayscale) {
        (4, _) => [src[2], src[1], src[0], src[3]],
        (3, _) => [src[2], src[1], src[0], 0xFF],
        (2, _) => {
            let v = u16::from_le_bytes([src[0], src[1]]);

            [
                expand5(((v >> 10) & 0x1F) as u8),
                expand5(((v >> 5) & 0x1F) as u8),
                expand5((v & 0x1F) as u8),
                if v & 0x8000 != 0 { 0xFF } else { 0x00 },
            ]
        }
        (1, _) => [src[0], src[0], src[0], 0xFF],
        _ => [0, 0, 0, 0xFF],
    }
}

pub fn header(wyt: &[u8]) -> Result<TgaHeader> {
    if !wyt.starts_with(MAGIC) {
        let mut found = [0u8; 4];

        let n = wyt.len().min(4);

        found[..n].copy_from_slice(&wyt[..n]);

        return Err(Error::BadMagic {
            kind: Kind::Wyt,
            expected: "WT10",
            found,
        });
    }

    TgaHeader::parse(wyt)
}

pub fn decode(wyt: &[u8]) -> Result<Texture> {
    if !wyt.starts_with(MAGIC) {
        let mut found = [0u8; 4];

        let n = wyt.len().min(4);

        found[..n].copy_from_slice(&wyt[..n]);

        return Err(Error::BadMagic {
            kind: Kind::Wyt,
            expected: "WT10",
            found,
        });
    }

    decode_at(wyt, TGA_START)
}

pub fn decode_tga(tga: &[u8]) -> Result<Texture> {
    decode_at(tga, 0)
}

fn decode_at(wyt: &[u8], start: usize) -> Result<Texture> {
    let h = TgaHeader::parse_at(wyt, start)?;

    if (h.colormap_type == 1 && matches!(h.image_type, 1 | 9))
        || h.bits_per_pixel == 15
        || h.image_type == 11
        || (h.image_type == 3 && h.bits_per_pixel == 16)
    {
        let mut texture = super::raw::decode(&wyt[start..], super::raw::Format::Tga)?;

        texture.source_len += start;

        return Ok(texture);
    }

    if h.colormap_type != 0 || h.cmap_len != 0 {
        return Err(Error::Wyt(format!(
            "colour-mapped TGA is not implemented (colormap_type={}, cmap_len={})",
            h.colormap_type, h.cmap_len
        )));
    }

    if h.width == 0 || h.height == 0 {
        return Err(Error::Wyt(format!(
            "degenerate dimensions {}x{}",
            h.width, h.height
        )));
    }

    let bpp = h.bytes_per_pixel();

    let grayscale = h.image_type == IMAGE_TYPE_GRAYSCALE;

    match (h.image_type, h.bits_per_pixel) {
        (IMAGE_TYPE_TRUECOLOR, 16 | 24 | 32) => {}
        (IMAGE_TYPE_GRAYSCALE, 8) => {}
        (IMAGE_TYPE_RLE_TRUECOLOR, 16 | 24 | 32) => {}
        (t, b) => {
            return Err(Error::Wyt(format!(
                "unsupported TGA image_type={t} with {b} bits per pixel"
            )));
        }
    }

    let pixel_start = start + TGA_HEADER_LEN + h.id_length as usize;

    let src = wyt.get(pixel_start..).ok_or(Error::Truncated {
        kind: Kind::Wyt,
        offset: pixel_start,
        needed: 0,
        len: wyt.len(),
    })?;

    let (width, height) = (h.width as usize, h.height as usize);

    let pixels = width
        .checked_mul(height)
        .ok_or_else(|| Error::Wyt("TGA pixel count exceeds the address space".into()))?;

    let output_len = pixels
        .checked_mul(4)
        .ok_or_else(|| Error::Wyt("TGA pixel bytes exceed the address space".into()))?;

    let consumed = if h.image_type == IMAGE_TYPE_RLE_TRUECOLOR {
        decode_rle(src, bpp, grayscale, pixels, None)?
    } else {
        let needed = pixels
            .checked_mul(bpp)
            .ok_or_else(|| Error::Wyt("TGA source bytes exceed the address space".into()))?;

        if src.len() < needed {
            return Err(Error::Truncated {
                kind: Kind::Wyt,
                offset: pixel_start,
                needed,
                len: wyt.len(),
            });
        }

        needed
    };

    let mut out = Vec::new();

    out.try_reserve_exact(output_len)
        .map_err(|_| Error::Wyt("TGA pixel allocation failed".into()))?;

    out.resize(output_len, 0);

    if h.image_type == IMAGE_TYPE_RLE_TRUECOLOR {
        decode_rle(src, bpp, grayscale, pixels, Some(&mut out))?;
    } else {
        for (i, chunk) in src[..consumed].chunks_exact(bpp).enumerate() {
            out[i * 4..i * 4 + 4].copy_from_slice(&to_rgba(chunk, bpp, grayscale));
        }
    }

    if !h.top_origin() {
        flip_rows(&mut out, width * 4, height);
    }

    if h.descriptor & 0x10 != 0 {
        for row in out.chunks_exact_mut(width * 4) {
            for x in 0..width / 2 {
                let (left, right) = row.split_at_mut((width - 1 - x) * 4);

                left[x * 4..x * 4 + 4].swap_with_slice(&mut right[..4]);
            }
        }
    }

    Ok(Texture {
        width: h.width as u32,
        height: h.height as u32,
        format: PixelFormat::Rgba8Unorm,
        mips: vec![MipLevel {
            width: h.width as u32,
            height: h.height as u32,
            offset: 0,
            len: out.len(),
        }],
        data: out,
        source_len: pixel_start + consumed,
    })
}

fn decode_rle(
    src: &[u8],
    bpp: usize,
    grayscale: bool,
    pixels: usize,
    mut out: Option<&mut [u8]>,
) -> Result<usize> {
    let mut read = 0usize;

    let mut written = 0usize;

    while written < pixels {
        let header = *src.get(read).ok_or_else(|| {
            Error::Wyt(format!(
                "RLE stream ended after {written} of {pixels} pixels"
            ))
        })?;

        read += 1;

        let count = (header & 0x7F) as usize + 1;

        if written + count > pixels {
            return Err(Error::Wyt(format!(
                "RLE run of {count} overruns the image at pixel {written} of {pixels}"
            )));
        }

        if header & 0x80 != 0 {
            let chunk = src
                .get(read..read + bpp)
                .ok_or_else(|| Error::Wyt("RLE run packet is truncated".to_string()))?;

            read += bpp;

            let rgba = to_rgba(chunk, bpp, grayscale);

            if let Some(out) = out.as_deref_mut() {
                for i in 0..count {
                    out[(written + i) * 4..(written + i) * 4 + 4].copy_from_slice(&rgba);
                }
            }
        } else {
            let need = count * bpp;

            let chunk = src
                .get(read..read + need)
                .ok_or_else(|| Error::Wyt("RLE raw packet is truncated".to_string()))?;

            read += need;

            if let Some(out) = out.as_deref_mut() {
                for (i, px) in chunk.chunks_exact(bpp).enumerate() {
                    out[(written + i) * 4..(written + i) * 4 + 4]
                        .copy_from_slice(&to_rgba(px, bpp, grayscale));
                }
            }
        }

        written += count;
    }

    Ok(read)
}

fn flip_rows(buf: &mut [u8], stride: usize, height: usize) {
    for y in 0..height / 2 {
        let (top, bottom) = (y * stride, (height - 1 - y) * stride);

        let (a, b) = buf.split_at_mut(bottom);

        a[top..top + stride].swap_with_slice(&mut b[..stride]);
    }
}
