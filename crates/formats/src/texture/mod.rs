mod dds_rgb;
mod dxt;
pub mod raw;
pub mod texture_list;
pub mod texture_set_list;
pub mod wyp;
pub mod wys;
pub mod wyt;

use crate::error::{Error, Result};
pub use texture_list::{TextureList, djb2_lower};
pub use texture_set_list::TextureSetList;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum PixelFormat {
    Bc1RgbaUnorm,
    Bc2RgbaUnorm,
    Rgba8Unorm,
}

impl PixelFormat {
    pub const fn block_bytes(self) -> Option<usize> {
        match self {
            PixelFormat::Bc1RgbaUnorm => Some(8),
            PixelFormat::Bc2RgbaUnorm => Some(16),
            PixelFormat::Rgba8Unorm => None,
        }
    }

    pub const fn is_block_compressed(self) -> bool {
        self.block_bytes().is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct MipLevel {
    pub width: u32,
    pub height: u32,
    pub offset: usize,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Texture {
    pub width: u32,
    pub height: u32,
    pub format: PixelFormat,
    pub mips: Vec<MipLevel>,
    pub data: Vec<u8>,
    pub source_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct CubeTexture {
    pub faces: [Texture; 6],
    pub source_len: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum TextureResource {
    Image(Texture),
    Cube(Box<CubeTexture>),
}

impl TextureResource {
    pub fn width(&self) -> u32 {
        match self {
            Self::Image(t) => t.width,
            Self::Cube(t) => t.faces[0].width,
        }
    }

    pub fn height(&self) -> u32 {
        match self {
            Self::Image(t) => t.height,
            Self::Cube(t) => t.faces[0].height,
        }
    }

    pub fn source_len(&self) -> usize {
        match self {
            Self::Image(t) => t.source_len,
            Self::Cube(t) => t.source_len,
        }
    }
}

pub fn decode_resource(bytes: &[u8]) -> Result<TextureResource> {
    let shift = if bytes.starts_with(wys::MAGIC) {
        Some(1)
    } else if bytes.starts_with(b"DDS ") {
        Some(0)
    } else {
        None
    };

    if let Some(shift) = shift {
        if crate::util::u32_at(bytes, 8 + shift, crate::error::Kind::Wys)? & 0x800000 != 0 {
            return Err(Error::Wys(
                "volume images are not created by the native CTexture loader".into(),
            ));
        }

        if crate::util::u32_at(bytes, 112 + shift, crate::error::Kind::Wys)? & 0xfe00 != 0 {
            return wys::decode_cube(bytes).map(|cube| TextureResource::Cube(Box::new(cube)));
        }
    }

    decode_image(bytes).map(TextureResource::Image)
}

pub fn decode_resource_with_color_key(bytes: &[u8], argb: u32) -> Result<TextureResource> {
    let resource = decode_resource(bytes)?;

    if argb == 0 {
        return Ok(resource);
    }

    let rgba = [
        (argb >> 16) as u8,
        (argb >> 8) as u8,
        argb as u8,
        (argb >> 24) as u8,
    ];

    let mut remaining = 256 * 1024 * 1024usize;

    let mut keyed = |texture: Texture| -> Result<Texture> {
        for mip in &texture.mips {
            let bytes = (mip.width as usize)
                .checked_mul(mip.height as usize)
                .and_then(|n| n.checked_mul(4));

            remaining = bytes
                .and_then(|n| remaining.checked_sub(n))
                .ok_or_else(|| {
                    Error::Wys("color-key image exceeds the 256 MiB decoded resource budget".into())
                })?;
        }

        let mut texture = match texture.format {
            PixelFormat::Bc1RgbaUnorm => dxt::expand(texture, 1)?,
            PixelFormat::Bc2RgbaUnorm => dxt::expand(texture, 3)?,
            PixelFormat::Rgba8Unorm => texture,
        };

        for pixel in texture.data.chunks_exact_mut(4) {
            if pixel == rgba {
                pixel.fill(0);
            }
        }

        Ok(texture)
    };

    match resource {
        TextureResource::Image(texture) => keyed(texture).map(TextureResource::Image),
        TextureResource::Cube(cube) => {
            let [a, b, c, d, e, f] = cube.faces;

            Ok(TextureResource::Cube(Box::new(CubeTexture {
                faces: [
                    keyed(a)?,
                    keyed(b)?,
                    keyed(c)?,
                    keyed(d)?,
                    keyed(e)?,
                    keyed(f)?,
                ],
                source_len: cube.source_len,
            })))
        }
    }
}

impl Texture {
    pub fn level(&self, index: usize) -> Option<&[u8]> {
        let m = self.mips.get(index)?;

        self.data.get(m.offset..m.offset + m.len)
    }

    pub fn base_level(&self) -> &[u8] {
        self.level(0).expect("a Texture always has mip 0")
    }

    pub fn trailing_bytes(&self, file_len: usize) -> usize {
        file_len.saturating_sub(self.source_len)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Container {
    Wys,
    Wyt,
    Wyp,
}

impl Container {
    pub fn sniff(bytes: &[u8]) -> Option<Container> {
        if bytes.starts_with(wys::MAGIC) {
            Some(Container::Wys)
        } else if bytes.starts_with(wyt::MAGIC) {
            Some(Container::Wyt)
        } else if bytes.starts_with(wyp::CIPHERTEXT_PREFIX) {
            Some(Container::Wyp)
        } else {
            None
        }
    }
}

pub fn decode(bytes: &[u8]) -> Result<Texture> {
    match Container::sniff(bytes) {
        Some(Container::Wys) => wys::decode(bytes),
        Some(Container::Wyt) => wyt::decode(bytes),
        Some(Container::Wyp) => wyp::decode(bytes),
        None => {
            let mut found = [0u8; 4];

            let n = bytes.len().min(4);

            found[..n].copy_from_slice(&bytes[..n]);

            Err(Error::BadMagic {
                kind: crate::error::Kind::Wys,
                expected: "WS10 / WT10 / .wyp cipher prefix",
                found,
            })
        }
    }
}

pub fn decode_image(bytes: &[u8]) -> Result<Texture> {
    if bytes.starts_with(b"DDS ") {
        wys::decode_dds(bytes)
    } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        wyp::decode_png(bytes)
    } else if Container::sniff(bytes).is_some() {
        let texture = decode(bytes)?;

        if texture.width % 4 != 0 || texture.height % 4 != 0 {
            match texture.format {
                PixelFormat::Bc1RgbaUnorm => dxt::expand(texture, 1),
                PixelFormat::Bc2RgbaUnorm => dxt::expand(texture, 3),
                PixelFormat::Rgba8Unorm => Ok(texture),
            }
        } else {
            Ok(texture)
        }
    } else if let Some(format) = raw::sniff(bytes) {
        raw::decode(bytes, format)
    } else {
        wyt::decode_tga(bytes)
    }
}
