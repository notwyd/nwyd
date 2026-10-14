use super::field::FieldScene;
use assets::GameDir;
use formats::data::collection::{self, Entry};
use formats::net::collection::Progress;

pub const TABLE: &[u8] = b"CollectionData.bin";

pub fn load(game: &GameDir) -> Vec<Entry> {
    match game.read(TABLE) {
        Ok(raw) => collection::parse(&raw),
        Err(_) => collection::parse(&[]),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Book {
    pub table: Vec<Entry>,
    pub progress: Option<Progress>,
}

fn costume_item(scene: &FieldScene) -> u16 {
    scene
        .player
        .and_then(|index| scene.world.actors.actors.get(index))
        .map_or(0, |actor| actor.costume_item)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Filter {
    #[default]
    All,
    Incomplete,
    Complete,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Row {
    pub index: usize,
    pub payload: i32,
    pub complete: Option<bool>,
}

pub fn split_for(costume_item: u16) -> i32 {
    match i32::from(costume_item) % 10 > 5 {
        true => 1,
        false => 0,
    }
}

pub fn rows(table: &[Entry], progress: Option<&Progress>, split: i32, filter: Filter) -> Vec<Row> {
    let mut out = Vec::new();

    for (index, entry) in table.iter().enumerate() {
        if !entry.is_listed() || entry.class_split() != split {
            continue;
        }

        let complete = entry
            .bit_index()
            .and_then(|b| progress.and_then(|p| p.complete(b)));

        let skip = matches!(
            (filter, complete),
            (Filter::Incomplete, Some(true)) | (Filter::Complete, Some(false))
        );

        if !skip {
            out.push(Row {
                index,
                payload: entry.payload(),
                complete,
            });
        }
    }

    out
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Restated {
    pub entity: u16,
    pub completed: usize,
    pub handled: bool,
    pub refreshed: bool,
    pub listed: usize,
}

pub fn replace(scene: &mut FieldScene, entity: u16, progress: &Progress) -> Restated {
    scene.collection.progress = Some(*progress);

    let screen = scene
        .panels
        .as_ref()
        .and_then(|panels| panels.screen(CONTENT));

    let handled = screen.is_some();

    let refreshed = screen.is_some_and(|screen| screen.open);

    if refreshed {
        if let Some(panels) = scene.panels.as_mut() {
            panels.collection_dirty = true;
        }

        panel_refresh(scene);
    }

    let split = split_for(costume_item(scene));

    let listed = rows(&scene.collection.table, Some(progress), split, Filter::All).len();

    Restated {
        entity,
        completed: progress.completed(),
        handled,
        refreshed,
        listed,
    }
}

pub const CONTENT: crate::frame::PanelId = 0x38;

pub fn describe(applied: &Restated) -> String {
    format!(
        "NWYD: 0x300 — {} of {} collection records complete, {} row(s) the book would list, \
         reported {}",
        applied.completed,
        formats::net::collection::FLAG_BYTES,
        applied.listed,
        match (applied.handled, applied.refreshed) {
            (false, _) => "unhandled (no PanelCollection)",
            (true, false) => "handled, book shut",
            (true, true) => "handled, book rebuilt",
        },
    )
}

pub const LIST_VIEW: &str = "ListView22";

pub const SIGN_UP: &str = "SignUp";

pub const ALL_BUTTON: &str = "All";

pub const INCOMPLETE_BUTTON: &str = "Incomplete";

pub const COMPLETE_BUTTON: &str = "Complete";

pub const ITEM_CELLS: usize = formats::data::collection::NEEDED;

pub const COUNT_LABEL: &str = "CollectionCount";

pub const REWARD_LABELS: [&str; 2] = ["CollectionReward1", "CollectionReward2"];

pub fn reward_label(effect: i32, value: i32, strings: &ui::Strings) -> String {
    let label = formats::data::item_effect::ROWS
        .iter()
        .skip(1)
        .find(|row| i32::from(row.id) == effect)
        .map(|row| strings.translate(&format!("ss_{}", row.strdef)))
        .unwrap_or_default();

    match effect {
        0x5B | 0x3C | 0x2D | 0x2E => format!("{label} : {value}%"),
        0x28 | 0x2A | 0x1C | 0x1A | 0x38 | 0x39 => {
            format!("{label} : {}.{}%", value / 10, value % 10)
        }
        _ => format!("{label} : {value}"),
    }
}

pub const RESULT_IMAGE: &str = "Result";

pub const COMPLETE_ARGB: u32 = 0xFFE6_C54D;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Count {
    pub registered: i32,
    pub required: i32,
}

impl Count {
    pub fn label(self) -> String {
        format!("{}/{}", self.registered, self.required)
    }
}

fn registered(entry: &Entry, progress: Option<&Progress>) -> i32 {
    let Some(byte) = entry.bit_index() else {
        return 0;
    };

    entry
        .need
        .iter()
        .enumerate()
        .filter(|(cell, need)| {
            need.index != 0 && progress.and_then(|p| p.slot(byte, *cell)).unwrap_or(false)
        })
        .count() as i32
}

pub fn rebuild_count(entry: &Entry, progress: Option<&Progress>) -> Count {
    Count {
        registered: registered(entry, progress),
        required: entry.required_count,
    }
}

pub fn refresh_count(entry: &Entry, progress: Option<&Progress>) -> Count {
    Count {
        registered: registered(entry, progress),
        required: entry.need.iter().filter(|need| need.index != 0).count() as i32,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cell {
    Missing = 1,
    Have = 2,
    Registered = 3,
}

pub fn cells(
    entry: &Entry,
    progress: Option<&Progress>,
    backpack: &[formats::net::account::Item],
    items: &formats::data::ItemList,
) -> [Cell; ITEM_CELLS] {
    let index = entry.bit_index();

    let mut out = [Cell::Missing; ITEM_CELLS];

    for (cell, state) in out.iter_mut().enumerate() {
        if index
            .and_then(|b| progress.and_then(|p| p.slot(b, cell)))
            .unwrap_or(false)
        {
            *state = Cell::Registered;

            continue;
        }

        *state = match matching_slot(&entry.need[cell], backpack, items).is_some() {
            true => Cell::Have,
            false => Cell::Missing,
        };
    }

    out
}

fn matching_slot(
    need: &formats::data::Item,
    backpack: &[formats::net::account::Item],
    items: &formats::data::ItemList,
) -> Option<usize> {
    backpack.iter().take(60).position(|held| {
        let id = i32::from(held.index as i16);

        (1..0x38A4).contains(&id)
            && items
                .get(id as usize)
                .map(|e| e.base_id as i32)
                .unwrap_or(0)
                == i32::from(need.index)
            && if items
                .get(usize::from(need.index))
                .is_some_and(|entry| entry.item_type == 0x16)
            {
                need.to_bytes()[8] == held.effects[1].id
            } else {
                formats::mesh::appearance::refine_level(
                    &super::combine::read_item(&need.to_bytes()),
                    items,
                ) == formats::mesh::appearance::refine_level(held, items)
            }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Filter(Filter),
    Nothing,
    Ask,
}

pub fn command(filter: Filter, name: &str) -> Option<Command> {
    for (button, wanted) in [
        (ALL_BUTTON, Filter::All),
        (INCOMPLETE_BUTTON, Filter::Incomplete),
        (COMPLETE_BUTTON, Filter::Complete),
    ] {
        if !name.eq_ignore_ascii_case(button) {
            continue;
        }

        return Some(match filter == wanted {
            true => Command::Nothing,
            false => Command::Filter(wanted),
        });
    }

    match name.eq_ignore_ascii_case(SIGN_UP) {
        true => Some(Command::Ask),
        false => None,
    }
}

pub const SIGN_UP_BOX: u32 = 0x46;

pub const SIGN_UP_STRDEF: usize = 1635;

pub fn confirm() -> super::msgbox::Request {
    super::msgbox::Request {
        caption: None,
        kind: CONTENT as i32,
        context: SIGN_UP_BOX,
        body: super::msgbox::Body::StrDef(SIGN_UP_STRDEF),
        flags: super::msgbox::FLAGS,
    }
}

pub const SIGN_UP_OPCODE: u16 = 0x3FE;

pub const SIGN_UP_LEN: usize = 100;

pub const SIGN_UP_INDEX_AT: usize = 0x0C;

pub const SIGN_UP_SLOT_AT: usize = 0x10;

pub const SIGN_UP_ITEMS_AT: usize = 0x14;

pub const SIGN_UP_SLOTS_AT: usize = 0x5C;

pub fn sign_up_frame(
    entry: &Entry,
    index: usize,
    cell: usize,
    states: &[Cell; ITEM_CELLS],
    backpack: &[formats::net::account::Item],
    items: &formats::data::ItemList,
) -> Option<Vec<u8>> {
    if index >= 210 || states.get(cell) != Some(&Cell::Have) || entry.need[cell].index == 0 {
        return None;
    }

    let slot = matching_slot(&entry.need[cell], backpack, items)?;

    let mut out = vec![0u8; SIGN_UP_LEN];

    protocol::header::set_opcode(&mut out, SIGN_UP_OPCODE);

    out[SIGN_UP_INDEX_AT..SIGN_UP_INDEX_AT + 4].copy_from_slice(&(index as i32).to_le_bytes());

    out[SIGN_UP_SLOT_AT..SIGN_UP_SLOT_AT + 4].copy_from_slice(&(cell as i32).to_le_bytes());

    let at = SIGN_UP_ITEMS_AT + cell * 12;

    out[at..at + 12].copy_from_slice(&backpack[slot].to_bytes());

    out[SIGN_UP_SLOTS_AT + cell] = slot as u8;

    Some(out)
}

fn material_mut(
    panels: &mut super::panels::Panels,
    owner: ui::draw::Owner,
) -> Option<&mut ui::Element> {
    let ui::draw::Owner::ListRow {
        element,
        row,
        child: Some(child),
    } = owner
    else {
        return None;
    };

    let list = panels
        .screen_mut(CONTENT)?
        .placed
        .ui
        .panel
        .elements
        .get_mut(element)?;

    if !list.name.eq_ignore_ascii_case(LIST_VIEW) {
        return None;
    }

    list.widget
        .list_mut()?
        .items
        .get_mut(row)?
        .children
        .get_mut(child)
}

pub fn row_selected(panels: &mut super::panels::Panels) {
    if let Some(owner) = panels.collection_icon.take()
        && let Some(button) = material_mut(panels, owner).and_then(|e| e.widget.button_mut())
    {
        button.checked = false;
    }
}

pub fn item_clicked(panels: &mut super::panels::Panels, event: &ui::input::Event) {
    let Some(owner) = event.owner else {
        return;
    };

    let Some(child) = material_mut(panels, owner) else {
        return;
    };

    if child.element_type != formats::ui::pane::ElementType::IconButton
        || !child.visible
        || !child.enabled
        || child.name != event.name
    {
        return;
    }

    let Some(cell) = child
        .name
        .strip_prefix("Item_")
        .and_then(|n| n.parse::<usize>().ok())
        .filter(|&cell| cell < 6)
    else {
        return;
    };

    let Some(button) = child.widget.button_mut() else {
        return;
    };

    let row = button.icon_slot_index;

    let enabled = button.collection.is_some_and(|state| state.state == 2);

    button.checked = true;

    row_selected(panels);

    panels.collection_cell = cell;

    panels.collection_icon = Some(owner);

    if let Some(screen) = panels.screen_mut(CONTENT) {
        if let Some(list) = screen
            .element_mut(LIST_VIEW, formats::ui::pane::ElementType::ListView)
            .and_then(|e| e.widget.list_mut())
        {
            list.selected = i32::from(row);
        }

        if let Some(button) = screen.element_mut(SIGN_UP, formats::ui::pane::ElementType::Button) {
            button.enabled = enabled;
        }
    }
}

pub fn opened(scene: &mut FieldScene) {
    if let Some(panels) = scene.panels.as_mut() {
        panels.collection_filter = Filter::All;

        panels.collection_dirty = true;

        panels.collection_cell = 0;

        panels.collection_icon = None;

        panels.collection_signed_up = false;

        check_filter(panels);
    }

    panel_refresh(scene);
}

pub fn check_filter(panels: &mut super::panels::Panels) {
    let filter = panels.collection_filter;

    if let Some(screen) = panels.screen_mut(CONTENT) {
        for (name, active) in [
            (ALL_BUTTON, Filter::All),
            (INCOMPLETE_BUTTON, Filter::Incomplete),
            (COMPLETE_BUTTON, Filter::Complete),
        ] {
            if let Some(button) = screen
                .element_mut(name, formats::ui::pane::ElementType::Button)
                .and_then(|e| e.widget.button_mut())
            {
                button.checked = active == filter;
            }
        }
    }
}

pub fn panel_refresh(scene: &mut FieldScene) -> Option<Filter> {
    let dirty = scene
        .panels
        .as_ref()
        .is_some_and(|panels| panels.collection_dirty);

    if !dirty {
        return None;
    }

    let book = scene.collection.clone();

    let costume_item = costume_item(scene);

    let backpack = scene.entered.carry;

    let panels = scene.panels.as_mut()?;

    panels.collection_dirty = false;

    panels.collection_icon = None;

    let filter = panels.collection_filter;

    let super::panels::Panels {
        screens,
        items,
        icons,
        strings,
        ..
    } = panels;

    for screen in screens.iter_mut().filter(|s| s.content == CONTENT) {
        super::screens::collection_refresh(
            screen,
            &book,
            costume_item,
            filter,
            &backpack,
            items,
            icons,
            strings,
        );
    }

    Some(filter)
}

pub fn panel_signs_up(scene: &mut FieldScene) -> Option<usize> {
    let armed = scene
        .panels
        .as_ref()
        .is_some_and(|panels| panels.collection_signed_up);

    if !armed {
        return None;
    }

    let book = scene.collection.clone();

    let backpack = scene.entered.carry;

    let index = {
        let panels = scene.panels.as_mut()?;

        panels.collection_signed_up = false;

        selected_row(panels)?
    };

    let entry = book.table.get(index)?;

    let panels = scene.panels.as_mut()?;

    let states = cells(entry, book.progress.as_ref(), &backpack, &panels.items);

    let mut frame = sign_up_frame(
        entry,
        index,
        panels.collection_cell,
        &states,
        &backpack,
        &panels.items,
    )?;

    frame[6..8].copy_from_slice(&scene.entered.client_id.to_le_bytes());

    panels.wire.push(frame);

    if let Some(screen) = panels.screen_mut(CONTENT)
        && let Some(button) = screen.element_mut(SIGN_UP, formats::ui::pane::ElementType::Button)
    {
        button.enabled = false;
    }

    if panels.collection_icon.is_some() {
        row_selected(panels);

        if let Some(screen) = panels.screen_mut(CONTENT)
            && let Some(list) = screen
                .element_mut(LIST_VIEW, formats::ui::pane::ElementType::ListView)
                .and_then(|e| e.widget.list_mut())
        {
            list.selected = -1;
        }
    }

    Some(index)
}

fn selected_row(panels: &super::panels::Panels) -> Option<usize> {
    let screen = panels.screen(CONTENT)?;

    let list = screen
        .placed
        .ui
        .panel
        .elements
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(LIST_VIEW))
        .and_then(|e| e.widget.list())?;

    let row = usize::try_from(list.selected).ok()?;

    usize::try_from(list.items.get(row)?.payload).ok()
}
