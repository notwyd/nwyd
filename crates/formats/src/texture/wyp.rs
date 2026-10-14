use super::{MipLevel, PixelFormat, Texture};
use crate::error::{Error, Result};

pub const CIPHERTEXT_PREFIX: &[u8; 16] = &[
    0x97, 0xe8, 0x38, 0x82, 0x93, 0xa2, 0xc2, 0xd3, 0xdb, 0xe4, 0xee, 0x06, 0x5b, 0xb0, 0xf4, 0x47,
];

const KEY_PERIOD: usize = 14;

pub fn decrypt(mut buf: Vec<u8>) -> Vec<u8> {
    if buf.is_empty() {
        return buf;
    }

    for i in (1..buf.len()).rev() {
        let prev = buf[i - 1];

        buf[i] = buf[i]
            .wrapping_sub(prev)
            .wrapping_sub((i % KEY_PERIOD) as u8);
    }

    buf[0] = buf[0].wrapping_sub(KEY_PERIOD as u8);

    buf
}

pub fn encrypt(mut buf: Vec<u8>) -> Vec<u8> {
    if buf.is_empty() {
        return buf;
    }

    buf[0] = buf[0].wrapping_add(KEY_PERIOD as u8);

    for i in 1..buf.len() {
        let prev = buf[i - 1];

        buf[i] = buf[i]
            .wrapping_add(prev)
            .wrapping_add((i % KEY_PERIOD) as u8);
    }

    buf
}

pub fn decode(wyp: &[u8]) -> Result<Texture> {
    let payload = decrypt(wyp.to_vec());

    if !payload.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Err(Error::Wyp(
            "decrypted payload is not a PNG — the cipher or the file is wrong".to_string(),
        ));
    }

    decode_png(&payload)
}

pub fn decode_png(payload: &[u8]) -> Result<Texture> {
    let mut decoder = png::Decoder::new(std::io::Cursor::new(payload));

    decoder.set_transformations(png::Transformations::STRIP_16 | png::Transformations::EXPAND);

    let mut reader = decoder.read_info()?;

    let mut raw = vec![0u8; reader.output_buffer_size().unwrap_or(0)];

    let info = reader.next_frame(&mut raw)?;

    let (w, h) = (info.width, info.height);

    let pixels = (w as usize)
        .checked_mul(h as usize)
        .filter(|n| n.checked_mul(4).is_some())
        .ok_or_else(|| Error::Wyp("PNG dimensions overflow RGBA storage".into()))?;

    let data = match info.color_type {
        png::ColorType::Rgba => {
            raw.truncate(pixels * 4);

            raw
        }
        png::ColorType::Rgb => {
            let mut out = vec![0u8; pixels * 4];

            for (i, px) in raw.chunks_exact(3).take(pixels).enumerate() {
                out[i * 4..i * 4 + 4].copy_from_slice(&[px[0], px[1], px[2], 0xFF]);
            }

            out
        }
        png::ColorType::GrayscaleAlpha => {
            let mut out = vec![0u8; pixels * 4];

            for (i, px) in raw.chunks_exact(2).take(pixels).enumerate() {
                out[i * 4..i * 4 + 4].copy_from_slice(&[px[0], px[0], px[0], px[1]]);
            }

            out
        }
        png::ColorType::Grayscale => {
            let mut out = vec![0u8; pixels * 4];

            for (i, px) in raw.iter().take(pixels).enumerate() {
                out[i * 4..i * 4 + 4].copy_from_slice(&[*px, *px, *px, 0xFF]);
            }

            out
        }
        other => {
            return Err(Error::Wyp(format!(
                "unexpected PNG colour type {other:?} after EXPAND"
            )));
        }
    };

    Ok(Texture {
        width: w,
        height: h,
        source_len: payload.len(),
        format: PixelFormat::Rgba8Unorm,
        mips: vec![MipLevel {
            width: w,
            height: h,
            offset: 0,
            len: data.len(),
        }],
        data,
    })
}
