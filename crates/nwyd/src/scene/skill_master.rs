use crate::frame::PanelId;
use crate::scene::msgbox;
use crate::scene::panels::Panels;
use crate::scene::spend;
use formats::net::account::Item;
use formats::net::shop::{STOCK_SLOTS, Stock};
use formats::ui::pane::ElementType;

pub const CONTENT: PanelId = 0x16;

pub const SCRIPT: &[u8] = b"ui/Panel/game/SkillMaster.pane";

pub const BUTTONS: usize = 24;

pub const GROUPS: usize = 3;

pub const STRIDE: usize = 9;

pub const OPEN: u16 = super::interact::OPEN_ITEM_SHOP;

pub const PROMPT_STRDEF: usize = 47;

pub const KIND: i32 = 0x16;

pub const CONTEXT: u32 = 0x16;

pub const FLAGS: u32 = 1;

pub fn learnable(id: i32) -> bool {
    let id = id as u32;

    id.wrapping_sub(5000) < 0x69 || id.wrapping_sub(0x1518) <= 0x2F
}

pub const GROUP_STRDEF: [[usize; GROUPS]; 4] = [
    [107, 108, 109],
    [110, 111, 112],
    [113, 114, 115],
    [133, 134, 135],
];

pub fn class_of(first: i16) -> usize {
    let index = (i32::from(first) - 5000) / 24;

    match u32::try_from(index) {
        Ok(n) if n <= 3 => n as usize,
        _ => 0,
    }
}

pub fn button_of(slot: usize) -> Option<usize> {
    (slot % STRIDE != STRIDE - 1).then(|| (slot % STRIDE) + (slot / STRIDE) * (STRIDE - 1))
}

pub fn button_name(index: usize) -> String {
    format!("SkillBtn{index}")
}

pub const CLEARED: i32 = -1;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Master {
    pub npc: u16,
    pub pending: Option<Pending>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Pending {
    pub item: i32,
    pub npc: u16,
}

pub const DESC_FILE: &str = "descskillm.txt";

pub const DESC: &str = "Desc";

pub fn on_open(panels: &mut Panels) {
    let text = panels.skill_master_desc.clone();

    if let Some(screen) = panels.screen_mut(CONTENT) {
        super::screens::text(screen, DESC, &text);
    }
}

pub fn open_request(master: &mut Master, npc: u16) -> Vec<u8> {
    master.npc = npc;

    super::interact::frame(OPEN, super::interact::INTERACT_LEN, u32::from(npc))
}

pub const HEAL: u16 = 0x13A3;

pub const ROTATION: usize = 3;

pub fn rotate_heal(items: &[Item; STOCK_SLOTS]) -> [Item; STOCK_SLOTS] {
    let mut out = *items;

    let Some(at) = items.iter().position(|item| item.index == HEAL) else {
        return out;
    };

    let read = |slot: usize| items.get(slot).copied().unwrap_or_default();

    let window: [Item; ROTATION] = [read(at + 1), read(at + 2), read(at)];

    for (step, item) in window.into_iter().enumerate() {
        if let Some(slot) = out.get_mut(at + step) {
            *slot = item;
        }
    }

    out
}

pub fn fill(panels: &mut Panels, stock: &Stock) -> usize {
    let items = rotate_heal(&stock.items);

    let mut painted: [Option<ui::icon::IconContents>; BUTTONS] = [None; BUTTONS];

    for (slot, item) in items.iter().enumerate() {
        let Some(button) = button_of(slot) else {
            continue;
        };

        let Some(cell) = painted.get_mut(button) else {
            continue;
        };

        let id = i32::from(item.index as i16);

        if id > 0 {
            *cell = ui::icon::set_skill_icon(id, &panels.items);
        }
    }

    let filled = painted.iter().flatten().count();

    let class = class_of(items.first().map_or(0, |item| item.index as i16));

    let labels = GROUP_STRDEF[class];

    let strings = panels.strings.clone();

    if let Some(screen) = panels.screen_mut(CONTENT) {
        for (button, contents) in painted.iter().enumerate() {
            if let Some(element) = screen.element_mut(&button_name(button), ElementType::IconButton)
            {
                element.set_icon_contents(*contents);
            }
        }

        for (group, record) in labels.iter().enumerate() {
            let text = strings.translate(&format!("ss_{record}"));

            super::screens::text(screen, &format!("SkillName{}", group + 1), &text);
        }
    }

    filled
}

pub fn clicked(panels: &mut Panels, name: &str) -> Option<Pending> {
    let index = (0..BUTTONS).find(|i| button_name(*i).eq_ignore_ascii_case(name))?;

    let item = panels
        .screen(CONTENT)
        .and_then(|screen| screen.element(&button_name(index), ElementType::IconButton))
        .and_then(|element| element.widget.button())
        .and_then(|state| state.icon_contents)
        .map_or(CLEARED, |contents| contents.index);

    if !(1..=0x38A3).contains(&item) || !learnable(item) {
        return None;
    }

    let pending = Pending {
        item,
        npc: panels.master.npc,
    };

    let name = panels.items.name(item as usize).unwrap_or_default();

    let body = panels
        .strings
        .translate(&format!("ss_{PROMPT_STRDEF}"))
        .replacen("%s", &name, 1);

    let request = msgbox::Request {
        caption: None,
        kind: KIND,
        context: CONTEXT,
        body: msgbox::Body::Text(body),
        flags: FLAGS,
    };

    let slot = panels.raise_box(&request, None)?;

    panels.box_skills[slot] = Some(pending);

    panels.master.pending = Some(pending);

    Some(pending)
}

pub fn clicked_owner(panels: &mut Panels, owner: ui::draw::Owner) -> Option<Pending> {
    let ui::draw::Owner::Element(owner) = owner else {
        return None;
    };

    let screen = panels.screen(CONTENT)?;

    let element = screen.placed.ui.panel.elements.get(owner)?;

    let name = (0..24).map(button_name).find(|name| {
        screen
            .element(name, ElementType::IconButton)
            .is_some_and(|bound| std::ptr::eq(bound, element))
    })?;

    clicked(panels, &name)
}

pub fn confirm(panels: &mut Panels, pending: Pending) -> Option<Pending> {
    if !panels.is_open(CONTENT) || pending.npc != panels.master.npc {
        return None;
    }

    let item = u16::try_from(pending.item).ok()?;

    panels.wire.push(spend::frame(
        spend::KIND_SKILL,
        item,
        u32::from(pending.npc),
    ));

    Some(pending)
}
