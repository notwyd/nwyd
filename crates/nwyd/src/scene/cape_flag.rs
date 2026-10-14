use formats::data::ItemList;
use formats::data::item_effect::Effects;
use formats::mesh::appearance;
use formats::net::account::Item;
use formats::texture::texture_list::TextureList;
use formats::texture::texture_set_list::TextureSetList;
use ui::Strings;
use ui::draw::Owner;

pub const TEXTURE_SET: usize = 0x41;

pub const HIDDEN_MESH: i16 = 0x13;

pub const DEFENSE_STRDEF: usize = 80;

pub const MAX_HP_STRDEF: usize = 81;

const DEFENSE_EFFECT: u8 = 3;

const MAX_HP_EFFECT: u8 = 4;

const SPECIAL_SELECTOR: i32 = 0x21;

const TEXTURE_SLOT_MAX: u32 = 0x200;

const SPECIAL_BODY_APPEARANCES: [u16; 4] = [0x0DAF, 0x0DB0, 0x0DB1, 0x0DB2];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Flag {
    pub texture_slot: Option<u32>,
    pub tooltip: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExternalImage {
    pub owner: Owner,
    pub texture_slot: u32,
    pub size: [u32; 2],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetState {
    Layer,
    External(ExternalImage),
}

pub fn selector(mesh_sub: i16) -> i32 {
    let mesh_sub = i32::from(mesh_sub);

    match mesh_sub / 10 {
        4 => 0x13,
        5 => 3,
        6 => 2,
        7 => 6,
        8 => 1,
        9 => 0,
        10 => 7,
        11 => 0x19,
        12 => 0x18,
        13 => 0x1A,
        14 => 0x1C,
        15 => 0x1B,
        16 => 0x1D,
        17 => 0x20,
        18 => 0x1F,
        19 => 0x21,
        _ => mesh_sub,
    }
}

pub fn resolve(
    loaded: Option<appearance::Cape>,
    body_appearance: u16,
    equipped: Item,
    items: &ItemList,
    sets: &TextureSetList,
    textures: &TextureList,
    strings: &Strings,
) -> Option<Flag> {
    let loaded = loaded.filter(|cape| cape.mesh != HIDDEN_MESH)?;

    let texture_slot = texture_slot(loaded, body_appearance, items, sets, textures);

    let name = items
        .get(usize::from(equipped.index))
        .map(|item| item.name)
        .unwrap_or_default();

    let effects = Effects::new(items, &[]);

    let defense = effects.total_scaled(&equipped, DEFENSE_EFFECT);

    let max_hp = effects.total_scaled(&equipped, MAX_HP_EFFECT);

    let label = |record| {
        strings
            .strdef
            .as_ref()
            .map_or("", |table| table.get(record))
    };

    let tooltip = format!(
        "<color 255 255 170>{name}</color><n>{} {defense}<n>{} {max_hp}",
        label(DEFENSE_STRDEF),
        label(MAX_HP_STRDEF),
    );

    Some(Flag {
        texture_slot,
        tooltip,
    })
}

pub fn texture_slot(
    loaded: appearance::Cape,
    body_appearance: u16,
    items: &ItemList,
    sets: &TextureSetList,
    textures: &TextureList,
) -> Option<u32> {
    let selector = if loaded.mesh == 2 && SPECIAL_BODY_APPEARANCES.contains(&body_appearance) {
        Some(SPECIAL_SELECTOR)
    } else {
        usize::try_from(loaded.item)
            .ok()
            .and_then(|item| items.get(item))
            .map(|item| selector(item.mesh_sub))
    };

    selector
        .and_then(|selector| usize::try_from(selector).ok())
        .and_then(|selector| sets.item(TEXTURE_SET, selector))
        .and_then(|item| u32::try_from(item.texture_index).ok())
        .filter(|slot| *slot <= TEXTURE_SLOT_MAX)
        .filter(|slot| textures.get(*slot as usize).is_some())
}
