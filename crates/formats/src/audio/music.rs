pub use super::sound_list::normalize_path;
use crate::error::{Error, Kind, Result};

pub const SLOTS: usize = 15;

pub const SLOT_LEN: usize = 0x100;

pub const PATH_COLUMN: usize = 17;

pub const LINE_LEN: usize = 0x100;

pub const BUILTIN: [&str; SLOTS] = [
    "music\\login.mp3",
    "music\\town01.mp3",
    "music\\field01.mp3",
    "music\\town02.mp3",
    "music\\field02.mp3",
    "music\\dungeon01.mp3",
    "music\\kingdom.mp3",
    "music\\dungeon02.mp3",
    "music\\town03.mp3",
    "music\\field03.mp3",
    "music\\CastleWar.mp3",
    "music\\kepra.mp3",
    "music\\khepraBoss.mp3",
    "",
    "",
];

pub const ENCRYPTED_SLOTS: [(usize, &str); 2] = [(13, "Mesh\\ed.dat"), (14, "Mesh\\tn.dat")];

pub use crate::scene::ending::KEY;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct MusicTable {
    slots: Vec<Option<String>>,
}

impl Default for MusicTable {
    fn default() -> Self {
        MusicTable {
            slots: vec![None; SLOTS],
        }
    }
}

impl MusicTable {
    pub fn get(&self, slot: usize) -> Option<&str> {
        self.slots.get(slot)?.as_deref()
    }

    pub fn overridden(&self) -> impl Iterator<Item = usize> + '_ {
        self.slots
            .iter()
            .enumerate()
            .filter_map(|(i, s)| s.is_some().then_some(i))
    }

    pub fn resolve(&self, slot: usize, exists: impl Fn(&str) -> bool) -> Option<&str> {
        if slot >= SLOTS {
            return None;
        }

        match self.get(slot) {
            Some(p) if exists(p) => Some(p),
            _ => Some(BUILTIN[slot]),
        }
    }

    pub fn disagreements(&self) -> Vec<usize> {
        self.overridden()
            .filter(|&i| {
                let o = self.get(i).unwrap_or_default();

                !o.is_empty()
                    && !normalize_path(o).eq_ignore_ascii_case(&normalize_path(BUILTIN[i]))
            })
            .collect()
    }
}

pub fn parse(bytes: &[u8]) -> Result<MusicTable> {
    let text: String = bytes.iter().map(|&b| b as char).collect();

    let mut table = MusicTable::default();

    for (slot, line) in text.split_inclusive('\n').take(SLOTS).enumerate() {
        let line = line.trim_end_matches(['\r', '\n']);

        if line.len() > LINE_LEN {
            return Err(Error::structure(
                Kind::MusicTable,
                format!(
                    "line {} is {} bytes, past the {LINE_LEN}-byte fgets buffer",
                    slot + 1,
                    line.len()
                ),
            ));
        }

        table.slots[slot] = Some(line.get(PATH_COLUMN..).unwrap_or_default().to_string());
    }

    Ok(table)
}

pub fn decrypt_bon(buf: &mut [u8]) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b = b.wrapping_sub(KEY[i % KEY.len()]);
    }
}

pub fn encrypt_bon(buf: &mut [u8]) {
    for (i, b) in buf.iter_mut().enumerate() {
        *b = b.wrapping_add(KEY[i % KEY.len()]);
    }
}
