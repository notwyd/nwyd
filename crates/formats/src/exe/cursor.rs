use super::resource::{RT_CURSOR, Resources};
use crate::error::{Error, Kind, Result};
use crate::util::{u16_at, u32_at};

const KIND: Kind = Kind::Executable;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cursor {
    pub width: u32,
    pub height: u32,
    pub hotspot_x: u16,
    pub hotspot_y: u16,
    pub rgba: Vec<u8>,
}

pub fn group_cursor(resources: &Resources<'_>, group_id: u32) -> Result<Cursor> {
    let group = resources
        .find(super::resource::RT_GROUP_CURSOR, group_id)?
        .ok_or_else(|| Error::structure(KIND, format!("no RT_GROUP_CURSOR {group_id}")))?;

    let kind = u16_at(group, 2, KIND)?;

    if kind != 2 {
        return Err(Error::structure(
            KIND,
            format!("resource group {group_id} has type {kind}, not a cursor"),
        ));
    }

    let count = u16_at(group, 4, KIND)?;

    if count == 0 {
        return Err(Error::structure(
            KIND,
            format!("RT_GROUP_CURSOR {group_id} lists no images"),
        ));
    }

    let ordinal = u16_at(group, 6 + 12, KIND)? as u32;

    let bytes = resources.find(RT_CURSOR, ordinal)?.ok_or_else(|| {
        Error::structure(
            KIND,
            format!("RT_GROUP_CURSOR {group_id} names RT_CURSOR {ordinal}, which is absent"),
        )
    })?;

    decode_cursor(bytes)
}

pub fn decode_cursor(bytes: &[u8]) -> Result<Cursor> {
    let hotspot_x = u16_at(bytes, 0, KIND)?;

    let hotspot_y = u16_at(bytes, 2, KIND)?;

    let dib = 4usize;

    let header_size = u32_at(bytes, dib, KIND)? as usize;

    if header_size < 40 {
        return Err(Error::structure(
            KIND,
            format!("cursor DIB header is {header_size} bytes, not a BITMAPINFOHEADER"),
        ));
    }

    let width = u32_at(bytes, dib + 4, KIND)? as i32;

    let doubled = u32_at(bytes, dib + 8, KIND)? as i32;

    let bpp = u16_at(bytes, dib + 14, KIND)?;

    let compression = u32_at(bytes, dib + 16, KIND)?;

    let palette_len = u32_at(bytes, dib + 32, KIND)? as usize;

    if compression != 0 {
        return Err(Error::structure(
            KIND,
            format!("cursor DIB uses compression {compression}, not BI_RGB"),
        ));
    }

    if !matches!(bpp, 1 | 4 | 8) {
        return Err(Error::structure(
            KIND,
            format!("cursor DIB is {bpp} bits per pixel; only indexed 1, 4 and 8 are read"),
        ));
    }

    if width <= 0 || doubled <= 0 || doubled % 2 != 0 {
        return Err(Error::structure(
            KIND,
            format!("cursor DIB is {width}x{doubled}, which is not a stacked colour-and-mask pair"),
        ));
    }

    let width = width as usize;

    let height = (doubled / 2) as usize;

    let palette_len = if palette_len == 0 {
        1usize << bpp
    } else {
        palette_len
    };

    let palette_at = dib + header_size;

    let colour_at = palette_at + 4 * palette_len;

    let colour_stride = ((width * bpp as usize + 31) / 32) * 4;

    let mask_at = colour_at + colour_stride * height;

    let mask_stride = ((width + 31) / 32) * 4;

    let needed = mask_at + mask_stride * height;

    if bytes.len() < needed {
        return Err(Error::Truncated {
            kind: KIND,
            offset: mask_at,
            needed,
            len: bytes.len(),
        });
    }

    let mut rgba = vec![0u8; width * height * 4];

    for y in 0..height {
        let source = height - 1 - y;

        let colour_row = colour_at + source * colour_stride;

        let mask_row = mask_at + source * mask_stride;

        for x in 0..width {
            let masked = (bytes[mask_row + (x >> 3)] >> (7 - (x & 7))) & 1 == 1;

            let out = (y * width + x) * 4;

            if masked {
                continue;
            }

            let index = index_at(bytes, colour_row, x, bpp);

            let entry = palette_at + 4 * index;

            if index >= palette_len || bytes.len() < entry + 4 {
                return Err(Error::structure(
                    KIND,
                    format!("cursor pixel names palette entry {index} of {palette_len}"),
                ));
            }

            rgba[out] = bytes[entry + 2];

            rgba[out + 1] = bytes[entry + 1];

            rgba[out + 2] = bytes[entry];

            rgba[out + 3] = 0xFF;
        }
    }

    Ok(Cursor {
        width: width as u32,
        height: height as u32,
        hotspot_x,
        hotspot_y,
        rgba,
    })
}

fn index_at(bytes: &[u8], row: usize, x: usize, bpp: u16) -> usize {
    match bpp {
        1 => ((bytes[row + (x >> 3)] >> (7 - (x & 7))) & 1) as usize,
        4 => {
            let byte = bytes[row + (x >> 1)];

            (if x & 1 == 0 { byte >> 4 } else { byte & 0x0F }) as usize
        }
        _ => bytes[row + x] as usize,
    }
}
