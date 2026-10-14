use crate::element::Element;
use crate::icon::IconContents;
use crate::widget::cell_group;

pub mod origin_screen {

    pub const SHEET: i32 = 0;

    pub const INVENTORY: i32 = 1;
}

pub const ITEM_KIND: i32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Carried {
    pub screen: i32,
    pub kind: i32,
    pub group: i32,
    pub slot: i32,
    pub index: i32,
    pub slot_type: i32,
    pub size: (i32, i32),
    pub contents: IconContents,
}

impl Carried {
    pub fn pick_up(screen: i32, element: &Element) -> Option<Carried> {
        let state = element.widget.button()?;

        let binding = state.tooltip_binding()?;

        let contents = state.icon_contents.unwrap_or(IconContents {
            kind: binding.kind,
            item: binding.item,
            index: binding.index,
            count: 0,
            refine: -1,
            sheet: 0,
            sheet_uv: [0; 4],
            grade: Default::default(),
            overlay: None,
            asking_price: crate::icon::NO_ASKING_PRICE,
            sweep: None,
            blink_off: false,
        });

        Some(Carried {
            screen,
            kind: binding.kind,
            group: state.cell_group,
            slot: i32::from(state.icon_slot_index),
            index: binding.index,
            slot_type: state.icon_slot_type,
            size: (element.width, element.height),
            contents,
        })
    }

    pub fn came_from(&self, group: i32, slot: i32) -> bool {
        self.kind == ITEM_KIND && self.group == group && self.slot == slot
    }

    pub fn rect(&self, cursor: (i32, i32)) -> [i32; 4] {
        let (w, h) = self.draw_size();

        let offset = if self.kind == ITEM_KIND {
            (quarter(w), quarter(h))
        } else {
            (w / 2, h / 2)
        };

        let (left, top) = (cursor.0 - offset.0, cursor.1 - offset.1);

        [left, top, left + w, top + h]
    }

    pub fn draw_size(&self) -> (i32, i32) {
        if self.kind == crate::icon::SKILL_KIND {
            (40, 40)
        } else {
            self.size
        }
    }

    pub fn from_a_container(&self) -> bool {
        matches!(
            self.group,
            cell_group::EQUIPMENT | cell_group::INVENTORY | cell_group::CARGO
        )
    }

    pub fn container(&self) -> i32 {
        self.group - 1
    }
}

fn quarter(value: i32) -> i32 {
    value.div_euclid(4) + i32::from(value < 0 && value % 4 != 0)
}

pub const CARRY_COLOR: u32 = 0xC0FF_FFFF;
