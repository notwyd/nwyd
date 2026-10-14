use std::collections::BTreeMap;

pub const MAX_MESH_ID: u32 = 3499;

pub const PRIMARY_SLOTS: usize = 3500;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MeshList {
    entries: BTreeMap<u32, String>,
}

pub fn parse(text: &[u8]) -> MeshList {
    let text = crate::text::codepage::decode(text, crate::text::codepage::Codepage::Windows1252);

    let mut entries = BTreeMap::new();

    for line in text.lines() {
        let mut f = line.split_ascii_whitespace();

        let (Some(id), Some(path)) = (f.next(), f.next()) else {
            continue;
        };

        let Ok(id) = id.parse::<u32>() else {
            continue;
        };

        if id == 0 || id > MAX_MESH_ID {
            continue;
        }

        entries.insert(id, path.to_string());
    }

    MeshList { entries }
}

impl MeshList {
    pub fn get(&self, id: u32) -> Option<&str> {
        self.entries.get(&id).map(String::as_str)
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn ids(&self) -> impl Iterator<Item = u32> + '_ {
        self.entries.keys().copied()
    }

    pub fn iter(&self) -> impl Iterator<Item = (u32, &str)> + '_ {
        self.entries.iter().map(|(&id, p)| (id, p.as_str()))
    }
}

pub fn split_authored(path: &str) -> (&str, &str) {
    match path.find('\\') {
        Some(i) if path.len() >= i + 2 => (&path[..i], path.get(i + 2..).unwrap_or("")),
        Some(i) => (&path[..i], ""),
        None => ("", path),
    }
}

pub fn split(path: &str) -> (&str, &str) {
    match path.find(['\\', '/']) {
        Some(i) => {
            let rest = path[i..].trim_start_matches(['\\', '/']);

            (&path[..i], rest)
        }
        None => ("", path),
    }
}

pub fn is_effect_dir(dir: &str) -> bool {
    dir.eq_ignore_ascii_case("effect")
}
