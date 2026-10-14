use crate::error::{Error, Kind, Result};
use crate::text::{Codepage, codepage};

pub const RECORD_LEN: usize = 0x34;

pub const NAME_LEN: usize = 0x30;

pub const SLOTS: usize = 2560;

pub const NONE: &str = "NONE";

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Entry {
    pub name: Vec<u8>,
    pub id: u32,
}

impl Entry {
    pub fn name_str(&self) -> String {
        codepage::decode(&self.name, Codepage::Windows1252)
    }
}

#[derive(Debug, Clone)]

pub struct ResourceTable {
    entries: Vec<Option<Entry>>,
}

pub fn parse(bytes: &[u8]) -> Result<ResourceTable> {
    if !bytes.len().is_multiple_of(RECORD_LEN) {
        return Err(Error::structure(
            Kind::ResourceTable,
            format!(
                "length {} is not a multiple of the {RECORD_LEN}-byte record",
                bytes.len()
            ),
        ));
    }

    let entries = bytes
        .chunks_exact(RECORD_LEN)
        .map(|rec| {
            if rec[0] == 0 {
                return None;
            }

            let field = &rec[..NAME_LEN];

            let end = field.iter().position(|&b| b == 0).unwrap_or(NAME_LEN);

            Some(Entry {
                name: field[..end].to_vec(),
                id: u32::from_le_bytes(rec[NAME_LEN..RECORD_LEN].try_into().expect("4 bytes")),
            })
        })
        .collect();

    Ok(ResourceTable { entries })
}

impl ResourceTable {
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

    pub fn lookup(&self, name: &[u8]) -> Option<u32> {
        if name == NONE.as_bytes() {
            return Some(0);
        }

        self.iter().find(|(_, e)| e.name == name).map(|(_, e)| e.id)
    }

    pub fn resolve(&self, name: &[u8]) -> u32 {
        self.lookup(name).unwrap_or(0)
    }

    pub fn duplicate_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self.iter().map(|(_, e)| e.id).collect();

        ids.sort_unstable();

        let mut dups: Vec<u32> = ids
            .windows(2)
            .filter(|w| w[0] == w[1])
            .map(|w| w[0])
            .collect();

        dups.dedup();

        dups
    }
}
