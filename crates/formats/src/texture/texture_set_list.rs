use crate::error::Result;

pub const MAX_SETS: usize = 600;

pub const FIELDS_PER_ITEM: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Item {
    pub texture_index: i32,
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
    pub dst_offset: [i32; 2],
}

impl Item {
    pub fn rect(&self) -> [i32; 4] {
        [self.x, self.y, self.w, self.h]
    }

    pub fn is_degenerate(&self) -> bool {
        self.w <= 0 || self.h <= 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Set {
    pub name: String,
    pub index: usize,
    pub items: Vec<Item>,
}

impl Set {
    pub fn bare_name(&self) -> &str {
        self.name
            .strip_prefix('[')
            .and_then(|s| s.strip_suffix(']'))
            .unwrap_or(&self.name)
    }
}

#[derive(Debug, Clone, Default)]
pub struct TextureSetList {
    sets: Vec<Option<Set>>,
    pub warnings: Vec<String>,
}

pub fn parse(bytes: &[u8]) -> Result<TextureSetList> {
    let text: String = bytes.iter().map(|&b| b as char).collect();

    let mut sets: Vec<Option<Set>> = Vec::new();

    let mut warnings = Vec::new();

    let mut lines = text
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty())
        .peekable();

    while let Some(header) = lines.next() {
        let index = match lines.next().and_then(parse_labelled) {
            Some(v) => v,
            None => {
                warnings.push(format!("{header}: missing or malformed SetIndex"));

                continue;
            }
        };

        let count = match lines.next().and_then(parse_labelled) {
            Some(v) => v,
            None => {
                warnings.push(format!("{header}: missing or malformed ItemCount"));

                continue;
            }
        };

        let mut items = Vec::with_capacity(count.max(0) as usize);

        for _ in 0..count.max(0) {
            let Some(row) = lines.next() else {
                warnings.push(format!(
                    "{header}: ended after {} of {count} items",
                    items.len()
                ));

                break;
            };

            match parse_item(row) {
                Some(item) => items.push(item),
                None => warnings.push(format!("{header}: cannot read item row {row:?}")),
            }
        }

        if !(0..MAX_SETS as i32).contains(&index) {
            warnings.push(format!("Cannot read [{header}] in UI TextureSetList"));

            continue;
        }

        let index = index as usize;

        if sets.len() <= index {
            sets.resize(index + 1, None);
        }

        sets[index] = Some(Set {
            name: header.to_string(),
            index,
            items,
        });
    }

    Ok(TextureSetList { sets, warnings })
}

fn parse_labelled(line: &str) -> Option<i32> {
    line.split_once(':')?.1.trim().parse().ok()
}

fn parse_item(row: &str) -> Option<Item> {
    let mut it = row.split(',').map(|f| f.trim().parse::<i32>());

    let mut next = || it.next()?.ok();

    let item = Item {
        texture_index: next()?,
        x: next()?,
        y: next()?,
        w: next()?,
        h: next()?,
        dst_offset: [next()?, next()?],
    };

    it.next().is_none().then_some(item)
}

impl TextureSetList {
    pub fn len(&self) -> usize {
        self.sets.len()
    }

    pub fn is_empty(&self) -> bool {
        self.sets.iter().all(Option::is_none)
    }

    pub fn get(&self, index: usize) -> Option<&Set> {
        self.sets.get(index).and_then(Option::as_ref)
    }

    pub fn item(&self, index: usize, item: usize) -> Option<&Item> {
        self.get(index)?.items.get(item)
    }

    pub fn iter(&self) -> impl Iterator<Item = &Set> {
        self.sets.iter().flatten()
    }

    pub fn occupied(&self) -> usize {
        self.sets.iter().flatten().count()
    }

    pub fn total_items(&self) -> usize {
        self.iter().map(|s| s.items.len()).sum()
    }

    pub fn find_by_name(&self, name: &str) -> Option<&Set> {
        self.iter()
            .find(|s| s.name == name || s.bare_name() == name)
    }

    pub fn duplicate_names(&self) -> Vec<String> {
        let mut names: Vec<&str> = self.iter().map(|s| s.name.as_str()).collect();

        names.sort_unstable();

        let mut dups: Vec<String> = names
            .windows(2)
            .filter(|w| w[0] == w[1])
            .map(|w| w[0].to_string())
            .collect();

        dups.dedup();

        dups
    }

    pub fn texture_indices(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self
            .iter()
            .flat_map(|s| s.items.iter().map(|i| i.texture_index))
            .collect();

        ids.sort_unstable();

        ids.dedup();

        ids
    }
}
