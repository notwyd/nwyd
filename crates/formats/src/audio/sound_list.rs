use crate::error::{Error, Kind, Result};

pub const SLOTS: usize = 512;

pub const DEFAULT_VOICES: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Slot(u16);

impl Slot {
    pub const MAX: usize = SLOTS;

    pub fn new(index: i64) -> Option<Slot> {
        (index > 0 && (index as usize) < Slot::MAX).then_some(Slot(index as u16))
    }

    pub fn get(self) -> usize {
        self.0 as usize
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub raw_path: String,
    pub voices: i32,
}

impl Entry {
    pub fn path(&self) -> String {
        normalize_path(&self.raw_path)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundList {
    entries: Vec<Option<Entry>>,
    duplicates: Vec<Slot>,
}

impl Default for SoundList {
    fn default() -> Self {
        SoundList {
            entries: vec![None; SLOTS],
            duplicates: Vec::new(),
        }
    }
}

impl SoundList {
    pub fn get(&self, slot: Slot) -> Option<&Entry> {
        self.entries[slot.get()].as_ref()
    }

    pub fn get_index(&self, index: i64) -> Option<&Entry> {
        Slot::new(index).and_then(|s| self.get(s))
    }

    pub fn slots(&self) -> impl Iterator<Item = Slot> + '_ {
        self.entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| e.as_ref().map(|_| Slot(i as u16)))
    }

    pub fn len(&self) -> usize {
        self.entries.iter().filter(|e| e.is_some()).count()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn duplicates(&self) -> &[Slot] {
        &self.duplicates
    }

    pub fn missing(&self, exists: impl Fn(&str) -> bool) -> Vec<Slot> {
        self.slots()
            .filter(|&s| !exists(&self.get(s).expect("slot is occupied").path()))
            .collect()
    }
}

pub fn normalize_path(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());

    for ch in raw.chars() {
        let ch = if ch == '\\' { '/' } else { ch };

        if ch == '/' && out.ends_with('/') {
            continue;
        }

        out.push(ch);
    }

    out
}

pub fn parse(bytes: &[u8]) -> Result<SoundList> {
    let text: String = bytes.iter().map(|&b| b as char).collect();

    let mut tokens = text.split_ascii_whitespace();

    let mut list = SoundList::default();

    while let Some(index) = tokens.next() {
        let index: i64 = index
            .parse()
            .map_err(|e| Error::structure(Kind::SoundList, format!("slot {index:?}: {e}")))?;

        let slot = Slot::new(index).ok_or_else(|| {
            Error::structure(
                Kind::SoundList,
                format!(
                    "slot {index} is outside the addressable range 1..{}",
                    Slot::MAX
                ),
            )
        })?;

        let (Some(path), Some(voices)) = (tokens.next(), tokens.next()) else {
            break;
        };

        let voices: i32 = voices.parse().map_err(|e| {
            Error::structure(Kind::SoundList, format!("slot {index} voice count: {e}"))
        })?;

        if list.entries[slot.get()].is_some() {
            list.duplicates.push(slot);
        }

        list.entries[slot.get()] = Some(Entry {
            raw_path: path.to_string(),
            voices,
        });
    }

    Ok(list)
}
