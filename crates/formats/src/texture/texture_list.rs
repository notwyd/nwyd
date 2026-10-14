use crate::error::{Error, Kind, Result};
use crate::text::{Codepage, codepage};

pub const RECORD_LEN: usize = 0x210;

pub const LEGACY_RECORD_LEN: usize = 0x108;

pub const PATH_LEN: usize = 255;

pub const EXTENSION_LEN: usize = 4;

pub mod slots {

    pub const UI: usize = 512;

    pub const EFFECT: usize = 600;

    pub const MESH: usize = 3048;

    pub const ENV: usize = 2048;
}

#[allow(dead_code)]

mod field {

    pub const PATH: usize = 0x000;

    pub const NAME: usize = 0x0FF;

    pub const FORMAT_FLAG: usize = 0x1FE;

    pub const LAST_USE_MS: usize = 0x200;

    pub const TIMEOUT_MS: usize = 0x204;

    pub const NAME_HASH: usize = 0x208;

    pub const IS_DDS: usize = 0x20C;
}

#[allow(dead_code)]

mod legacy_field {

    pub const PATH: usize = 0x000;

    pub const FORMAT_FLAG: usize = 0x0FF;

    pub const LAST_USE_MS: usize = 0x100;

    pub const TIMEOUT_MS: usize = 0x104;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum FormatFlag {
    None_,
    Alpha,
    AlphaLower,
    ColorKey,
    Other(u8),
}

impl FormatFlag {
    pub fn from_byte(b: u8) -> FormatFlag {
        match b {
            b'N' => FormatFlag::None_,
            b'A' => FormatFlag::Alpha,
            b'a' => FormatFlag::AlphaLower,
            b'C' => FormatFlag::ColorKey,
            other => FormatFlag::Other(other),
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            FormatFlag::None_ => b'N',
            FormatFlag::Alpha => b'A',
            FormatFlag::AlphaLower => b'a',
            FormatFlag::ColorKey => b'C',
            FormatFlag::Other(b) => b,
        }
    }

    pub fn has_alpha(self) -> bool {
        matches!(self, FormatFlag::Alpha | FormatFlag::AlphaLower)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Entry {
    pub path: Vec<u8>,
    pub name: Vec<u8>,
    pub format_flag: FormatFlag,
    pub name_hash: u32,
    pub is_dds: bool,
}

impl Entry {
    pub fn path_str(&self) -> String {
        codepage::decode(&self.path, Codepage::Windows1252)
    }

    pub fn name_str(&self) -> String {
        codepage::decode(&self.name, Codepage::Windows1252)
    }

    pub fn fs_path(&self) -> String {
        to_relative_path(&self.path)
    }

    pub fn hash_is_consistent(&self) -> bool {
        djb2_lower(&self.name) == self.name_hash
    }
}

pub fn to_relative_path(raw: &[u8]) -> String {
    let mut out = String::with_capacity(raw.len());

    let mut prev_sep = false;

    for &b in raw {
        let is_sep = b == b'\\' || b == b'/';

        if is_sep {
            if !prev_sep {
                out.push('/');
            }
        } else {
            out.push(b as char);
        }

        prev_sep = is_sep;
    }

    out
}

pub fn djb2_lower(s: &[u8]) -> u32 {
    let mut h: u32 = 0x1505;

    for &b in s {
        let c = if b.is_ascii_uppercase() { b + 0x20 } else { b };

        h = h.wrapping_mul(0x21).wrapping_add(c as u32);
    }

    h
}

#[derive(Debug, Clone)]

pub struct TextureList {
    entries: Vec<Option<Entry>>,
}

fn nul_terminated(rec: &[u8], off: usize, len: usize) -> Vec<u8> {
    let field = &rec[off..off + len];

    let end = field.iter().position(|&b| b == 0).unwrap_or(len);

    field[..end].to_vec()
}

fn u32_at(rec: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(rec[off..off + 4].try_into().expect("4 bytes"))
}

pub fn parse(bytes: &[u8]) -> Result<TextureList> {
    if !bytes.len().is_multiple_of(RECORD_LEN) {
        return Err(Error::structure(
            Kind::TextureList,
            format!(
                "length {} is not a multiple of the {RECORD_LEN}-byte record",
                bytes.len()
            ),
        ));
    }

    let entries = bytes
        .chunks_exact(RECORD_LEN)
        .map(|rec| {
            if rec[field::PATH] == 0 {
                return None;
            }

            Some(Entry {
                path: nul_terminated(rec, field::PATH, PATH_LEN),
                name: nul_terminated(rec, field::NAME, PATH_LEN),
                format_flag: FormatFlag::from_byte(rec[field::FORMAT_FLAG]),
                name_hash: u32_at(rec, field::NAME_HASH),
                is_dds: u32_at(rec, field::IS_DDS) != 0,
            })
        })
        .collect();

    Ok(TextureList { entries })
}

pub fn parse_legacy(bytes: &[u8]) -> Result<TextureList> {
    if !bytes.len().is_multiple_of(LEGACY_RECORD_LEN) {
        return Err(Error::structure(
            Kind::TextureList,
            format!(
                "length {} is not a multiple of the {LEGACY_RECORD_LEN}-byte legacy record",
                bytes.len()
            ),
        ));
    }

    let entries = bytes
        .chunks_exact(LEGACY_RECORD_LEN)
        .map(|rec| {
            if rec[legacy_field::PATH] == 0 {
                return None;
            }

            let path = nul_terminated(rec, legacy_field::PATH, PATH_LEN);

            let name = strip_extension(&path).to_vec();

            let is_dds = path.len() >= EXTENSION_LEN
                && path[path.len() - EXTENSION_LEN..].eq_ignore_ascii_case(b".wys");

            Some(Entry {
                name_hash: djb2_lower(&name),
                path,
                name,
                format_flag: FormatFlag::from_byte(rec[legacy_field::FORMAT_FLAG]),
                is_dds,
            })
        })
        .collect();

    Ok(TextureList { entries })
}

pub fn strip_extension(path: &[u8]) -> &[u8] {
    match path.len().checked_sub(EXTENSION_LEN) {
        Some(n) => &path[..n],
        None => &[],
    }
}

impl TextureList {
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, index: usize) -> Option<&Entry> {
        self.entries.get(index).and_then(Option::as_ref)
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &Entry)> {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.as_ref().map(|e| (i, e)))
    }

    pub fn occupied(&self) -> usize {
        self.entries.iter().flatten().count()
    }

    pub fn find(&self, path: &[u8]) -> Option<usize> {
        let key = strip_extension(path);

        let h = djb2_lower(key);

        for (i, e) in self.iter() {
            if e.name_hash == h && e.name == key {
                return Some(i);
            }
        }

        for (i, e) in self.iter() {
            if e.path == path {
                return Some(i);
            }
        }

        None
    }

    pub fn mismatched_hashes(&self) -> Vec<usize> {
        self.iter()
            .filter(|(_, e)| !e.hash_is_consistent())
            .map(|(i, _)| i)
            .collect()
    }

    pub fn mismatched_extensions(&self) -> Vec<usize> {
        self.iter()
            .filter(|(_, e)| {
                let wys = e.path.len() >= EXTENSION_LEN
                    && e.path[e.path.len() - EXTENSION_LEN..].eq_ignore_ascii_case(b".wys");

                wys != e.is_dds
            })
            .map(|(i, _)| i)
            .collect()
    }
}
