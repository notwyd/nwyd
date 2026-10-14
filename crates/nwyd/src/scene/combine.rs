use crate::frame::PanelId;
use crate::scene::item_move::Coord;
use crate::scene::msgbox;
use crate::scene::panels::{Action, Panels};
use formats::data::item_effect::{Effects, effect, is_special_family};
use formats::data::item_list::ItemList;
use formats::mesh::appearance::refine_level;
use formats::net::account::Item;

pub const CELLS: [&str; 3] = ["Mat0", "Mat1", "Mat2"];

pub const COMBINE_BUTTON: &str = "Combine4";

pub const FRAME_LEN: usize = 0x84;

pub const ITEMS_AT: usize = 0x0E;

pub const ITEMS: usize = 8;

pub const INDEX_AT: usize = 0x0C;

pub const SLOTS_AT: usize = 0x6E;

pub const SLOTS: usize = 8;

pub const NO_SLOT: u8 = 0xFF;

pub const NOT_ENOUGH_ITEMS: usize = 274;

pub const REFINED_EQUIPMENT_WRONG_RESTRICTION: usize = 273;

pub const REFINED_EQUIPMENT_WRONG_GRADE_LETTER: usize = 302;

pub const REFINED_EQUIPMENT_INCOMPATIBLE: usize = 276;

pub const REFINED_EQUIPMENT_NEEDS_REFINEMENT: usize = 192;

pub const CONFIRM_STRDEF: usize = 319;

pub const OPTION_STONE_CONFIRM_STRDEF: usize = 1631;

pub const OPTION_STONE_ITEM: u16 = 0x1911;

pub const OPTION_STONE_EQUIP_MASKS: [u32; 7] = [4, 8, 0x10, 0x20, 0x40, 0x80, 0xC0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Kind {
    Mix,
    RefinedEquipment,
    OptionStone,
}

impl Kind {
    pub fn content(self) -> PanelId {
        match self {
            Kind::Mix => 0x19,
            Kind::RefinedEquipment => 0x1A,
            Kind::OptionStone => 0x36,
        }
    }

    pub fn cells(self) -> usize {
        match self {
            Kind::Mix => 8,
            Kind::RefinedEquipment | Kind::OptionStone => 3,
        }
    }

    pub fn opcode(self) -> Option<u16> {
        match self {
            Kind::Mix => None,
            Kind::RefinedEquipment => Some(0x3C0),
            Kind::OptionStone => Some(0x3FC),
        }
    }

    pub fn confirm_box(self) -> u32 {
        match self {
            Kind::Mix => 0x1A,
            Kind::RefinedEquipment => 0x1B,
            Kind::OptionStone => 0x45,
        }
    }

    pub fn required_items(self) -> usize {
        match self {
            Kind::Mix => 1,
            Kind::RefinedEquipment => 2,
            Kind::OptionStone => 1,
        }
    }

    pub fn slot_for(self, item: &Item, staged: &Staged) -> Option<usize> {
        match self {
            Kind::Mix | Kind::RefinedEquipment => {
                (0..self.cells()).find(|&cell| staged.item(cell).is_empty())
            }
            Kind::OptionStone if item.index == OPTION_STONE_ITEM => {
                let written =
                    item.effects[1] != Default::default() || item.effects[2] != Default::default();

                Some(match written {
                    true => 2,
                    false => 1,
                })
            }
            Kind::OptionStone => Some(0),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Staged {
    pub kind: Kind,
    bytes: Vec<u8>,
}

impl Staged {
    pub fn new(kind: Kind) -> Staged {
        let mut bytes = vec![0u8; FRAME_LEN];

        if let Some(opcode) = kind.opcode() {
            protocol::header::set_opcode(&mut bytes, opcode);
        }

        for slot in 0..SLOTS {
            bytes[SLOTS_AT + slot] = NO_SLOT;
        }

        Staged { kind, bytes }
    }

    pub fn item(&self, cell: usize) -> Item {
        if cell >= ITEMS {
            return Item::default();
        }

        let at = ITEMS_AT + cell * 12;

        read_item(&self.bytes[at..at + 12])
    }

    pub fn slot(&self, cell: usize) -> Option<u8> {
        match self.bytes.get(SLOTS_AT + cell) {
            Some(&NO_SLOT) | None => None,
            Some(&slot) => Some(slot),
        }
    }

    pub fn put(&mut self, cell: usize, item: Item, slot: u8) {
        if cell >= ITEMS {
            return;
        }

        let at = ITEMS_AT + cell * 12;

        self.bytes[at..at + 12].copy_from_slice(&item.to_bytes());

        self.bytes[SLOTS_AT + cell] = slot;
    }

    pub fn clear(&mut self, cell: usize) {
        if cell >= ITEMS {
            return;
        }

        let at = ITEMS_AT + cell * 12;

        self.bytes[at..at + 12].fill(0);

        self.bytes[SLOTS_AT + cell] = NO_SLOT;
    }

    pub fn frame(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    pub fn set_recipe(&mut self, index: u16, opcode: u16) {
        protocol::header::set_opcode(&mut self.bytes, opcode);

        self.bytes[INDEX_AT..INDEX_AT + 2].copy_from_slice(&index.to_le_bytes());
    }

    pub fn recipe(&self) -> u16 {
        u16::from_le_bytes([self.bytes[INDEX_AT], self.bytes[INDEX_AT + 1]])
    }

    pub fn any_slot(&self) -> bool {
        (0..SLOTS).any(|n| self.bytes[SLOTS_AT + n] != NO_SLOT)
    }
}

pub fn ready(staged: &Staged) -> Result<bool, usize> {
    let filled = (0..staged.kind.required_items()).all(|cell| !staged.item(cell).is_empty());

    match filled {
        true => Ok(staged.any_slot()),
        false => Err(NOT_ENOUGH_ITEMS),
    }
}

pub fn confirm(kind: Kind) -> msgbox::Request {
    msgbox::Request {
        caption: None,
        kind: kind.content() as i32,
        context: kind.confirm_box(),
        body: msgbox::Body::StrDef(if kind == Kind::OptionStone {
            OPTION_STONE_CONFIRM_STRDEF
        } else {
            CONFIRM_STRDEF
        }),
        flags: msgbox::FLAGS,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Staging {
    mix: Staged,
    refined_equipment: Staged,
    stone: Staged,
    option_stone: OptionStoneState,
}

impl Default for Staging {
    fn default() -> Staging {
        Staging {
            mix: Staged::new(Kind::Mix),
            refined_equipment: Staged::new(Kind::RefinedEquipment),
            stone: Staged::new(Kind::OptionStone),
            option_stone: OptionStoneState::default(),
        }
    }
}

impl Staging {
    pub fn get(&self, kind: Kind) -> &Staged {
        match kind {
            Kind::Mix => &self.mix,
            Kind::RefinedEquipment => &self.refined_equipment,
            Kind::OptionStone => &self.stone,
        }
    }

    pub fn get_mut(&mut self, kind: Kind) -> &mut Staged {
        match kind {
            Kind::Mix => &mut self.mix,
            Kind::RefinedEquipment => &mut self.refined_equipment,
            Kind::OptionStone => &mut self.stone,
        }
    }

    pub fn reset(&mut self, kind: Kind) {
        *self.get_mut(kind) = Staged::new(kind);

        if kind == Kind::OptionStone {
            self.option_stone = OptionStoneState::default();
        }
    }

    pub fn kind_of(content: PanelId) -> Option<Kind> {
        [Kind::Mix, Kind::RefinedEquipment, Kind::OptionStone]
            .into_iter()
            .find(|k| k.content() == content)
    }

    pub fn stage_option_stone(
        &mut self,
        item: Item,
        at: Coord,
        items: &ItemList,
    ) -> Result<OptionStoneReplacement, OptionStoneRefused> {
        stage_option_stone(&mut self.stone, &mut self.option_stone, item, at, items)
    }

    pub fn unstage_option_stone(&mut self, cell: usize) -> Option<u8> {
        self.option_stone.cleared(cell);

        unstage(&mut self.stone, cell)
    }

    pub fn option_stone_preview(&self, items: &ItemList) -> Option<OptionStonePreview> {
        option_stone_preview(&self.stone, items)
    }
}

pub fn stage(staged: &mut Staged, item: Item, at: Coord) -> Option<usize> {
    if item.is_empty() {
        return None;
    }

    let cell = staged.kind.slot_for(&item, staged)?;

    staged.put(cell, item, at.slot as u8);

    Some(cell)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum RefinedEquipmentRefused {
    Full,
    WrongRestriction,
    WrongGradeLetter,
    Incompatible,
    NeedsRefinement,
}

impl RefinedEquipmentRefused {
    pub fn strdef(self) -> Option<usize> {
        match self {
            RefinedEquipmentRefused::Full => None,
            RefinedEquipmentRefused::WrongRestriction => Some(REFINED_EQUIPMENT_WRONG_RESTRICTION),
            RefinedEquipmentRefused::WrongGradeLetter => Some(REFINED_EQUIPMENT_WRONG_GRADE_LETTER),
            RefinedEquipmentRefused::Incompatible => Some(REFINED_EQUIPMENT_INCOMPATIBLE),
            RefinedEquipmentRefused::NeedsRefinement => Some(REFINED_EQUIPMENT_NEEDS_REFINEMENT),
        }
    }
}

pub fn stage_refined_equipment(
    staged: &mut Staged,
    item: Item,
    at: Coord,
    items: &ItemList,
) -> Result<usize, RefinedEquipmentRefused> {
    assert_eq!(
        staged.kind,
        Kind::RefinedEquipment,
        "the RefinedEquipment admission rules belong to PanelItemMix4"
    );

    let cell = Kind::RefinedEquipment
        .slot_for(&item, staged)
        .ok_or(RefinedEquipmentRefused::Full)?;

    let effects = Effects::new(items, &[]);

    let entry = items.get(usize::from(item.index));

    match cell {
        0 => {
            if effects.total_scaled(&item, effect::RESTRICTION) != 1 {
                return Err(RefinedEquipmentRefused::WrongRestriction);
            }

            if effects.total_scaled(&item, effect::GRADE_LETTER) != 6 {
                return Err(RefinedEquipmentRefused::WrongGradeLetter);
            }

            if entry
                .as_ref()
                .is_none_or(|entry| entry.equip_mask & 0x3F != 0)
            {
                return Err(RefinedEquipmentRefused::Incompatible);
            }
        }
        1 => {
            let Some(entry) = entry.as_ref() else {
                return Err(RefinedEquipmentRefused::Incompatible);
            };

            if entry.grade / 10 < 4 {
                return Err(RefinedEquipmentRefused::Incompatible);
            }

            if effects.total_scaled(&item, effect::GRADE_LETTER) != 6 {
                return Err(RefinedEquipmentRefused::WrongGradeLetter);
            }

            let first = staged.item(0);

            if first.is_empty() {
                return Err(RefinedEquipmentRefused::Full);
            }

            let Some(first_entry) = items.get(usize::from(first.index)) else {
                return Err(RefinedEquipmentRefused::Incompatible);
            };

            let grade_matches = first_entry.grade == entry.grade
                || (first_entry.grade == 0x2D && entry.grade == 0x30);

            if first_entry.equip_mask != entry.equip_mask || !grade_matches {
                return Err(RefinedEquipmentRefused::Incompatible);
            }
        }
        2 => {}
        _ => unreachable!("PanelItemMix4 has exactly three cells"),
    }

    if refine_level(&item, items) <= 8 || entry.as_ref().is_none_or(|entry| entry.equip_mask == 0) {
        return Err(RefinedEquipmentRefused::NeedsRefinement);
    }

    staged.put(cell, item, at.slot as u8);

    Ok(cell)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct OptionStoneState {
    blank: bool,
    written: bool,
}

impl OptionStoneState {
    pub fn blank(&self) -> bool {
        self.blank
    }

    pub fn written(&self) -> bool {
        self.written
    }

    fn cleared(&mut self, cell: usize) {
        match cell {
            1 => self.blank = false,
            2 => self.written = false,
            _ => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum OptionStoneRefused {
    Incompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct OptionStoneReplacement {
    pub cell: usize,
    pub replaced: Option<u8>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct OptionStonePreview {
    pub piece: Item,
    pub stone: Item,
}

fn option_bytes_empty(item: &Item) -> bool {
    item.effects[1] == Default::default() && item.effects[2] == Default::default()
}

pub fn stage_option_stone(
    staged: &mut Staged,
    state: &mut OptionStoneState,
    item: Item,
    at: Coord,
    items: &ItemList,
) -> Result<OptionStoneReplacement, OptionStoneRefused> {
    assert_eq!(
        staged.kind,
        Kind::OptionStone,
        "the OptionStone admission rules belong to PanelItemOptionStone",
    );

    let effects = Effects::new(items, &[]);

    let cell = if item.index == OPTION_STONE_ITEM {
        if option_bytes_empty(&item) {
            state.blank = true;

            let piece = staged.item(0);

            if !piece.is_empty() && option_bytes_empty(&piece) {
                return Err(OptionStoneRefused::Incompatible);
            }

            1
        } else {
            state.written = true;

            let piece = staged.item(0);

            if !piece.is_empty() {
                let id = i32::from(piece.index as i16);

                if !is_special_family(id)
                    || effects.total_scaled(&piece, effect::EQUIP_MASK)
                        != i32::from(item.effects[0].value)
                    || effects.total_scaled(&piece, effect::GRADE_LETTER)
                        != i32::from(item.reserved.to_le_bytes()[1])
                {
                    return Err(OptionStoneRefused::Incompatible);
                }
            }

            2
        }
    } else {
        let id = i32::from(item.index as i16);

        let Some(entry) = items.get(usize::from(item.index)) else {
            return Err(OptionStoneRefused::Incompatible);
        };

        let equip_mask = entry.equip_mask;

        let grade = effects.total_scaled(&item, effect::GRADE_LETTER);

        if !OPTION_STONE_EQUIP_MASKS.contains(&equip_mask) || !is_special_family(id) {
            return Err(OptionStoneRefused::Incompatible);
        }

        let written = staged.item(2);

        if !written.is_empty() && written.effects[0].id == effect::EQUIP_MASK {
            let stored_mask = written.effects[0].value;

            let equip_matches = match stored_mask {
                0x40 | 0xC0 => matches!(equip_mask, 0x40 | 0xC0),
                other => equip_mask == u32::from(other),
            };

            if !equip_matches || grade != i32::from(written.reserved.to_le_bytes()[1]) {
                return Err(OptionStoneRefused::Incompatible);
            }
        }

        if state.blank && option_bytes_empty(&item) {
            return Err(OptionStoneRefused::Incompatible);
        }

        0
    };

    let replaced = staged.slot(cell);

    staged.put(cell, item, at.slot as u8);

    Ok(OptionStoneReplacement { cell, replaced })
}

pub fn option_stone_preview(staged: &Staged, items: &ItemList) -> Option<OptionStonePreview> {
    assert_eq!(
        staged.kind,
        Kind::OptionStone,
        "the OptionStone preview belongs to PanelItemOptionStone",
    );

    let piece = staged.item(0);

    if piece.is_empty() {
        return None;
    }

    let blank = staged.item(1);

    let written = staged.item(2);

    if blank.is_empty() && written.is_empty() {
        return None;
    }

    let mut result_piece = piece;

    if written.is_empty() {
        result_piece.effects[1] = Default::default();

        result_piece.effects[2] = Default::default();
    } else {
        result_piece.effects[1] = written.effects[1];

        result_piece.effects[2] = written.effects[2];
    }

    if blank.is_empty() {
        return Some(OptionStonePreview {
            piece: result_piece,
            stone: Default::default(),
        });
    }

    let effects = Effects::new(items, &[]);

    let grade = effects.total_scaled(&piece, effect::GRADE_LETTER) as u8;

    let equip_mask = items
        .get(usize::from(piece.index))
        .map_or(0, |entry| entry.equip_mask as u8);

    let mut result_stone = piece;

    result_stone.index = OPTION_STONE_ITEM;

    result_stone.reserved = u16::from_le_bytes([piece.reserved.to_le_bytes()[0], grade]);

    result_stone.effects[0] = formats::net::account::Effect {
        id: effect::EQUIP_MASK,
        value: equip_mask,
    };

    Some(OptionStonePreview {
        piece: result_piece,
        stone: result_stone,
    })
}

pub fn unstage(staged: &mut Staged, cell: usize) -> Option<u8> {
    let slot = staged.slot(cell);

    staged.clear(cell);

    slot
}

pub fn wire(staged: &Staged) -> Vec<u8> {
    staged.frame()
}

pub fn closing(panels: &mut Panels) -> Vec<Action> {
    match panels.is_open(super::panels::CONTENT_INVENTORY) {
        true => vec![Action::SetOpen(super::panels::CONTENT_INVENTORY, false)],
        false => Vec::new(),
    }
}

pub(crate) fn read_item(bytes: &[u8]) -> Item {
    let u16_at = |at: usize| u16::from_le_bytes([bytes[at], bytes[at + 1]]);

    let mut effects = [formats::net::account::Effect::default(); 3];

    for (n, effect) in effects.iter_mut().enumerate() {
        *effect = formats::net::account::Effect {
            id: bytes[6 + n * 2],
            value: bytes[7 + n * 2],
        };
    }

    Item {
        index: u16_at(0),
        appearance: u16_at(2),
        reserved: u16_at(4),
        effects,
    }
}
