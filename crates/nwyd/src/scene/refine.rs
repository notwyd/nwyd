use super::item_move::{self, Coord};
use crate::frame::PanelId;
use formats::data::item_effect::{
    Effects, effect, is_bound_family, is_extended_bound_family, is_special_family,
};
use formats::data::item_list::ItemList;
use formats::data::mix_list::{MixList, Requirement};
use formats::mesh::appearance::refine_level;
use formats::net::account::Item;

pub const CONTENT: PanelId = super::panels::CONTENT_REFINE;

pub const SCRIPT: &[u8] = b"ui/Panel/game/ItemRefine.pane";

pub const CELLS: usize = 7;

pub const MATERIAL_CELLS: std::ops::Range<usize> = 1..CELLS;

pub const CELL_ICON_SLOT_TYPE: i32 = 2;

pub const CELL_POOL: i32 = ui::widget::cell_group::NONE;

pub const ELEMENTS: [(&str, u32); 12] = [
    ("Money", 0x7A),
    ("RateValue", 0x7E),
    ("RateString", 0x82),
    ("ResultSanc", 0x86),
    ("ItemName", 0x8A),
    ("ResultItem", 0x8E),
    ("SancUp", 0x92),
    ("SancDown", 0x96),
    ("Refine", 0x9A),
    ("Stop", 0x9E),
    ("Auto", 0xA2),
    ("ResultSancEdit", 0xA6),
];

pub const AUTO_CAPTION: &str = "uu_316";

pub const STOP_ON_CAPTION: &str = "uu_315";

pub const STOP_OFF_CAPTION: &str = "uu_313";

pub const AUTO_CALLBACK_CRASH_REPAIRED: bool = false;

pub const ITEM_LOCKED: usize = 1436;

pub const ITEM_LOCK_HAS_NO_WRITER: bool = false;

pub const NOT_ENOUGH_GOLD: usize = super::pgt::NOT_ENOUGH_GOLD;

pub const WRONG_POWDER: usize = 1360;

pub const NO_RECIPE_ITEMS: usize = super::mix::NO_ITEM;

pub const NOT_ENOUGH_RECIPE_ITEMS: usize = super::mix::NO_COUNT;

pub const MAT_LABEL_ARGB: u32 = 0xFFAA_0000;

pub const POWDER_OPCODE: u16 = 0x373;

pub const POWDER_LEN: usize = 0x24;

pub const RECIPE_LEN: usize = super::combine::FRAME_LEN;

pub const REPEAT_MS: u32 = 0x5DC;

pub const AUTO_DELAY_MS: u32 = 0x7D0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct AutoDelay(u32);

impl Default for AutoDelay {
    fn default() -> Self {
        Self(600)
    }
}

impl AutoDelay {
    pub fn milliseconds(self) -> u32 {
        self.0
    }

    pub fn step(&mut self, increase: bool) {
        self.0 = if increase {
            (self.0 + 200).min(2000)
        } else {
            self.0.saturating_sub(200).max(200)
        };
    }
}

pub const MAX_CONSIDERED: i32 = 0xE;

pub const LADDER_BREAK: i32 = 9;

pub fn sanc_up(current: i32, max: i32) -> Option<i32> {
    if current >= max {
        return None;
    }

    Some(if current < LADDER_BREAK {
        current + 1
    } else {
        current * 4 - 0x17
    })
}

pub fn sanc_down(current: i32) -> i32 {
    let next = current - 1;

    if next <= LADDER_BREAK {
        next
    } else {
        next * 4 - 0x1B
    }
}

pub const D_531_THE_SPINNER_SKIPS_TEN_ELEVEN_AND_TWELVE: bool = false;

pub fn powder_matches(
    powder: i32,
    item_id: i32,
    level: i32,
    effect_70: i32,
    effect_71: i32,
) -> bool {
    if level > MAX_CONSIDERED {
        return false;
    }

    match powder {
        0x19C => {
            if effect_70 == 3 || level > 5 || effect_71 == 1 {
                return false;
            }

            item_id != 0xD88
        }
        0x19D => !(0xD45..=0xD4E).contains(&item_id),
        0x102D => {
            if (0x24F..=0x253).contains(&item_id) && level < 9 {
                return false;
            }

            for range in [
                0x204..=0x209,
                0x24F..=0x253,
                0x1644..=0x164A,
                0x1676..=0x167C,
                0x164B..=0x1651,
                0x167D..=0x1687,
                0x1659..=0x165E,
                0x168B..=0x1690,
                0x237..=0x23A,
            ] {
                if range.contains(&item_id) {
                    return true;
                }
            }

            for range in [
                0x204..=0x209,
                0x24F..=0x253,
                0x1644..=0x164A,
                0x1676..=0x167C,
                0xD45..=0xD4E,
            ] {
                if range.contains(&item_id) {
                    return false;
                }
            }

            effect_71 != 1
        }
        _ => true,
    }
}

pub fn displayed_piece_accepts_powder(items: &ItemList, piece: Item, powder: Item) -> bool {
    let effects = Effects::new(items, &[]);

    piece.is_empty()
        || powder_matches(
            i32::from(powder.index as i16),
            i32::from(piece.index as i16),
            refine_level(&piece, items) as i32,
            effects.total_scaled(&piece, 0x70),
            effects.total_scaled(&piece, 0x71),
        )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Source {
    pub at: Coord,
    pub selected: bool,
    pub preview: Item,
}

impl Default for Source {
    fn default() -> Self {
        Source {
            at: Coord {
                container: 0,
                slot: 0,
            },
            selected: false,
            preview: Item::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]

pub struct Refine {
    pub mode: i32,
    pub current: i32,
    pub target: i32,
    pub result: Item,
    pub running: bool,
    pub auto: bool,
    pub auto_delay: AutoDelay,
    pub powder: Source,
    pub piece: Source,
    pub remembered_powder: i32,
    pub recipe: Option<RecipeFill>,
    pub manual_requested: bool,
    pub last_send_ms: u32,
}

impl Refine {
    pub fn preview_target(&mut self, item: Item, raw: i32, items: &ItemList) {
        self.result = preview_item(item, raw);

        self.target = refine_level(&self.result, items) as i32;
    }

    pub fn retain_powder(&mut self, at: Coord, preview: Item) {
        self.powder = Source {
            at,
            selected: true,
            preview,
        };
    }

    pub fn retain_piece(&mut self, at: Coord, preview: Item) {
        self.piece = Source {
            at,
            selected: true,
            preview,
        };
    }

    pub fn retain_recipe(&mut self, list: &MixList, mode: i32) -> bool {
        let Some(fill) = recipe_fill(list, mode) else {
            return false;
        };

        self.mode = mode;

        self.recipe = Some(fill);

        true
    }

    pub fn reset(&mut self) {
        *self = Refine {
            auto_delay: self.auto_delay,
            ..Default::default()
        };
    }
}

pub fn preview_item(mut item: Item, raw: i32) -> Item {
    if raw > 33 {
        return item;
    }

    let raw = raw.max(0);

    let value = if raw < 10 {
        raw as u8
    } else {
        (raw - 36) as u8
    };

    let slot = item
        .effects
        .iter()
        .position(|e| e.id == b'+')
        .or_else(|| {
            item.effects
                .iter()
                .position(|e| (0x73..=0x7E).contains(&e.id))
        })
        .or_else(|| item.effects.iter().position(|e| e.id == 0));

    if let Some(slot) = slot {
        if item.effects[slot].id == 0 {
            item.effects[slot].id = b'+';
        }

        item.effects[slot].value = value;
    }

    item
}

#[derive(Debug, Clone, PartialEq)]

pub enum Action {
    Send(Vec<u8>),
    Toast(usize),
    Target(i32),
    Halt,
    Manual,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct AutoChanged {
    pub halted: bool,
    pub stop_caption: &'static str,
}

pub fn auto_changed(state: &mut Refine, open: bool, checked: u8) -> Option<AutoChanged> {
    if !open {
        return None;
    }

    state.auto = checked == 1;

    let halted = state.running;

    if halted {
        state.running = false;

        state.auto = false;
    }

    Some(AutoChanged {
        halted,
        stop_caption: if checked == 1 {
            STOP_ON_CAPTION
        } else {
            STOP_OFF_CAPTION
        },
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Repeat {
    Refresh,
    Send,
}

pub fn repeat(state: &Refine, last_send_ms: u32, now_ms: u32) -> Option<Repeat> {
    if last_send_ms == 0
        || now_ms.wrapping_sub(last_send_ms) <= state.auto_delay.milliseconds()
        || !state.running
    {
        return None;
    }

    Some(if state.auto {
        Repeat::Send
    } else {
        Repeat::Refresh
    })
}

pub fn send_ready(last_kind_ms: u32, now_ms: u32) -> bool {
    last_kind_ms == 0 || now_ms.wrapping_sub(last_kind_ms) >= REPEAT_MS
}

pub fn command(state: &Refine, name: &str, max: i32, locked: bool) -> Vec<Action> {
    if name == "Stop" {
        return vec![Action::Halt];
    }

    if name == "Refine" {
        if locked {
            return vec![Action::Toast(ITEM_LOCKED)];
        }

        if state.running {
            return Vec::new();
        }

        return vec![Action::Manual];
    }

    if name == "SancDown" {
        if state.running || state.current >= state.target - 1 {
            return Vec::new();
        }

        return vec![Action::Target(sanc_down(state.target))];
    }

    if name == "SancUp" {
        if state.running {
            return Vec::new();
        }

        return match sanc_up(state.target, max) {
            Some(next) => vec![Action::Target(next)],
            None => Vec::new(),
        };
    }

    Vec::new()
}

pub fn powder_frame(client: u16, powder: Coord, piece: Coord) -> Vec<u8> {
    let mut frame = vec![0u8; POWDER_LEN];

    frame[0x04..0x06].copy_from_slice(&POWDER_OPCODE.to_le_bytes());

    frame[0x06..0x08].copy_from_slice(&client.to_le_bytes());

    for (offset, value) in [
        (12, powder.container),
        (16, powder.slot),
        (20, piece.container),
        (24, piece.slot),
    ] {
        frame[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    }

    frame
}

pub fn recipe_frame(list: &MixList, mode: i32) -> Option<super::combine::Staged> {
    let row = list.recipe(usize::try_from(mode).ok()?)?;

    let mut staged = super::combine::Staged::new(super::combine::Kind::Mix);

    staged.set_recipe(u16::try_from(mode).ok()?, row.opcode);

    Some(staged)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct RecipeCell {
    pub value: i32,
    pub count: u32,
    pub bare: bool,
    pub item: Item,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct RecipeFill {
    pub mode: i32,
    pub cells: [RecipeCell; 8],
    pub staged: super::combine::Staged,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct RecipeAdmission {
    pub need: usize,
    pub staged: Option<usize>,
    pub locked: usize,
    pub count: u32,
}

pub fn recipe_fill(list: &MixList, mode: i32) -> Option<RecipeFill> {
    if !(0..0x5209).contains(&mode) {
        return None;
    }

    let index = usize::try_from(mode).ok()?;

    let recipe = list.recipe(index)?;

    let mut cells = [RecipeCell::default(); 8];

    for (cell, need) in recipe.need.iter().take(7).enumerate() {
        if !(1..=0x5207).contains(&need.value) {
            break;
        }

        cells[cell].value = need.value;

        match need.kind {
            0 => {
                cells[cell].bare = true;

                cells[cell].item.index = need.value as u16;
            }
            2 => {}
            _ => {
                let Some(row) = usize::try_from(need.value)
                    .ok()
                    .and_then(|row| list.requirements.get(row))
                else {
                    continue;
                };

                if (row.result.index as i16) > 0 {
                    cells[cell].item = super::combine::read_item(&row.result.to_bytes());
                }
            }
        }
    }

    Some(RecipeFill {
        mode,
        cells,
        staged: recipe_frame(list, mode)?,
    })
}

pub fn selected_recipe(
    items: &ItemList,
    sets: &[formats::data::item_set::SetBonus],
    item: &Item,
) -> Option<i32> {
    if !(1..0x38A4).contains(&item.index) || !super::item_use::refinable(items, item) {
        return None;
    }

    let entry = items.get(usize::from(item.index))?;

    if entry.set_index >= 0x38A4 {
        return None;
    }

    let mode = formats::data::item_set::effect_for(sets, entry.set_index).unwrap_or(0);

    if mode != 0 {
        return Some(mode);
    }

    let effects = Effects::new(items, &[]);

    let restriction = effects.total_scaled(item, 0x70);

    let level = refine_level(item, items);

    if level > 10 && restriction == 3 && effects.total_scaled(item, 0x71) == 0 {
        Some(60)
    } else if level > 10 && restriction < 3 && !super::item_use::slow_refine(i32::from(item.index))
    {
        Some(70)
    } else {
        Some(0)
    }
}

pub fn gather_recipe(
    fill: &mut RecipeFill,
    list: &MixList,
    items: &ItemList,
    carry: &[Item],
) -> [bool; 60] {
    let mut locked = [false; 60];

    for cell in 0..8 {
        fill.cells[cell].count = 0;

        fill.staged.clear(cell);
    }

    for group in 0..8 {
        let cell = fill.cells[group];

        if cell.value <= 0 {
            continue;
        }

        for (source, candidate) in carry.iter().take(60).enumerate() {
            if locked[source] || !(1..0x38A4).contains(&candidate.index) {
                continue;
            }

            if cell.bare {
                if i32::from(candidate.index) != cell.value || fill.cells[group].count != 0 {
                    continue;
                }

                locked[source] = true;

                fill.cells[group].count = 1;

                if let Some(target) = (group..8).find(|&n| fill.staged.item(n).is_empty()) {
                    fill.staged.put(target, *candidate, source as u8);
                }
            } else if let Some(row) = list.requirement(cell.value as usize) {
                let template = super::combine::read_item(&row.result.to_bytes());

                if !super::mix::template_matches(&template, candidate, items)
                    || fill.cells[group].count >= row.required_count.max(0) as u32
                {
                    continue;
                }

                locked[source] = true;

                if fill.staged.item(group).is_empty() {
                    fill.staged.put(group, *candidate, source as u8);
                }

                fill.cells[group].count = fill.cells[group]
                    .count
                    .saturating_add(super::consume::stored_count(candidate).max(0) as u32)
                    .min(row.required_count.max(0) as u32);
            }
        }
    }

    locked
}

pub fn recipe_label(
    cell: &RecipeCell,
    list: &MixList,
    items: &ItemList,
    strings: &ui::text::Strings,
) -> (String, u32) {
    let row = (!cell.bare && cell.value > 0)
        .then(|| list.requirement(cell.value as usize))
        .flatten();

    let required = if cell.bare {
        1
    } else {
        row.map_or(0, |row| row.required_count.max(0) as u32)
    };

    if cell.value <= 0 || required == 0 {
        return (String::new(), 0xFFAA0000);
    }

    let name = if cell.bare {
        items.name(cell.value as usize).unwrap_or_default()
    } else if let Some(row) = row {
        if row.id != 0 {
            strings.translate(&format!("ss_{}", row.id))
        } else {
            items
                .name(usize::from(row.result.index))
                .unwrap_or_default()
        }
    } else {
        String::new()
    };

    let satisfied = if cell.bare {
        cell.count != 0
    } else {
        cell.count == required
    };

    (
        format!("{name}<n>{} / {required}", cell.count),
        if satisfied { 0xFFFFFFFF } else { 0xFFAA0000 },
    )
}

fn template_minimum(raw: u8) -> u32 {
    match raw {
        0xE6..=u8::MAX => u32::from(raw - 0xE6) / 4 + 10,
        _ => u32::from(raw),
    }
}

fn first_effect_matches(template: &Item, candidate: &Item, items: &ItemList) -> bool {
    let first = template.effects[0];

    first.id != b'+' || refine_level(candidate, items) >= template_minimum(first.value)
}

fn requirement_31_accepts_item(id: i32) -> bool {
    [0x4DA..=0x4DD, 0x561..=0x564, 0x5F7..=0x5FA, 0x68D..=0x690]
        .into_iter()
        .any(|range| range.contains(&id))
        || id == 0x6B3
}

fn requirement_32_accepts_item(id: i32) -> bool {
    [0x4DE..=0x4E1, 0x565..=0x568, 0x5FB..=0x5FE, 0x691..=0x694]
        .into_iter()
        .any(|range| range.contains(&id))
        || id == 0x6B4
}

fn requirement_53_accepts_item(id: i32) -> bool {
    [
        0xE19..=0xE1C,
        0xE2D..=0xE30,
        0xE41..=0xE44,
        0xE55..=0xE58,
        0xE69..=0xE6C,
        0xE7D..=0xE80,
        0xE99..=0xEA0,
        0xEB9..=0xEBC,
        0xECD..=0xED0,
    ]
    .into_iter()
    .any(|range| range.contains(&id))
}

fn requirement_0f_accepts_item(id: i32) -> bool {
    [
        0x76D..=0x776,
        0x4D2..=0x4D5,
        0x559..=0x55C,
        0x5EF..=0x5F2,
        0x685..=0x688,
    ]
    .into_iter()
    .any(|range| range.contains(&id))
        || id == 0x6B2
}

fn requirement_10_accepts_item(id: i32) -> bool {
    [
        0x9BB..=0x9BE,
        0x9F7..=0x9FA,
        0xA33..=0xA36,
        0xA6F..=0xA72,
        0xAAB..=0xAAE,
        0xAE7..=0xAEA,
        0xB2B..=0xB32,
        0xB4F..=0xB52,
        0xB77..=0xB7A,
        0x4C5..=0x4C8,
        0x54C..=0x54F,
        0x5E2..=0x5E5,
        0x678..=0x67B,
        0x6AF..=0x6AF,
    ]
    .into_iter()
    .any(|range| range.contains(&id))
}

fn requirements_54_55_accept_item(id: i32) -> bool {
    [
        0x9BB..=0x9BE,
        0x9F7..=0x9FA,
        0xA33..=0xA36,
        0xA6F..=0xA72,
        0xAAB..=0xAAE,
        0xAE7..=0xAEA,
        0xB2B..=0xB32,
        0xB4F..=0xB52,
        0xB77..=0xB7A,
    ]
    .into_iter()
    .any(|range| range.contains(&id))
}

fn requirement_matches(
    row_index: usize,
    row: &Requirement,
    candidate: &Item,
    items: &ItemList,
    effects: &Effects<'_>,
) -> bool {
    let id = i32::from(candidate.index as i16);

    let mut admitted = match row_index {
        0x0F => requirement_0f_accepts_item(id),
        0x10 => requirement_10_accepts_item(id),
        0x2C | 0x51 => is_extended_bound_family(id),
        0x2D => is_special_family(id),
        0x30 => is_bound_family(id),
        0x31 => requirement_31_accepts_item(id),
        0x32 => requirement_32_accepts_item(id),
        0x53 => requirement_53_accepts_item(id),
        0x54 | 0x55 => requirements_54_55_accept_item(id),
        _ => false,
    };

    if matches!(
        row_index, 0x0F..= 0x10 | 0x2C | 0x2D | 0x30..= 0x32 | 0x51 | 0x53..= 0x55
    ) && !admitted
    {
        return false;
    }

    if row.rate[0] != 0 {
        let restriction = effects.total_scaled(candidate, effect::RESTRICTION) as u32;

        if restriction < row.rate[0] as u32 || (row.rate[1] as u32) < restriction {
            return false;
        }

        admitted = true;
    }

    let entry = usize::try_from(id).ok().and_then(|id| items.get(id));

    if row_index == 0x14 {
        let Some(entry) = entry.as_ref() else {
            return false;
        };

        if entry.grade == 0 {
            return false;
        }

        let grade = i32::from(entry.grade) % 10;

        let class = effects.total_scaled(candidate, effect::CLASS);

        if grade != 0 && grade < if class == 1 { 7 } else { 6 } {
            return false;
        }
    }

    if row.rate2[0] != 0 {
        let Some(entry) = entry.as_ref() else {
            return false;
        };

        if entry.equip_mask < row.rate2[0] as u32 || (row.rate2[1] as u32) < entry.equip_mask {
            return false;
        }

        admitted = true;
    }

    if row.rate == [0, 0] && row.rate2 == [0, 0] {
        true
    } else {
        admitted
    }
}

fn in_requirement_ranges(row: &Requirement, id: u32) -> bool {
    row.items
        .chunks_exact(2)
        .any(|pair| pair[0] as u32 <= id && id <= pair[1] as u32)
}

enum AdmissionKind {
    One,
    Counted { row: usize },
}

pub fn admit_recipe_item(
    fill: &mut RecipeFill,
    list: &MixList,
    items: &ItemList,
    effects: &Effects<'_>,
    source: usize,
    candidate: Item,
) -> Option<RecipeAdmission> {
    if fill.mode <= 0 {
        return None;
    }

    admit_material_item(fill, list, items, effects, source, candidate, 1)
}

pub(super) fn admit_material_item(
    fill: &mut RecipeFill,
    list: &MixList,
    items: &ItemList,
    effects: &Effects<'_>,
    source: usize,
    candidate: Item,
    first_need: usize,
) -> Option<RecipeAdmission> {
    if source >= 60 || candidate.is_empty() {
        return None;
    }

    for need in first_need..8 {
        let cell = fill.cells[need];

        let Ok(row_index) = usize::try_from(cell.value) else {
            continue;
        };

        let Some(row) = list.requirements.get(row_index) else {
            continue;
        };

        let template = super::combine::read_item(&row.result.to_bytes());

        let candidate_id = u32::from(candidate.index as u16);

        let kind = if cell.bare {
            if row_index >= 100
                || (template.index as i16) <= 0
                || !first_effect_matches(&template, &candidate, items)
                || !cell.item.is_empty()
            {
                continue;
            }

            AdmissionKind::One
        } else if cell.value == 0 {
            continue;
        } else if (template.index as i16) > 0 {
            if template.index != candidate.index
                || !super::mix::template_matches(&template, &candidate, items)
                || !cell.item.is_empty()
            {
                continue;
            }

            AdmissionKind::One
        } else if matches!(row_index, 0x1E | 0x1F) {
            let entry = usize::try_from(i32::from(candidate.index as i16))
                .ok()
                .and_then(|id| items.get(id));

            let item_type = entry.as_ref().map_or(0, |entry| entry.item_type);

            let equip = entry.as_ref().map_or(0, |entry| entry.equip_mask);

            let shape = matches!(equip, 2 | 4 | 8 | 0x10 | 0x20 | 0x40 | 0x80 | 0xC0);

            if row_index == 0x1E {
                if !(item_type == 0x16 || shape)
                    || !fill.cells[0].item.is_empty()
                    || (fill.staged.item(0).is_empty() && item_type == 0x3F)
                {
                    continue;
                }
            } else if !(item_type == 0x18 || shape) {
                continue;
            }

            AdmissionKind::One
        } else if row.items[0] == 0 {
            if !first_effect_matches(&template, &candidate, items)
                || !requirement_matches(row_index, row, &candidate, items, effects)
                || !cell.item.is_empty()
            {
                continue;
            }

            AdmissionKind::One
        } else if !in_requirement_ranges(row, candidate_id) {
            if row.items[1] != 0
                || row.items[0] as u32 != candidate_id
                || !cell.item.is_empty()
                || !requirement_matches(row_index, row, &candidate, items, effects)
            {
                continue;
            }

            AdmissionKind::One
        } else {
            let count = ui::icon::stack_count(&candidate, items);

            if row_index != 0x37 || count != 100 {
                if !first_effect_matches(&template, &candidate, items)
                    || !requirement_matches(row_index, row, &candidate, items, effects)
                {
                    continue;
                }

                if row_index == 0x1B && candidate.index == 0x19D && count < 10 {
                    return None;
                }
            }

            if !cell.item.is_empty() {
                continue;
            }

            AdmissionKind::Counted { row: row_index }
        };

        fill.cells[need].item = candidate;

        let staged = (need..8).find(|&slot| fill.staged.item(slot).is_empty());

        if let Some(slot) = staged {
            fill.staged.put(slot, candidate, source as u8);
        }

        let count = match kind {
            AdmissionKind::One => 1,
            AdmissionKind::Counted { row } => {
                let stored = ui::icon::stack_count(&candidate, items).max(1) as u32;

                stored.min(list.requirements[row].required_count as u32)
            }
        };

        fill.cells[need].count = count;

        return Some(RecipeAdmission {
            need,
            staged,
            locked: source,
            count,
        });
    }

    None
}

#[derive(Debug, Clone, Copy)]

pub struct ManualInput<'a> {
    pub client: u16,
    pub now_ms: u32,
    pub cooldowns: item_move::Cooldowns,
    pub gold: i32,
    pub powder: Option<Item>,
    pub piece: Option<Item>,
    pub items: &'a ItemList,
    pub mixes: Option<&'a MixList>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct PowderSpend {
    pub at: Coord,
    pub after: Item,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct ManualSend {
    pub frame: Option<Vec<u8>>,
    pub spend: Option<PowderSpend>,
    pub cooldown_bucket: usize,
    pub now_ms: u32,
    pub broadcast_slot: Option<i32>,
    pub animate: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub enum Manual {
    Idle { settle: bool },
    Refused { strdef: usize },
    Sent(ManualSend),
}

fn halt(state: &mut Refine) {
    state.running = false;

    state.auto = false;
}

fn spent_powder(at: Coord, powder: Item, items: &ItemList) -> PowderSpend {
    let count = ui::icon::stack_count(&powder, items);

    let after = if count < 2 {
        Item::default()
    } else {
        Item {
            appearance: (count - 1) as u16,
            ..powder
        }
    };

    PowderSpend { at, after }
}

fn recipe_refusal(fill: &RecipeFill, list: &MixList) -> Option<usize> {
    let populated = fill.cells.iter().filter(|cell| cell.value != 0).count();

    if populated == 0 {
        return Some(NO_RECIPE_ITEMS);
    }

    if populated == 1 {
        let cell = fill.cells[0];

        if (1..=199).contains(&cell.value)
            && usize::try_from(cell.value)
                .ok()
                .and_then(|n| list.requirements.get(n))
                .is_some_and(|row| cell.count < row.required_count as u32)
        {
            return Some(NOT_ENOUGH_RECIPE_ITEMS);
        }

        return None;
    }

    for cell in 1..populated {
        if fill.staged.slot(cell).is_none() {
            return Some(NO_RECIPE_ITEMS);
        }

        let retained = fill.cells[cell];

        if (1..=199).contains(&retained.value)
            && usize::try_from(retained.value)
                .ok()
                .and_then(|n| list.requirements.get(n))
                .is_some_and(|row| retained.count < row.required_count as u32)
        {
            return Some(NOT_ENOUGH_RECIPE_ITEMS);
        }
    }

    None
}

const RECIPE_EXTRA_ITEM_AT: usize = 0x76;

const RECIPE_EXTRA_SLOT_AT: usize = 0x82;

pub fn manual_send(state: &mut Refine, input: ManualInput<'_>) -> Manual {
    let (Some(powder), Some(piece)) = (input.powder, input.piece) else {
        return Manual::Idle { settle: false };
    };

    let level = refine_level(&piece, input.items);

    if !((state.mode != 0 || (!powder.is_empty() && !piece.is_empty()))
        && level != state.target as u32
        && (level as i32) < 15)
    {
        return Manual::Idle { settle: true };
    }

    let effects = Effects::new(input.items, &[]);

    let kind = effects.total_scaled(&powder, effect::USE_KIND);

    let cooldown_bucket = item_move::cooldown_bucket(kind);

    let last_kind_ms = input.cooldowns.last[cooldown_bucket];

    let delay = if state.running && state.auto {
        state.auto_delay.milliseconds().min(REPEAT_MS)
    } else {
        REPEAT_MS
    };

    if last_kind_ms != 0 && input.now_ms.wrapping_sub(last_kind_ms) < delay {
        return Manual::Idle { settle: false };
    }

    let powder_id = i32::from(powder.index as i16);

    let powder_path = (state.mode < 1 && matches!(powder_id, 0x19C | 0x19D | 0x102D))
        || matches!(state.mode, 0x49 | 0x4A);

    let mut frame = None;

    let mut spend = None;

    let mut broadcast_slot = None;

    if powder_path {
        let preview = state.piece.preview;

        if !powder_matches(
            powder_id,
            i32::from(preview.index as i16),
            refine_level(&preview, input.items) as i32,
            effects.total_scaled(&preview, 0x70),
            effects.total_scaled(&preview, 0x71),
        ) {
            halt(state);

            return Manual::Refused {
                strdef: WRONG_POWDER,
            };
        }

        spend = Some(spent_powder(state.powder.at, powder, input.items));

        frame = Some(powder_frame(input.client, state.powder.at, state.piece.at));

        broadcast_slot = Some(state.powder.at.slot);
    } else if state.mode != 0 {
        let Some(row) = input.mixes.and_then(|list| {
            usize::try_from(state.mode)
                .ok()
                .and_then(|mode| list.recipe(mode))
        }) else {
            return Manual::Idle { settle: false };
        };

        if input.gold < row.gold_cost {
            halt(state);

            return Manual::Refused {
                strdef: NOT_ENOUGH_GOLD,
            };
        }

        let Some(fill) = state.recipe.as_mut() else {
            halt(state);

            return Manual::Refused {
                strdef: NO_RECIPE_ITEMS,
            };
        };

        fill.mode = state.mode;

        fill.staged.set_recipe(state.mode as u16, row.opcode);

        fill.staged.clear(0);

        if state.piece.at.container == item_move::container::INVENTORY {
            fill.staged.put(0, piece, state.piece.at.slot as u8);
        }

        let mut outgoing = fill.staged.frame();

        outgoing[0x06..0x08].copy_from_slice(&input.client.to_le_bytes());

        if state.piece.at.container == item_move::container::EQUIPMENT {
            outgoing[RECIPE_EXTRA_ITEM_AT..RECIPE_EXTRA_ITEM_AT + 12]
                .copy_from_slice(&piece.to_bytes());

            outgoing[RECIPE_EXTRA_SLOT_AT] = state.piece.at.slot as u8;
        }

        if let Some(strdef) = recipe_refusal(fill, input.mixes.expect("recipe row came from list"))
        {
            halt(state);

            return Manual::Refused { strdef };
        }

        frame = Some(outgoing);

        broadcast_slot = Some(state.powder.at.slot);
    }

    Manual::Sent(ManualSend {
        frame,
        spend,
        cooldown_bucket,
        now_ms: input.now_ms,
        broadcast_slot,
        animate: true,
    })
}

pub fn manual_after_notification(state: &mut Refine, powder: Item) {
    if state.auto {
        state.running = true;

        if state.mode < 1 {
            state.remembered_powder = i32::from(powder.index as i16);
        }
    }
}

#[derive(Debug, Clone, Copy)]

pub struct SettleInput<'a> {
    pub powder: Option<Item>,
    pub piece: Option<Item>,
    pub displayed_piece: Item,
    pub carry: &'a [Item],
    pub items: &'a ItemList,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Settlement {
    pub recovered_powder: Option<Coord>,
    pub rejected_powder: bool,
    pub cleared_powder: bool,
    pub halted: bool,
    pub refresh_powder: Option<Coord>,
    pub refresh_piece: Coord,
    pub stop_caption: &'static str,
}

pub fn settle(state: &mut Refine, input: SettleInput<'_>) -> Option<Settlement> {
    if !state.piece.selected || (!state.powder.selected && state.mode == 0) {
        return None;
    }

    let (Some(powder), Some(piece)) = (input.powder, input.piece) else {
        return None;
    };

    let was_running = state.running;

    let mut recovered_powder = None;

    let mut rejected_powder = false;

    let mut cleared_powder = false;

    let mut failed = false;

    if state.running {
        if state.remembered_powder != 0 && powder.is_empty() && state.mode == 0 {
            if let Some((slot, found)) = input
                .carry
                .iter()
                .copied()
                .take(64)
                .enumerate()
                .find(|(_, item)| i32::from(item.index as i16) == state.remembered_powder)
            {
                let count = input
                    .carry
                    .iter()
                    .take(64)
                    .filter(|item| item.index == found.index)
                    .map(super::consume::stored_count)
                    .sum::<i32>();

                let at = Coord {
                    container: item_move::container::INVENTORY,
                    slot: slot as i32,
                };

                if displayed_piece_accepts_powder(input.items, input.displayed_piece, found) {
                    state.retain_powder(
                        at,
                        Item {
                            index: found.index,
                            appearance: count as u16,
                            ..Default::default()
                        },
                    );

                    recovered_powder = Some(at);
                } else {
                    rejected_powder = true;

                    halt(state);
                }
            } else {
                failed = true;

                state.remembered_powder = 0;

                state.powder.preview = Item::default();

                cleared_powder = true;
            }
        }

        let level = refine_level(&piece, input.items);

        failed |= level > 14;

        if level == state.target as u32 || failed {
            halt(state);
        }
    }

    let halted = was_running && !state.running;

    Some(Settlement {
        recovered_powder,
        rejected_powder,
        cleared_powder,
        halted,
        refresh_powder: (state.mode == 0).then_some(state.powder.at),
        refresh_piece: state.piece.at,
        stop_caption: if state.auto {
            STOP_ON_CAPTION
        } else {
            STOP_OFF_CAPTION
        },
    })
}

pub fn affordable(list: &MixList, mode: i32, gold: i32) -> bool {
    let Some(row) = usize::try_from(mode).ok().and_then(|n| list.recipe(n)) else {
        return false;
    };

    gold >= row.gold_cost
}

pub fn cleared_labels() -> Vec<String> {
    (0..CELLS).map(|n| format!("MatText{n}")).collect()
}
