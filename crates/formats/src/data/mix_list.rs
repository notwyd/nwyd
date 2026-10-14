use crate::data::Item;
use crate::error::{Error, Kind, Result};

pub const RECIPES: usize = 200;

pub const RECIPE_LEN: usize = 0xA68;

pub const BLOCK1_LEN: usize = RECIPES * RECIPE_LEN;

pub const REQUIREMENTS: usize = 200;

pub const REQUIREMENT_LEN: usize = 0x40;

pub const BLOCK2_LEN: usize = REQUIREMENTS * REQUIREMENT_LEN;

pub const LEN: usize = BLOCK1_LEN + BLOCK2_LEN;

pub const NEEDS: usize = 8;

pub const TEXT_SLOTS: usize = 20;

pub const TEXT_LEN: usize = 0x80;

pub const REQUIRED_ITEMS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Need {
    pub kind: i32,
    pub value: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recipe {
    pub index: i32,
    pub npc: i32,
    pub category: i32,
    pub subcategory: i32,
    pub result_item: u16,
    pub result_effects: [u8; 12],
    pub result_appearance: u16,
    pub result_reserved: u16,
    pub opcode: u16,
    pub need: [Need; NEEDS],
    pub gold_cost: i32,
    pub text: Vec<Vec<u8>>,
}

impl Recipe {
    pub fn is_empty(&self) -> bool {
        self.index == 0
            && self.npc == 0
            && self.category == 0
            && self.subcategory == 0
            && self.result_item == 0
            && self.opcode == 0
            && self.gold_cost == 0
            && self.need.iter().all(|n| n == &Need::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Requirement {
    pub field_00: i32,
    pub id: i32,
    pub required_count: i32,
    pub result: Item,
    pub items: [i32; REQUIRED_ITEMS],
    pub rate: [i32; 2],
    pub rate2: [i32; 2],
}

impl Requirement {
    pub fn is_empty(&self) -> bool {
        self == &Requirement::default()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixList {
    pub recipes: Vec<Recipe>,
    pub requirements: Vec<Requirement>,
}

impl MixList {
    pub fn recipe(&self, index: usize) -> Option<&Recipe> {
        self.recipes.get(index).filter(|r| !r.is_empty())
    }

    pub fn requirement(&self, index: usize) -> Option<&Requirement> {
        self.requirements.get(index).filter(|r| !r.is_empty())
    }

    pub fn occupied_recipes(&self) -> Vec<usize> {
        (0..self.recipes.len())
            .filter(|&i| self.recipe(i).is_some())
            .collect()
    }

    pub fn occupied_requirements(&self) -> Vec<usize> {
        (0..self.requirements.len())
            .filter(|&i| self.requirement(i).is_some())
            .collect()
    }

    pub fn opcodes(&self) -> Vec<u16> {
        let mut out: Vec<u16> = self
            .occupied_recipes()
            .iter()
            .map(|&i| self.recipes[i].opcode)
            .collect();

        out.sort_unstable();

        out.dedup();

        out
    }
}

fn i32_at(r: &[u8], o: usize) -> i32 {
    i32::from_le_bytes([r[o], r[o + 1], r[o + 2], r[o + 3]])
}

fn u16_at(r: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([r[o], r[o + 1]])
}

pub fn parse(bytes: &[u8]) -> Result<MixList> {
    if bytes.len() != LEN {
        return Err(Error::structure(
            Kind::MixList,
            format!("expected exactly {LEN} bytes, found {}", bytes.len()),
        ));
    }

    let recipes = bytes[..BLOCK1_LEN]
        .chunks_exact(RECIPE_LEN)
        .map(|r| {
            let mut need = [Need::default(); NEEDS];

            for (i, n) in need.iter_mut().enumerate() {
                n.kind = i32_at(r, 0x24 + i * 8);

                n.value = i32_at(r, 0x28 + i * 8);
            }

            let mut result_effects = [0u8; 12];

            result_effects.copy_from_slice(&r[0x12..0x1E]);

            Recipe {
                index: i32_at(r, 0x00),
                npc: i32_at(r, 0x04),
                category: i32_at(r, 0x08),
                subcategory: i32_at(r, 0x0C),
                result_item: u16_at(r, 0x10),
                result_effects,
                result_appearance: u16_at(r, 0x1E),
                result_reserved: u16_at(r, 0x20),
                opcode: u16_at(r, 0x22),
                need,
                gold_cost: i32_at(r, 0x64),
                text: r[0x68..]
                    .chunks_exact(TEXT_LEN)
                    .map(|t| {
                        let end = t.iter().position(|&b| b == 0).unwrap_or(TEXT_LEN);

                        t[..end].to_vec()
                    })
                    .collect(),
            }
        })
        .collect();

    let requirements = bytes[BLOCK1_LEN..]
        .chunks_exact(REQUIREMENT_LEN)
        .map(|r| {
            let mut items = [0i32; REQUIRED_ITEMS];

            for (i, v) in items.iter_mut().enumerate() {
                *v = i32_at(r, 0x18 + i * 4);
            }

            Requirement {
                field_00: i32_at(r, 0x00),
                id: i32_at(r, 0x04),
                required_count: i32_at(r, 0x08),
                result: Item::parse(&r[0x0C..0x18]),
                items,
                rate: [i32_at(r, 0x30), i32_at(r, 0x34)],
                rate2: [i32_at(r, 0x38), i32_at(r, 0x3C)],
            }
        })
        .collect();

    Ok(MixList {
        recipes,
        requirements,
    })
}
