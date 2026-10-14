use crate::error::{Error, Kind, Result};

pub const ENTRY_LEN: usize = 528;

pub const ENTRY_COUNT: usize = 2048;

pub const FILE_LEN: usize = ENTRY_LEN * ENTRY_COUNT;

const PATH_LEN: usize = 510;

const OFF_KIND: usize = 0x1FE;

const OFF_IS_DDS: usize = 0x20C;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum TextureKind {
    Alpha4,
    Alpha1,
    Opaque,
    Unknown(u8),
}

impl TextureKind {
    fn from_byte(b: u8) -> TextureKind {
        match b {
            b'A' | b'C' => TextureKind::Alpha4,
            b'a' => TextureKind::Alpha1,
            b'N' => TextureKind::Opaque,
            other => TextureKind::Unknown(other),
        }
    }

    pub fn has_alpha(self) -> bool {
        matches!(self, TextureKind::Alpha4 | TextureKind::Alpha1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct EnvTexture {
    pub id: usize,
    pub path: String,
    pub kind: TextureKind,
    pub is_dds: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct EnvTextureList {
    entries: Vec<Option<EnvTexture>>,
}

pub fn parse(bytes: &[u8]) -> Result<EnvTextureList> {
    if bytes.len() != FILE_LEN {
        return Err(Error::Structure {
            kind: Kind::EnvTextureList,
            message: format!(
                "EnvTextureList3.bin is {} bytes, expected {FILE_LEN} ({ENTRY_COUNT} x {ENTRY_LEN})",
                bytes.len()
            ),
        });
    }

    let entries = bytes
        .chunks_exact(ENTRY_LEN)
        .enumerate()
        .map(|(id, e)| {
            let end = e[..PATH_LEN]
                .iter()
                .position(|&b| b == 0)
                .unwrap_or(PATH_LEN);

            let path = &e[..end];

            if path.is_empty() || !path.iter().all(|&b| (0x20..0x7F).contains(&b)) {
                return None;
            }

            Some(EnvTexture {
                id,
                path: String::from_utf8_lossy(path).into_owned(),
                kind: TextureKind::from_byte(e[OFF_KIND]),
                is_dds: u32::from_le_bytes(
                    e[OFF_IS_DDS..OFF_IS_DDS + 4]
                        .try_into()
                        .expect("chunk is 528"),
                ) == 1,
            })
        })
        .collect();

    Ok(EnvTextureList { entries })
}

impl EnvTextureList {
    pub fn get(&self, id: usize) -> Option<&EnvTexture> {
        self.entries.get(id)?.as_ref()
    }

    pub fn iter(&self) -> impl Iterator<Item = &EnvTexture> {
        self.entries.iter().flatten()
    }

    pub fn populated_count(&self) -> usize {
        self.iter().count()
    }
}
