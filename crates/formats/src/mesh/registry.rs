use crate::error::{Error, Kind, Result};

pub const MAX_MODELS: usize = 100;

pub const SLOTS_PER_MODEL: usize = 186;

pub const VALID_INDEX_LEN: usize = MAX_MODELS * SLOTS_PER_MODEL * 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelEntry {
    pub model_id: u32,
    pub num_anims: u32,
    pub num_parts: u32,
    pub name: String,
}

impl ModelEntry {
    pub fn base_name(&self) -> &str {
        self.name.rsplit(['\\', '/']).next().unwrap_or(&self.name)
    }

    pub fn dir(&self) -> &str {
        match self.name.rfind(['\\', '/']) {
            Some(i) => &self.name[..i],
            None => "",
        }
    }

    pub fn clip_file_name(&self, valid_index: i32) -> String {
        format!("{}{:04}.ani", self.base_name(), valid_index + 1)
    }

    pub fn skeleton_file_name(&self) -> String {
        format!("{}.bon", self.base_name())
    }

    pub fn part_mesh_file_name(&self, part: u32, mesh: u32, page: u32) -> String {
        if matches!(self.model_id, 0x2D | 0x2E | 0x35 | 0x36) {
            return format!("{}{:02}{:02}.msh", self.base_name(), part + 1, 1);
        }

        format!(
            "{}{:02}{:02}.msh",
            self.base_name(),
            part + 1,
            mesh + 1 + page * 20
        )
    }

    pub fn part_texture_file_name(&self, part: u32, mesh: u32, page: u32, colour: u32) -> String {
        let index = (colour & 0xFFF) + page * 20 + 1 + mesh;

        if self.texture_uses_part_one(part) {
            if self.model_id == 0x35 {
                return format!("{}{:02}{:02}.wyt", self.base_name(), part + 1, 1);
            }

            return format!("{}{:02}{:02}.wyt", self.base_name(), 1, index);
        }

        if matches!(self.model_id, 0x2D | 0x2E | 0x35 | 0x36) {
            return format!("{}{:02}{:02}.wyt", self.base_name(), part + 1, 1);
        }

        format!("{}{:02}{:02}.wyt", self.base_name(), part + 1, index)
    }

    pub fn texture_uses_part_one(&self, part: u32) -> bool {
        let b = self.base_name().as_bytes();

        let (c0, c1) = (b.first().copied(), b.get(1).copied());

        let c3 = b.get(3).copied();

        match (c0, c1) {
            (Some(b'g'), _) | (Some(b'o'), _) => true,
            (Some(b'd'), Some(b'r')) => {
                (c3 == Some(b'2') && (part == 1 || part == 2)) || c3 == Some(b'1')
            }
            (Some(b'b'), Some(b'd')) => part == 1,
            (Some(b'h'), Some(b's')) => part == 1,
            (Some(b'b'), Some(b'e' | b'o' | b'm' | b'n' | b'l')) => true,
            (Some(b'h'), Some(b'y')) => true,
            (Some(b's'), Some(b'p')) => true,
            (Some(b'c'), Some(b'r' | b'b')) => true,
            (Some(b'w'), Some(b'b' | b'f')) => true,
            (Some(b'm'), Some(b'i' | b'o')) => true,
            (Some(b't'), Some(b'w' | b'r' | b'g')) => true,
            (Some(b'e'), Some(b't')) => true,
            (Some(b'r'), Some(b'c')) => true,
            (Some(b'f'), Some(b'n')) => true,
            _ => false,
        }
    }

    pub fn substitute_part_names(
        &self,
        part: u32,
        mesh_name: &mut String,
        texture_name: &mut String,
    ) {
        if self.dir() != "mesh" {
            return;
        }

        if !self.texture_uses_part_one(part) {
            if mesh_name == "ch010218.msh" && texture_name == "ch010219.wyt" {
                *texture_name = "ch010214.wyt".to_string();
            } else if let Some(replacement) = cape_mesh_for_texture(texture_name) {
                *mesh_name = replacement.to_string();
            }
        }

        literal_texture_fixups(texture_name);

        if let Some(&(_, to)) = TEXTURE_EXACT
            .iter()
            .find(|&&(from, _)| from == texture_name)
        {
            *texture_name = to.to_string();

            return;
        }

        let prefixed = TEXTURE_PREFIXED
            .iter()
            .find(|&&(from, _)| texture_name.starts_with(from));

        if let Some(&(_, to)) = prefixed {
            *texture_name = to.to_string();
        }
    }
}

const TEXTURE_EXACT: [(&str, &str); 2] = [
    ("ch020315.wyt", "ch020314.wyt"),
    ("bm010102.wyt", "mi010105.wyt"),
];

const TEXTURE_PREFIXED: [(&str, &str); 12] = [
    ("tr13", "tr130101.wyt"),
    ("tr14", "tr130101.wyt"),
    ("tr15", "tr130101.wyt"),
    ("tr16", "tr130101.wyt"),
    ("tr17", "tr130101.wyt"),
    ("tr190101", "tr180101.wyt"),
    ("tr190102", "tr180102.wyt"),
    ("tr200101", "tr180101.wyt"),
    ("tr200102", "tr180102.wyt"),
    ("ch010237", "ch010137.wyt"),
    ("ch010238", "ch010138.wyt"),
    ("ch020217", "ch020117.wyt"),
];

fn cape_mesh_for_texture(texture: &str) -> Option<&'static str> {
    let index: u32 = texture
        .strip_prefix("mt0101")?
        .strip_suffix(".wyt")?
        .parse()
        .ok()?;

    if (170..=200).contains(&index) {
        return Some("mt010131.msh");
    }

    match index {
        24 => Some("mt010124.msh"),
        32..=37 => Some("mt010131.msh"),
        201..=204 => Some("mt010124.msh"),
        211..=214 => Some("mt010125.msh"),
        221..=224 => Some("mt010126.msh"),
        _ => None,
    }
}

fn literal_texture_fixups(texture_name: &mut String) {
    let b = texture_name.as_bytes();

    let at = |i: usize| b.get(i).copied();

    if at(0) != Some(b'c') || at(1) != Some(b'h') {
        return;
    }

    if at(3) == Some(b'2') && at(6) == Some(b'1') && at(7) == Some(b'3') {
        let replacement = match at(5) {
            Some(b'1') => Some("ch010130.wyt"),
            Some(b'4') => Some("ch010430.wyt"),
            Some(b'5') => Some("ch010530.wyt"),
            _ => None,
        };

        if let Some(name) = replacement {
            *texture_name = name.to_string();
        }
    }

    let b = texture_name.as_bytes();

    let at = |i: usize| b.get(i).copied();

    if at(0) == Some(b'c')
        && at(1) == Some(b'h')
        && at(2) == Some(b'0')
        && at(3) == Some(b'2')
        && at(6) == Some(b'1')
        && at(7) == Some(b'0')
        && at(8) == Some(b'3')
    {
        *texture_name = "ch0201103.wys".to_string();
    }
}

fn err(message: impl Into<String>) -> Error {
    Error::Structure {
        kind: Kind::Ani,
        message: message.into(),
    }
}

pub fn parse_bone_ani(text: &[u8]) -> Vec<ModelEntry> {
    let text = String::from_utf8_lossy(text);

    let mut out = Vec::new();

    for line in text.lines() {
        let mut f = line.split_whitespace();

        let (Some(id), Some(n_anim), Some(n_part), Some(name)) =
            (f.next(), f.next(), f.next(), f.next())
        else {
            continue;
        };

        let (Ok(model_id), Ok(num_anims), Ok(num_parts)) = (
            id.parse::<u32>(),
            n_anim.parse::<u32>(),
            n_part.parse::<u32>(),
        ) else {
            continue;
        };

        if model_id as usize >= MAX_MODELS {
            continue;
        }

        out.push(ModelEntry {
            model_id,
            num_anims,
            num_parts,
            name: name.to_string(),
        });

        if out.len() >= MAX_MODELS {
            break;
        }
    }

    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidIndex {
    table: Vec<i32>,
}

pub fn parse_valid_index(bytes: &[u8]) -> Result<ValidIndex> {
    if bytes.len() != VALID_INDEX_LEN {
        return Err(err(format!(
            "ValidIndex.bin is {} bytes, expected {VALID_INDEX_LEN} (i32[{MAX_MODELS}][{SLOTS_PER_MODEL}])",
            bytes.len()
        )));
    }

    let table = bytes
        .chunks_exact(4)
        .map(|c| i32::from_le_bytes(c.try_into().expect("chunk is 4 bytes")))
        .collect();

    Ok(ValidIndex { table })
}

impl ValidIndex {
    pub fn get(&self, model_id: u32, slot: usize) -> Option<i32> {
        if slot >= SLOTS_PER_MODEL {
            return None;
        }

        self.table
            .get(model_id as usize * SLOTS_PER_MODEL + slot)
            .copied()
    }

    pub fn clip_names(&self, entry: &ModelEntry) -> Vec<String> {
        (0..entry.num_anims as usize)
            .filter_map(|slot| self.get(entry.model_id, slot))
            .map(|idx| entry.clip_file_name(idx))
            .collect()
    }
}

pub const ANIM_GROUPS: usize = 60;

pub const ANIM_ACTIONS: usize = 56;

pub const ANIM_SLOT_MODELS: [u32; 6] = [0, 1, 2, 3, 4, 5];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimSlotTable {
    rows: Vec<u16>,
}

impl Default for AnimSlotTable {
    fn default() -> Self {
        AnimSlotTable {
            rows: vec![0; ANIM_GROUPS * ANIM_ACTIONS],
        }
    }
}

impl AnimSlotTable {
    pub fn get(&self, group: usize, code: usize) -> Option<u16> {
        if group >= ANIM_GROUPS || code >= ANIM_ACTIONS {
            return None;
        }

        self.rows.get(group * ANIM_ACTIONS + code).copied()
    }

    fn set(&mut self, group: usize, code: usize, slot: u16) {
        if group < ANIM_GROUPS && code < ANIM_ACTIONS {
            self.rows[group * ANIM_ACTIONS + code] = slot;
        }
    }

    pub fn build(model_id: u32, valid: &ValidIndex, entry: &ModelEntry, loaded: &[bool]) -> Self {
        let mut table = AnimSlotTable::default();

        if !ANIM_SLOT_MODELS.contains(&model_id) {
            return table;
        }

        let mut slot: u16 = 0;

        for i in 0..entry.num_anims as usize {
            let Some(index) = valid.get(model_id, i) else {
                continue;
            };

            if !loaded.get(i).copied().unwrap_or(false) {
                continue;
            }

            let number = index + 1;

            if number < 100 {
                slot += 1;

                continue;
            }

            let group = (number / 100 - 1) as usize;

            let one_based = (number % 100) as usize;

            let action = one_based.saturating_sub(1);

            table.set(group, action, slot);

            if model_id < 2 {
                if (5..10).contains(&one_based) {
                    let value = table.get(group, action).unwrap_or(0);

                    for code in one_based..10 {
                        table.set(group, code, value);
                    }
                }

                if (0x1A..0x1E).contains(&one_based) {
                    let value = table.get(group, action).unwrap_or(0);

                    for code in one_based..0x1E {
                        table.set(group, code, value);
                    }
                }
            }

            if model_id == 0 && group > 11 {
                for code in [11, 12, 14] {
                    table.set(group, code, table.get(11, code).unwrap_or(0));
                }
            }

            if model_id == 1 && group == 2 && action == 4 {
                for code in 0..4 {
                    table.set(2, code, table.get(1, code).unwrap_or(0));
                }
            }

            if matches!(model_id, 2 | 4) && action == 3 {
                table.set(group, 4, table.get(group, 3).unwrap_or(0));
            }

            if model_id == 2 && number == 307 {
                for code in 0..ANIM_ACTIONS {
                    table.set(3, code, table.get(2, code).unwrap_or(0));
                }
            }

            let broadcast = match model_id {
                0 => 138,
                1 => 137,
                2 => 107,
                3 | 4 => 108,
                5 => 111,
                _ => unreachable!("ANIM_SLOT_MODELS admitted only 0..=5"),
            };

            if number == broadcast {
                for group in 1..ANIM_GROUPS {
                    for code in 0..ANIM_ACTIONS {
                        table.set(group, code, table.get(0, code).unwrap_or(0));
                    }
                }
            }

            slot += 1;
        }

        table
    }
}
