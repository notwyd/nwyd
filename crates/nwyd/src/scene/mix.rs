use crate::frame::PanelId;
use crate::scene::combine::{self, Kind, Staged};
use crate::scene::msgbox;
use formats::data::ItemList;
use formats::data::mix_list::{self, MixList, Recipe};
use formats::mesh::appearance::refine_level;
use formats::net::account::{Effect, Item};

pub const CONTENT: PanelId = 0x19;

pub const SCRIPT: &[u8] = b"ui/Panel/game/ItemMix.pane";

pub const HELP_SCRIPT: &str = "MixHelpN.dat";

#[derive(Debug, Default)]
pub struct HelpTables {
    pub help: Vec<formats::data::item_help::Help>,
    pub names: Vec<String>,
}

impl HelpTables {
    pub fn parse(bytes: &[u8]) -> Self {
        let (help, names) =
            formats::data::item_help::parse_mix(bytes, formats::text::Codepage::Windows1252);

        Self { help, names }
    }

    pub fn icon(&self, id: usize) -> Option<ui::IconContents> {
        if !(1..200).contains(&id) {
            return None;
        }

        let packed = i32::from(self.help.get(id)?.count as i16);

        if packed <= 0 {
            return None;
        }

        let cell = (packed - 1) % 100;

        let x = cell % 10 * 100;

        let y = cell / 10 * 100;

        Some(ui::IconContents {
            kind: 2,
            item: Item::default(),
            index: 0,
            count: 0,
            refine: -1,
            sheet: (packed - 1) / 100 + 1,
            sheet_uv: [x, y, x + 100, y + 100],
            grade: Default::default(),
            overlay: None,
            asking_price: -1,
            sweep: None,
            blink_off: false,
        })
    }

    pub fn tooltip(&self, id: usize) -> super::item_tip::Content {
        use ui::tooltip::Row;

        let valid = (1..200).contains(&id);

        let title = valid
            .then(|| self.names.get(id))
            .flatten()
            .map_or("", String::as_str);

        let mut rows = vec![Row::comment("")];

        if valid
            && let Some(help) = self.help.get(id)
            && help.color[0] != 0
        {
            rows.extend(help.line.iter().zip(help.color).map(|(text, color)| {
                Row::new(
                    text.replace('_', " "),
                    i32::from(color as i16) as u32 | 0xFF000000,
                )
            }));
        }

        rows.push(Row::new("", 0xFFCCCCCC));

        super::item_tip::Content {
            title: Row::new(title.replace('_', " "), 0xFFAAAAFF),
            rows,
            price: None,
            price_gap: false,
        }
    }
}

pub const RECIPE_CELLS: usize = 24;

pub const MAT_CELLS: usize = mix_list::NEEDS;

pub const MAT_LABELS: usize = MAT_CELLS;

pub const MONEY: &str = "Money";

pub const SEND_BUTTON: &str = "Btn";

pub const UNGATED_RECIPES: usize = 0x27;

pub fn npc_filter(
    entity: u16,
    body: i32,
    grade: u8,
    block: (u8, u8),
) -> Option<Option<(i32, i32, i32)>> {
    if entity < 1000 {
        return None;
    }

    let filter = match (body, block) {
        (67, (13, 13) | (24, 31)) => (67, i32::from(block.0), i32::from(block.1)),
        (67, (28, 24)) if entity != 1033 => (67, 28, 24),
        (54, (19, 13)) => (54, 19, 13),
        (54, (25, 13)) => return Some(None),
        (55 | 56, _) => (body, 19, 13),
        (66, (25, 13)) => (66, 25, 13),
        (68, _) => (68, i32::from(block.0), i32::from(block.1)),
        (67, (25, 13)) if grade & 15 == 8 => (67, 25, 13),
        _ => return None,
    };

    Some(Some(filter))
}

pub const MAX_PICK: usize = 0x5208;

pub const NEED_RANGE: std::ops::RangeInclusive<i32> = 1..=0x5207;

pub const NO_ITEM: usize = 412;

pub const NO_COUNT: usize = 413;

pub const NO_MONEY: usize = 414;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    NoItems,
    NotEnough,
    NoMoney,
    Gap,
}

impl Refused {
    pub fn strdef(self) -> usize {
        match self {
            Refused::NoItems | Refused::Gap => NO_ITEM,
            Refused::NotEnough => NO_COUNT,
            Refused::NoMoney => NO_MONEY,
        }
    }
}

pub const GAP_EXEMPT_FAMILY: i32 = 0x36;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Click {
    Recipe(usize),
    Send,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Browser {
    pub filter: (i32, i32, i32),
    pub matched: Vec<usize>,
    pub picked: Option<usize>,
    pub needs: Vec<Shown>,
    pub cost_visible: bool,
}

impl Browser {
    pub fn drawn(&self) -> Vec<usize> {
        self.matched
            .iter()
            .copied()
            .filter(|&index| index != 0)
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shown {
    pub need: mix_list::Need,
    pub item: Option<Item>,
    pub icon_kind: i32,
    pub bare: bool,
    pub count: i32,
}

impl Shown {
    pub fn required(&self, list: &MixList) -> i32 {
        if self.bare {
            return 1;
        }

        usize::try_from(self.need.value)
            .ok()
            .filter(|&row| (1..200).contains(&row))
            .and_then(|row| list.requirement(row))
            .map_or(0, |row| row.required_count)
    }

    pub fn satisfied(&self, list: &MixList) -> bool {
        let required = self.required(list);

        match self.bare {
            true => self.count != 0,
            false => self.count != 0 && required != 0 && self.count == required,
        }
    }
}

pub const MAT_LABEL_ARGB: (u32, u32) = (0xFFAA_0000, 0xFFFF_FFFF);

pub fn mat_label(
    shown: &Shown,
    list: &MixList,
    items: &formats::data::ItemList,
    strings: &ui::text::Strings,
) -> (String, u32) {
    if shown.need.value <= 0 {
        return (String::new(), MAT_LABEL_ARGB.0);
    }

    let row = usize::try_from(shown.need.value)
        .ok()
        .filter(|_| !shown.bare)
        .and_then(|row| list.requirement(row));

    let name = match (shown.bare, row) {
        (true, _) => items.name(shown.need.value as usize).unwrap_or_default(),
        (false, Some(row)) if row.id != 0 => strings
            .strdef
            .as_ref()
            .map(|table| table.get(row.id as usize).to_string())
            .unwrap_or_default(),
        (false, Some(row)) if (row.result.index as u32) < 0x38A4 => {
            items.name(row.result.index as usize).unwrap_or_default()
        }
        _ => String::new(),
    };

    let required = shown.required(list);

    let counted = shown.bare || required == 1;

    let text = match counted {
        true => format!("{name} {} / {required}", shown.count),
        false => name,
    };

    let argb = match shown.satisfied(list) {
        true => MAT_LABEL_ARGB.1,
        false => MAT_LABEL_ARGB.0,
    };

    (text, argb)
}

pub fn open(list: &MixList, filter: (i32, i32, i32), gated: bool) -> Browser {
    let mut matched = Vec::new();

    for (index, recipe) in list.recipes.iter().enumerate() {
        if (recipe.npc, recipe.category, recipe.subcategory) != filter {
            continue;
        }

        if matched.len() > RECIPE_CELLS - 1 {
            break;
        }

        if filter.0 == 0 && index >= UNGATED_RECIPES && !gated {
            continue;
        }

        matched.push(index);
    }

    Browser {
        filter,
        matched,
        picked: None,
        needs: Vec::new(),
        cost_visible: false,
    }
}

pub fn cell_item(recipe: &Recipe) -> Option<Item> {
    if recipe.result_item == 0 {
        return None;
    }

    let byte = |n: usize| recipe.result_effects.get(n).copied().unwrap_or(0);

    Some(Item {
        index: recipe.result_item,
        appearance: recipe.result_appearance,
        reserved: u16::from(recipe.result_reserved.to_le_bytes()[1]) << 8,
        effects: [
            Effect {
                id: byte(0),
                value: byte(1),
            },
            Effect {
                id: byte(4),
                value: byte(5),
            },
            Effect {
                id: byte(8),
                value: byte(9),
            },
        ],
    })
}

pub fn pick(list: &MixList, browser: &mut Browser, index: usize) -> bool {
    if index >= MAX_PICK {
        return false;
    }

    browser.needs.clear();

    let Some(recipe) = list.recipe(index) else {
        return false;
    };

    if (recipe.npc, recipe.category, recipe.subcategory) != browser.filter
        || recipe.index as usize != index
    {
        return false;
    }

    browser.picked = Some(index);

    browser.needs = recipe
        .need
        .iter()
        .take_while(|need| NEED_RANGE.contains(&need.value))
        .map(|need| shown(list, *need))
        .collect();

    true
}

fn shown(list: &MixList, need: mix_list::Need) -> Shown {
    match need.kind {
        0 => Shown {
            need,
            item: u16::try_from(need.value).ok().map(|index| Item {
                index,
                appearance: 1,
                ..Default::default()
            }),
            icon_kind: 2,
            bare: true,
            count: 0,
        },
        2 => Shown {
            need,
            item: None,
            icon_kind: 0,
            bare: false,
            count: 0,
        },
        _ => {
            let row = usize::try_from(need.value)
                .ok()
                .and_then(|n| list.requirement(n))
                .filter(|row| row.result.index != 0);

            Shown {
                need,
                item: row.map(|row| combine::read_item(&row.result.to_bytes())),
                icon_kind: 1,
                bare: false,
                count: 0,
            }
        }
    }
}

pub fn template_matches(template: &Item, candidate: &Item, items: &ItemList) -> bool {
    if template.index != candidate.index {
        return false;
    }

    let candidate_refine = refine_level(candidate, items);

    template.effects.iter().all(|effect| {
        effect.id != b'+' || candidate_refine >= template_refine_minimum(effect.value)
    })
}

fn template_refine_minimum(raw: u8) -> u32 {
    match raw {
        0xE6..=u8::MAX => u32::from(raw - 0xE6) / 4 + 10,
        _ => u32::from(raw),
    }
}

fn live_backpack_item(item: &Item) -> bool {
    (1..=0x38A3).contains(&i32::from(item.index as i16))
}

pub fn gather(
    list: &MixList,
    items: &ItemList,
    browser: &mut Browser,
    backpack: &[Item],
) -> (Staged, Vec<usize>) {
    let mut staged = Staged::new(Kind::Mix);

    let mut locked = Vec::new();

    let mut taken = vec![false; backpack.len()];

    browser.cost_visible = true;

    for (need_cell, shown) in browser.needs.iter_mut().take(MAT_CELLS).enumerate() {
        shown.count = 0;

        if shown.bare {
            let Ok(wanted) = u16::try_from(shown.need.value) else {
                continue;
            };

            let found = backpack.iter().take(60).enumerate().find(|(slot, item)| {
                !taken[*slot] && live_backpack_item(item) && item.index == wanted
            });

            let Some((slot, item)) = found else {
                continue;
            };

            taken[slot] = true;

            locked.push(slot);

            shown.count += 1;

            let Some(cell) =
                (need_cell..Kind::Mix.cells()).find(|&cell| staged.item(cell).is_empty())
            else {
                break;
            };

            staged.put(cell, *item, slot as u8);

            continue;
        }

        let row = usize::try_from(shown.need.value)
            .ok()
            .filter(|&row| (1..200).contains(&row))
            .and_then(|row| list.requirement(row));

        let (Some(row), Some(template)) = (row, shown.item) else {
            continue;
        };

        let required = row.required_count as u32;

        let mut count = 0u32;

        for (slot, candidate) in backpack.iter().take(60).enumerate() {
            if taken[slot]
                || !live_backpack_item(candidate)
                || count >= required
                || !template_matches(&template, candidate, items)
            {
                continue;
            }

            taken[slot] = true;

            locked.push(slot);

            if count == 0 && staged.item(need_cell).is_empty() {
                staged.put(need_cell, *candidate, slot as u8);
            }

            count = count
                .saturating_add(super::consume::stored_count(candidate) as u32)
                .min(required);
        }

        shown.count = count as i32;
    }

    if let Some(index) = browser.picked
        && let Some(recipe) = list.recipe(index)
    {
        staged.set_recipe(index as u16, recipe.opcode);
    }

    (staged, locked)
}

pub fn admit_item(
    list: &MixList,
    items: &ItemList,
    browser: &mut Browser,
    staged: &mut Staged,
    source: usize,
    candidate: Item,
) -> Option<super::refine::RecipeAdmission> {
    use super::refine::{RecipeCell, RecipeFill};

    let mut fill = RecipeFill {
        mode: 1,
        cells: [RecipeCell::default(); 8],
        staged: staged.clone(),
    };

    for (cell, shown) in fill.cells.iter_mut().zip(&browser.needs) {
        *cell = RecipeCell {
            value: shown.need.value,
            count: shown.count as u32,
            bare: shown.bare,
            item: shown.item.unwrap_or_default(),
        };
    }

    let effects = formats::data::item_effect::Effects::new(items, &[]);

    let admitted =
        super::refine::admit_material_item(&mut fill, list, items, &effects, source, candidate, 0)?;

    let shown = browser.needs.get_mut(admitted.need)?;

    shown.item = Some(candidate);

    shown.icon_kind = 1;

    shown.count = admitted.count as i32;

    *staged = fill.staged;

    Some(admitted)
}

pub fn ready(
    list: &MixList,
    browser: &Browser,
    staged: &Staged,
    money: u32,
) -> Result<(), Refused> {
    let filled: Vec<usize> = (0..Kind::Mix.cells())
        .filter(|&cell| !staged.item(cell).is_empty())
        .collect();

    if filled.is_empty() {
        return Err(Refused::NoItems);
    }

    let index = browser.picked.ok_or(Refused::NoItems)?;

    let recipe = list.recipe(index).ok_or(Refused::NoItems)?;

    for &cell in &filled {
        if staged.slot(cell).is_none() && recipe.npc != GAP_EXEMPT_FAMILY {
            return Err(Refused::Gap);
        }
    }

    for shown in browser.needs.iter().take(MAT_CELLS) {
        if !shown.satisfied(list) && shown.required(list) != 0 {
            return Err(Refused::NotEnough);
        }
    }

    match money >= recipe.gold_cost.max(0) as u32 {
        true => Ok(()),
        false => Err(Refused::NoMoney),
    }
}

pub fn confirm() -> msgbox::Request {
    combine::confirm(Kind::Mix)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    Picked { recipe: usize, staged: usize },
    Confirmed,
}

impl Outcome {
    pub fn describe(&self) -> String {
        match self {
            Outcome::Picked { recipe, staged } => {
                format!("NWYD: mix recipe {recipe} picked — {staged} ingredient(s) staged")
            }
            Outcome::Confirmed => "NWYD: mix — the combine was confirmed".to_string(),
        }
    }
}

pub fn panel_clicks(scene: &mut super::field::FieldScene) -> Vec<Outcome> {
    let player_present = scene
        .player
        .and_then(|i| scene.world.actors.actors.get(i))
        .is_some();

    let pending: Vec<Click> = match scene.panels.as_mut() {
        Some(panels) => std::mem::take(&mut panels.mix_clicks),
        None => return Vec::new(),
    };

    let mut out = Vec::new();

    for click in pending {
        let backpack = scene.entered.carry;

        let Some(panels) = scene.panels.as_mut() else {
            break;
        };

        if !panels.is_open(CONTENT) {
            continue;
        }

        match click {
            Click::Recipe(index) => {
                if !player_present || index >= MAX_PICK {
                    continue;
                }

                let Some(list) = panels.mixes.clone() else {
                    continue;
                };

                let picked = pick(&list, &mut panels.mix, index);

                let mut linked = false;

                for (cell, need) in list
                    .recipe(index)
                    .filter(|_| picked)
                    .into_iter()
                    .flat_map(|r| r.need.iter())
                    .take_while(|need| NEED_RANGE.contains(&need.value))
                    .enumerate()
                {
                    if need.kind == 2 {
                        panels.set_open(CONTENT, false);

                        panels.open_mix((need.value, 0, 0), false);

                        linked = true;
                    } else if linked {
                        panels.mix.needs.resize_with(cell + 1, || Shown {
                            need: mix_list::Need { kind: 0, value: 0 },
                            item: None,
                            icon_kind: 0,
                            bare: false,
                            count: 0,
                        });

                        panels.mix.needs[cell] = shown(&list, *need);
                    }
                }

                let (staged, locked) = {
                    let super::panels::Panels { items, mix, .. } = panels;

                    gather(&list, items, mix, &backpack)
                };

                let filled = (0..Kind::Mix.cells())
                    .filter(|&c| !staged.item(c).is_empty())
                    .count();

                *panels.staging.get_mut(Kind::Mix) = staged;

                if let Some(inventory) = panels
                    .screens
                    .iter_mut()
                    .find(|screen| screen.content == super::panels::CONTENT_INVENTORY)
                {
                    for slot in 0..60 {
                        super::screens::inventory_lock(inventory, slot, false);
                    }

                    for &slot in &locked {
                        super::screens::inventory_lock(inventory, slot, true);
                    }
                }

                panels.refresh_mix();

                if picked {
                    out.push(Outcome::Picked {
                        recipe: index,
                        staged: filled,
                    });
                }
            }
            Click::Send => {
                panels.raise_box(&confirm(), None);

                out.push(Outcome::Confirmed);
            }
        }
    }

    out
}
