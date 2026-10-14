use crate::resource::Resource;
use formats::data::item_icon::{self, IconTable};
use formats::data::item_list::{ItemList, item_type};
use formats::mesh::appearance::{refine_level, refine_rank};
use formats::net::account::Item;

pub const ID_RANGE: std::ops::RangeInclusive<i32> = 1..=0x38A3;

pub const REFINE_ALWAYS: [i32; 3] = [0xCE5, 0x149F, 0x1718];

pub const MAX_STACK: i32 = 500;

pub const MIN_SHOWN_STACK: i32 = 2;

pub const MAX_GRADE_FRAME: i32 = 9;

pub const REFINE_HIGHLIGHT: i32 = 10;

pub const LABEL_WHITE: u32 = 0xFFFF_FFFF;

pub const LABEL_YELLOW: u32 = 0xFFFF_FF55;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AliasRect {
    pub texture_index: i32,
    pub uv: [i32; 4],
}

impl AliasRect {
    fn resolve(resource: &Resource, name: &str) -> AliasRect {
        let (texture_index, uv) = resource.alias_rect(name);

        AliasRect { texture_index, uv }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollectionIcon {
    pub state: i8,
    pub mark: Option<AliasRect>,
}

impl CollectionIcon {
    pub fn resolve(state: i8, resource: &Resource) -> Self {
        Self {
            state,
            mark: match state {
                2 => Some(AliasRect::resolve(resource, "Collection.S2")),
                3 => Some(AliasRect::resolve(resource, "Collection.S3")),
                _ => None,
            },
        }
    }
}

pub fn set_picture_icon(slot: i32) -> Option<IconContents> {
    if slot < 0 {
        return None;
    }

    let x = slot % 10 * 100;

    let y = (slot % 100) / 10 * 100;

    Some(IconContents {
        kind: 6,
        item: Item::default(),
        index: 1,
        count: -1,
        refine: -1,
        sheet: slot / 100 + 1,
        sheet_uv: [x, y, x + 100, y + 100],
        grade: AliasRect::default(),
        overlay: None,
        asking_price: -1,
        sweep: None,
        blink_off: false,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooltipBinding {
    pub kind: i32,
    pub index: i32,
    pub item: Item,
    pub help_id: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IconContents {
    pub kind: i32,
    pub item: Item,
    pub index: i32,
    pub count: i32,
    pub refine: i32,
    pub sheet: i32,
    pub sheet_uv: [i32; 4],
    pub grade: AliasRect,
    pub overlay: Option<AliasRect>,
    pub asking_price: i32,
    pub sweep: Option<Sweep>,
    pub blink_off: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sweep {
    pub elapsed_ms: u32,
    pub cooldown_s: u32,
}

impl Sweep {
    pub fn served(self) -> f32 {
        self.elapsed_ms as f32 / (self.cooldown_s as f32 * 1000.0)
    }
}

pub const NO_ASKING_PRICE: i32 = -1;

impl IconContents {
    pub fn label(&self) -> Option<(String, u32)> {
        if self.refine > 0 {
            let colour = if self.refine < REFINE_HIGHLIGHT {
                LABEL_WHITE
            } else {
                LABEL_YELLOW
            };

            return Some((format!("+{}", self.refine), colour));
        }

        (self.count > 0).then(|| (self.count.to_string(), LABEL_WHITE))
    }
}

pub fn stack_count(item: &Item, items: &ItemList) -> i32 {
    let id = i32::from(item.index as i16);

    let Some(entry) = usize::try_from(id).ok().and_then(|id| items.get(id)) else {
        return 0;
    };

    if entry.equip_mask != 0 && entry.item_type != 0x37 {
        return 0;
    }

    if entry.item_type != 0 && entry.item_type <= 0x31 {
        return 0;
    }

    let amount = i32::from(item.appearance as i16);

    let amount = if !(1..=MAX_STACK).contains(&amount) {
        1
    } else {
        amount
    };

    if amount < MAX_STACK { amount } else { 0 }
}

fn overlay_tier(item: &Item, items: &ItemList) -> i32 {
    let id = i32::from(item.index as i16);

    let Some(entry) = usize::try_from(id).ok().and_then(|id| items.get(id)) else {
        return 0;
    };

    let mut tier = i32::from(entry.tier);

    let refine = refine_level(item, items).min(12) as i32;

    if (entry.grade == 0x33 || tier != 0) && tier < 5 && refine > 9 {
        tier = i32::from(refine_rank(item, items)) + 4;
    }

    let marker = first_marker_effect(item);

    if (0x74..=0x7D).contains(&marker) {
        tier = marker;
    }

    if matches!(
        entry.item_type,
        item_type::PET_STAGE1 | item_type::PET_STAGE2
    ) {
        tier = 0;
    }

    tier
}

fn first_marker_effect(item: &Item) -> i32 {
    let ids = [item.effects[0].id, item.effects[1].id, item.effects[2].id];

    if ids.contains(&b'+') {
        return i32::from(b'+');
    }

    ids.iter()
        .copied()
        .find(|&id| (0x73..0x7F).contains(&id))
        .map_or(0, i32::from)
}

fn overlay_alias(tier: i32) -> Option<String> {
    match tier {
        0x74..=0x7D => Some(format!("UI_Icon.IconRectC{}", tier - 0x74)),
        5 | 9 => Some(String::from("UI_Icon.IconRectJ0")),
        6 | 10 => Some(String::from("UI_Icon.IconRectJ1")),
        _ => None,
    }
}

fn takes_a_refine(item: &Item, items: &ItemList) -> bool {
    let id = i32::from(item.index as i16);

    if REFINE_ALWAYS.contains(&id) {
        return true;
    }

    usize::try_from(id)
        .ok()
        .and_then(|id| items.get(id))
        .is_some_and(|e| e.equip_mask != 0)
}

pub fn set_icon(
    item: &Item,
    items: &ItemList,
    icons: &IconTable,
    resource: &Resource,
) -> Option<IconContents> {
    let index = i32::from(item.index as i16);

    if !ID_RANGE.contains(&index) {
        return None;
    }

    let count = match stack_count(item, items) {
        n if n < MIN_SHOWN_STACK => 0,
        n => n,
    };

    let refine = match takes_a_refine(item, items) {
        true => refine_level(item, items) as i32,
        false => -1,
    };

    let overridden = item.effects[1].id != 0 || item.effects[2].id != 0;

    let icon = icons.resolve(index as usize, overridden);

    let grade = AliasRect::resolve(
        resource,
        &format!("UI_Icon.IconRect{}", refine.clamp(0, MAX_GRADE_FRAME)),
    );

    let overlay =
        overlay_alias(overlay_tier(item, items)).map(|name| AliasRect::resolve(resource, &name));

    Some(IconContents {
        kind: ITEM_KIND,
        item: *item,
        index,
        count,
        refine,
        sheet: icon.sheet,
        sheet_uv: [
            icon.u0 as i32,
            icon.v0 as i32,
            icon.u1 as i32,
            icon.v1 as i32,
        ],
        grade,
        overlay,
        asking_price: NO_ASKING_PRICE,
        sweep: None,
        blink_off: false,
    })
}

pub const CELL: u32 = item_icon::CELL;

pub const ITEM_KIND: i32 = 1;

pub const AFFECT_KIND: i32 = 4;

pub const SKILL_KIND: i32 = 3;

pub fn set_affect_icon(kind: u8, slot: i32, seconds: i32) -> Option<IconContents> {
    let sheet_uv = formats::net::affect::icon_cell(kind)?;

    Some(IconContents {
        kind: AFFECT_KIND,
        item: Item::default(),
        index: i32::from(kind),
        count: slot,
        refine: seconds,
        sheet: AFFECT_SHEET,
        sheet_uv,
        grade: AliasRect::default(),
        overlay: None,
        asking_price: NO_ASKING_PRICE,
        sweep: None,
        blink_off: false,
    })
}

pub const AFFECT_SHEET: i32 = -2;

impl IconContents {
    pub fn countdown(&self) -> Option<String> {
        if self.kind != AFFECT_KIND {
            return None;
        }

        let text = formats::net::affect::countdown(self.refine);

        (!text.is_empty()).then_some(text)
    }
}

pub const SKILL_ID_RANGE: std::ops::RangeInclusive<i32> = 0..=0x38A3;

pub const SKILL_SHEET: i32 = -1;

pub const SKILL_SHEET_ON: i32 = AFFECT_SHEET;

pub const BLANK_CELL: [i32; 4] = [0, 0, 2, 2];

pub const SKILL_NO_LABEL: i32 = -1;

pub fn set_skill_icon(id: i32, items: &ItemList) -> Option<IconContents> {
    if !SKILL_ID_RANGE.contains(&id) {
        return None;
    }

    let entry = usize::try_from(id).ok().and_then(|id| items.get(id))?;

    let number = formats::net::affect::remap(i32::from(entry.mesh_sub));

    Some(IconContents {
        kind: SKILL_KIND,
        item: Item {
            index: id as u16,
            ..Item::default()
        },
        index: id,
        count: SKILL_NO_LABEL,
        refine: SKILL_NO_LABEL,
        sheet: SKILL_SHEET,
        sheet_uv: match entry.name.is_empty() {
            true => BLANK_CELL,
            false => formats::net::affect::cell_of(number),
        },
        grade: AliasRect::default(),
        overlay: None,
        asking_price: NO_ASKING_PRICE,
        sweep: None,
        blink_off: false,
    })
}

pub const SWEEP_TURN: f32 = std::f32::consts::TAU;

impl IconContents {
    pub fn sweep_angle(&self) -> Option<f32> {
        let served = self.sweep?.served();

        Some((1.0 - served).fract() * SWEEP_TURN)
    }
}
