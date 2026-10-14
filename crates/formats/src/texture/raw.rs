use super::{MipLevel, PixelFormat, Texture};
use crate::error::{Error, Kind, Result};
use image::{DynamicImage, ImageDecoder, ImageFormat, ImageReader, Limits};
use std::io::Cursor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Bmp,
    Jpeg,
    Ppm,
    Hdr,
    Dib,
    Pfm,
    Tga,
}

pub fn sniff(bytes: &[u8]) -> Option<Format> {
    if bytes.starts_with(b"BM") {
        Some(Format::Bmp)
    } else if bytes.starts_with(b"\xff\xd8\xff") {
        Some(Format::Jpeg)
    } else if bytes.starts_with(b"#?RADIANCE") || bytes.starts_with(b"#?RGBE") {
        Some(Format::Hdr)
    } else if bytes.len() >= 3
        && bytes[2].is_ascii_whitespace()
        && matches!(&bytes[..2], b"P3" | b"P6")
    {
        Some(Format::Ppm)
    } else if bytes.len() >= 3
        && bytes[2].is_ascii_whitespace()
        && matches!(&bytes[..2], b"PF" | b"Pf")
    {
        Some(Format::Pfm)
    } else if bytes.get(..4).is_some_and(|b| {
        matches!(
            u32::from_le_bytes(b.try_into().unwrap()),
            12 | 40 | 52 | 56 | 108 | 124
        )
    }) {
        Some(Format::Dib)
    } else {
        None
    }
}

fn bad(message: impl ToString) -> Error {
    Error::Structure {
        kind: Kind::Image,
        message: message.to_string(),
    }
}

fn read(decoder: impl ImageDecoder) -> Result<DynamicImage> {
    let (w, h) = decoder.dimensions();

    let mut limits = Limits::default();

    limits.reserve(decoder.total_bytes()).map_err(bad)?;

    let rgba_bytes = u64::from(w)
        .checked_mul(u64::from(h))
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| bad("image dimensions overflow"))?;

    limits.reserve(rgba_bytes).map_err(bad)?;

    if w == 0 || h == 0 {
        return Err(bad("zero-sized image"));
    }

    DynamicImage::from_decoder(decoder).map_err(bad)
}

pub fn decode(bytes: &[u8], format: Format) -> Result<Texture> {
    if format == Format::Pfm {
        return pfm(bytes);
    }

    let mut cursor = Cursor::new(bytes);

    let decoded = if format == Format::Dib {
        let mut decoder =
            image::codecs::bmp::BmpDecoder::new_without_file_header(&mut cursor).map_err(bad)?;

        decoder.set_limits(Limits::default()).map_err(bad)?;

        read(decoder)?
    } else {
        let format = match format {
            Format::Bmp => ImageFormat::Bmp,
            Format::Jpeg => ImageFormat::Jpeg,
            Format::Ppm => ImageFormat::Pnm,
            Format::Hdr => ImageFormat::Hdr,
            Format::Tga => ImageFormat::Tga,
            Format::Dib | Format::Pfm => unreachable!("handled above"),
        };

        let decoder = ImageReader::with_format(&mut cursor, format)
            .into_decoder()
            .map_err(bad)?;

        read(decoder)?
    };

    let rgba = decoded.into_rgba8();

    let (width, height) = rgba.dimensions();

    let data = rgba.into_raw();

    Ok(Texture {
        width,
        height,
        format: PixelFormat::Rgba8Unorm,
        mips: vec![MipLevel {
            width,
            height,
            offset: 0,
            len: data.len(),
        }],
        data,
        source_len: usize::try_from(cursor.position())
            .map_err(bad)?
            .min(bytes.len()),
    })
}

fn pfm(bytes: &[u8]) -> Result<Texture> {
    fn line<'a>(bytes: &'a [u8], offset: &mut usize) -> Result<&'a str> {
        loop {
            let rest = bytes
                .get(*offset..)
                .ok_or_else(|| bad("truncated PFM header"))?;

            let end = rest
                .iter()
                .position(|&b| b == b'\n')
                .ok_or_else(|| bad("truncated PFM line"))?;

            let s = std::str::from_utf8(&rest[..end]).map_err(bad)?.trim();

            *offset += end + 1;

            if !s.is_empty() && !s.starts_with('#') {
                return Ok(s);
            }
        }
    }

    let mut offset = 0;

    let channels = match line(bytes, &mut offset)? {
        "PF" => 3usize,
        "Pf" => 1,
        _ => return Err(bad("invalid PFM magic")),
    };

    let dimensions: Vec<_> = line(bytes, &mut offset)?.split_ascii_whitespace().collect();

    if dimensions.len() != 2 {
        return Err(bad("PFM requires width and height"));
    }

    let width = dimensions[0].parse::<u32>().map_err(bad)?;

    let height = dimensions[1].parse::<u32>().map_err(bad)?;

    let scale = line(bytes, &mut offset)?.parse::<f32>().map_err(bad)?;

    if width == 0 || height == 0 || !scale.is_finite() || scale == 0.0 {
        return Err(bad("invalid PFM dimensions or scale"));
    }

    let pixels = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| bad("PFM dimensions overflow"))?;

    let len = pixels
        .checked_mul(channels * 4)
        .ok_or_else(|| bad("PFM input size overflows"))?;

    let end = offset
        .checked_add(len)
        .ok_or_else(|| bad("PFM payload range overflows"))?;

    let input = bytes
        .get(offset..end)
        .ok_or_else(|| bad("truncated PFM pixels"))?;

    let output_len = pixels
        .checked_mul(4)
        .ok_or_else(|| bad("PFM output size overflows"))?;

    Limits::default().reserve(output_len as u64).map_err(bad)?;

    let mut data = Vec::new();

    data.try_reserve_exact(output_len).map_err(bad)?;

    data.resize(output_len, 0);

    for (index, pixel) in input.chunks_exact(channels * 4).enumerate() {
        let x = index % width as usize;

        let y = height as usize - 1 - index / width as usize;

        let at = (y * width as usize + x) * 4;

        let mut rgba = [0, 0, 0, 255];

        for (channel, bytes) in pixel.chunks_exact(4).enumerate() {
            let bytes = bytes.try_into().unwrap();

            let v = if scale < 0.0 {
                f32::from_le_bytes(bytes)
            } else {
                f32::from_be_bytes(bytes)
            };

            if !v.is_finite() {
                return Err(bad("nonfinite PFM component"));
            }

            rgba[channel] = (v * scale.abs() * 255.0).round().clamp(0.0, 255.0) as u8;
        }

        if channels == 1 {
            rgba[1] = rgba[0];

            rgba[2] = rgba[0];
        }

        data[at..at + 4].copy_from_slice(&rgba);
    }

    Ok(Texture {
        width,
        height,
        format: PixelFormat::Rgba8Unorm,
        mips: vec![MipLevel {
            width,
            height,
            offset: 0,
            len: data.len(),
        }],
        data,
        source_len: end,
    })
}
