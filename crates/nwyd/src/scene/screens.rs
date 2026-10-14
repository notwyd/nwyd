use crate::scene::arena;
use crate::scene::enter::Entered;
use crate::scene::panels::{
    self, CONTENT_CARGO, CONTENT_CHARACTER, CONTENT_HELP, CONTENT_INVENTORY, CONTENT_ITEM_SHOP,
    CONTENT_MISSION_MIX, CONTENT_PARTY, CONTENT_QUEST, CONTENT_SKILL, CONTENT_SKILL_MASTER,
    CONTENT_STORE, Panels, Screen,
};
use crate::scene::portal;
use crate::scene::skill_master;
use formats::data::item_icon::IconTable;
use formats::data::item_list::ItemList;
use formats::net::account::Status;
use formats::net::enter;
use formats::ui::pane::ElementType;
use ui::Strings;
use ui::widget::cell_group;

pub const CLASS_NAME_STRDEF: usize = 121;

pub const CLASSES: usize = 4;

pub const MONSTER_BODY: i32 = 40;

pub const EVOLVED_DIGIT: i32 = 6;

pub const CLASS_DECADE: i32 = 10;

pub const SHEET_TAGS: [&str; 2] = ["tag0", "tag1"];

pub const SHEET_TABS: [&str; 2] = ["Tab0", "Tab1"];

pub const HELP_TABS: [&str; 5] = ["Tab0", "Tab1", "Tab2", "Tab3", "Tab4"];

pub const HELP_BUTTON_PAGES: [i32; 5] = [0, 3, 4, 5, 6];

pub const HELP_NOTICE_PAGE: i32 = 6;

pub const HELP_FILES: [&str; 5] = [
    "interface1.txt",
    "interface2.txt",
    "interface3.txt",
    "command.txt",
    "etc.txt",
];

pub const HELP_LINES: usize = 9;

pub const HELP_IMAGES: usize = 3;

pub const HELP_MEMO_ROWS: usize = 102;

pub fn help_memo(rows: &mut Vec<ColorLine>, text: String) {
    if rows.len() >= HELP_MEMO_ROWS {
        rows.remove(0);
    }

    rows.push(ColorLine {
        color: LINE_WHITE,
        text,
    });
}

pub const BAG_TABS: usize = 4;

pub const BAG_CELLS: usize = 15;

pub const INVENTORY_CELLS: usize = BAG_TABS * BAG_CELLS;

pub const EQUIP_CELLS: usize = 17;

pub const EQUIP_FIRST: usize = 1;

pub const BAG_SKINS: [&str; 3] = ["UI_Inven.BagBtnN", "UI_Inven.BagBtnD", "UI_Inven.BagBtnO"];

pub const BAG_LOCKED_SKINS: [&str; 3] = [
    "UI_Inven.LockBagBtnN",
    "UI_Inven.LockBagBtnD",
    "UI_Inven.LockBagBtnO",
];

pub const BAG_EXTENSION_ITEM: u16 = enter::BAG_EXTENSION_ITEM;

pub const BAG_DAY_EFFECT: u8 = b'j';

pub const BAG_MONTH_EFFECT: u8 = b'n';

pub const BAG_DAY_STRDEF: usize = 291;

pub const BAG_MONTH_STRDEF: usize = 296;

pub const CARGO_TABS: usize = 3;

pub const CARGO_PAGE_CELLS: usize = 40;

pub const CARGO_CELLS: usize = CARGO_TABS * CARGO_PAGE_CELLS;

pub const CARGO_HIDDEN_SLOTS: usize = formats::net::account::CARGO_SLOTS - CARGO_CELLS;

pub const SHOP_CELLS: usize = 40;

pub const SHOP_STOCK_CELLS: usize = 27;

pub const SHOP_SELL_CELLS: usize = 10;

pub const SKILL_CELLS: usize = 48;

pub const SKILL_COLUMNS: usize = 6;

pub const SKILL_ROWS: usize = 8;

pub const SKILL_BLOCK: usize = SKILL_COLUMNS * 2;

pub const SKILL_GROUPS: usize = 3;

pub const SKILL_CLASS_BITS: usize = 24;

pub const SKILL_MASTERY_BITS: usize = 12;

pub const SKILL_SEPHIRA_BITS: usize = 8;

pub const SKILL_CLASS_BASE: i32 = 5000;

pub const SKILL_MASTERY_BASE: i32 = 0x1518;

pub const SKILL_SEPHIRA_BASE: i32 = 0x13E8;

pub const SKILL_SEPHIRA_ROTATE: u32 = 24;

pub const fn skill_class_cell(bit: usize) -> usize {
    (bit & 7) + (bit >> 3) * SKILL_BLOCK
}

pub const fn skill_mastery_cell(bit: usize) -> usize {
    (bit & 3) + 8 + (bit >> 2) * SKILL_BLOCK
}

pub const fn skill_sephira_cell(bit: usize) -> usize {
    (bit & 3) + ((bit >> 2) * 3 + 0x12) * 2
}

pub fn skill_cell_name(cell: usize) -> String {
    format!("SkillBtn{cell}")
}

pub const QUEST_TABS: [&str; 4] = ["Quest0", "Quest1", "Quest2", "Quest3"];

pub const QUEST_SUBJECT_FILES: [&str; 4] = [
    "QuestSubjects.txt",
    "QuestSubjects2.txt",
    "QuestSubjects3.txt",
    "QuestSubjects4.txt",
];

pub const QUEST_CONTENT_FILES: [&str; 4] = [
    "QuestContents.txt",
    "QuestContents2.txt",
    "QuestContents3.txt",
    "QuestContents4.txt",
];

pub const QUEST_LINES: usize = 20;

pub const QUEST_COLUMNS: usize = 4;

pub const QUEST_EVOLVED_LEVEL: i32 = 350;

pub const QUEST_WHITE: u32 = 0xFFFF_FFFF;

pub const QUEST_GREY: u32 = 0xFF77_7777;

pub const QUEST_GREEN: u32 = 0xFF00_FF00;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Quest {
    pub class_mask: u8,
    pub level: i32,
    pub flag: (i32, i32),
    pub title: String,
    pub lines: Vec<ColorLine>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColorLine {
    pub color: u32,
    pub text: String,
}

pub const LINE_WHITE: u32 = 0xFFFF_FFFF;

fn color_line(line: &str) -> Option<ColorLine> {
    let trimmed = line.trim_end_matches(['\r', '\n']);

    if trimmed.len() < 6 {
        return None;
    }

    let color = u32::from_str_radix(trimmed.get(..6)?, 16).unwrap_or(0) | 0xFF00_0000;

    let text = match trimmed.len() >= 8 {
        true => trimmed.get(7..)?.to_string(),
        false => String::new(),
    };

    Some(ColorLine { color, text })
}

pub fn thousands(value: u64) -> String {
    let digits = value.to_string();

    let mut out = String::with_capacity(digits.len() + digits.len() / 3);

    for (index, ch) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }

        out.push(ch);
    }

    out
}

pub fn class_names(strings: &Strings, class: usize, body: i32) -> (String, String) {
    let name = |index: usize| match strings.strdef.as_ref() {
        Some(table) => table
            .get(CLASS_NAME_STRDEF + index.min(CLASSES - 1))
            .to_string(),
        None => String::new(),
    };

    if body >= MONSTER_BODY {
        return ("Monster".to_string(), " ".to_string());
    }

    let primary = name(class);

    match body % CLASS_DECADE >= EVOLVED_DIGIT {
        true => (primary, name((body / CLASS_DECADE) as usize)),
        false => (primary, String::new()),
    }
}

pub fn text(screen: &mut Screen, name: &str, value: &str) {
    let element = if screen.content == super::refine::CONTENT {
        screen.refine_element_mut(name, ElementType::Static)
    } else if screen.content == CONTENT_CHARACTER {
        screen.sheet_element_mut(name, ElementType::Static)
    } else {
        screen.element_mut(name, ElementType::Static)
    };

    if let Some(element) = element {
        element.set_display(value);
    }
}

fn show(screen: &mut Screen, name: &str, visible: bool) {
    if screen.content == super::refine::CONTENT {
        if let Some(owner) = screen.refine_widgets.get(name).copied()
            && let Some(e) = screen
                .placed
                .ui
                .panel
                .elements
                .iter_mut()
                .find(|e| e.instance == owner)
        {
            e.visible = visible;
        }

        return;
    }

    if let Some(element) = screen
        .placed
        .ui
        .panel
        .elements
        .iter_mut()
        .find(|e| e.name.eq_ignore_ascii_case(name))
    {
        element.visible = visible;
    }
}

pub fn store_exhibition(screen: &mut Screen, slot: usize, visible: bool) {
    show(screen, &format!("ExpoFrame{slot}"), visible);
}

pub fn sheet_bound(screen: &mut Screen) {
    use ElementType::{Button, Image, Static};

    let resource = screen.placed.ui.resource.clone();

    let mut names = vec![("Tab0".to_owned(), Button), ("Tab1".to_owned(), Button)];

    names.extend(
        [
            "Name", "Level", "Grade", "Class", "Class2", "Exp", "ExpMax", "Str", "Int", "Dex",
            "Con",
        ]
        .into_iter()
        .map(|n| (n.to_owned(), Static)),
    );

    names.extend((0..4).map(|i| (format!("StatBtn{i}"), Button)));

    names.extend(
        [
            "StatPoint",
            "HP",
            "MP",
            "Dam",
            "Def",
            "Move",
            "MagicDam",
            "AttSpd",
            "Cri",
            "Reg0",
            "Reg1",
            "Reg2",
            "Reg3",
        ]
        .into_iter()
        .map(|n| (n.to_owned(), Static)),
    );

    for prefix in ["SpecialName", "Special"] {
        names.extend((0..4).map(|i| (format!("{prefix}{i}"), Static)));
    }

    names.extend((0..4).map(|i| (format!("MasterBtn{i}"), Button)));

    names.extend([
        ("SpecialPoint".into(), Static),
        ("CPPoint".into(), Static),
        ("Flags".into(), Image),
    ]);

    names.extend((0..17).map(|i| (format!("Value{i}"), Static)));

    for (name, kind) in names {
        let owner = screen
            .placed
            .ui
            .panel
            .element_or_create(&name, kind, &resource)
            .instance;

        screen.sheet_widgets.insert(name, owner);
    }
}

pub fn sheet_created(screen: &mut Screen) {
    screen.page = 0;

    sheet_page(screen, 0);
}

pub fn sheet_page(screen: &mut Screen, page: i32) {
    screen.page = page;

    for (index, tag) in SHEET_TAGS.iter().enumerate() {
        screen
            .placed
            .ui
            .panel
            .set_visible_by_tagname(tag, index as i32 == page);
    }

    for (index, tab) in SHEET_TABS.iter().enumerate() {
        if let Some(button) = screen.sheet_element_mut(tab, ElementType::Button)
            && let Some(state) = button.widget.button_mut()
        {
            state.checked = index as i32 == page;
        }
    }

    let flag_visible = screen.cape_flag.is_some();

    if let Some(flags) = screen.sheet_element_mut("Flags", ElementType::Image) {
        flags.visible = page == 0 && flag_visible;
    }
}

pub fn sheet_command(screen: &mut Screen, name: &str) -> bool {
    let Some(page) = SHEET_TABS.iter().position(|t| *t == name) else {
        return false;
    };

    if screen.page != page as i32 {
        sheet_page(screen, page as i32);
    }

    true
}

pub fn sheet_refresh(screen: &mut Screen, strings: &Strings, entered: &Entered) {
    populate_character_summary(screen, strings, entered);

    populate_character_details(
        screen,
        entered,
        entered.status.hp.min(entered.status.max_hp),
    );
}

pub fn sheet_refresh_current(screen: &mut Screen, strings: &Strings, entered: &Entered) {
    if !screen.open {
        return;
    }

    if screen.page == 0 {
        populate_character_summary(screen, strings, entered);
    } else {
        populate_character_details(
            screen,
            entered,
            entered.status.hp.min(entered.status.max_hp),
        );
    }
}

fn populate_character_summary(screen: &mut Screen, strings: &Strings, entered: &Entered) {
    let status: &Status = &entered.status;

    let derived = &entered.derived;

    let class = usize::from(entered.class);

    let body = i32::from(entered.equipment.body().index);

    let hp = status.hp.min(status.max_hp);

    text(screen, "Name", &format!("[{}]", entered.name()));

    text(screen, "Level", &status.shown_level().to_string());

    text(screen, "Grade", &entered.wearer.rank_word.to_string());

    let (primary, secondary) = class_names(strings, class, body);

    text(screen, "Class", &primary);

    text(screen, "Class2", &secondary);

    text(screen, "Exp", &thousands(entered.exp));

    let transcendent = entered.equipment.is_transcendent();

    let span = formats::net::exp::level_span(status.level, transcendent);

    if let Some((_, high)) = span {
        text(screen, "ExpMax", &thousands(high as u64));
    }

    for (name, value) in [
        ("Str", status.stat[0]),
        ("Int", status.stat[1]),
        ("Dex", status.stat[2]),
        ("Con", status.stat[3]),
    ] {
        text(screen, name, &value.to_string());
    }

    text(screen, "HP", &hp.to_string());

    text(screen, "MP", &status.mp.to_string());

    text(screen, "Dam", &status.attack_damage.to_string());

    text(screen, "Def", &status.defense.to_string());

    let speed = super::enter::move_speed(status) as i32;

    text(screen, "Move", &speed.to_string());

    text(
        screen,
        "AttSpd",
        &format!("{}%", attack_speed(derived.attack_speed) / 10),
    );

    text(
        screen,
        "Cri",
        &format!("{}%", critical(derived.critical) / 10),
    );

    for (index, value) in derived.regen.iter().enumerate() {
        text(screen, &format!("Reg{index}"), &value.to_string());
    }

    text(screen, "StatPoint", &derived.stat_points.to_string());

    text(screen, "SpecialPoint", &derived.mastery_points.to_string());

    let caps = mastery_caps(
        status.level,
        entered.class,
        entered.equipment.evolved(),
        entered.skill_mask,
    );

    for (index, value) in status.mastery.iter().enumerate() {
        text(
            screen,
            &format!("Special{index}"),
            &format!("{value:3}/{:3}", caps[index]),
        );

        let record = MASTERY_NAME_STRDEF + class.min(CLASSES - 1) * MASTERIES + index;

        let name = strings
            .strdef
            .as_ref()
            .map_or(String::new(), |t| t.get(record).to_string());

        text(screen, &format!("SpecialName{index}"), &name);
    }

    text(screen, "CPPoint", &cp_line(entered, span));
}

pub fn sheet_magic(panels: &mut Panels, display: super::magic_damage::Display) {
    let Some(screen) = panels
        .screens
        .iter_mut()
        .find(|screen| screen.content == CONTENT_CHARACTER)
    else {
        return;
    };

    for name in ["MagicDam", "Value3"] {
        match display {
            super::magic_damage::Display::Invalid => text(screen, name, "0"),
            super::magic_damage::Display::Value { shown, colour } => {
                text(screen, name, &shown.to_string());

                if let Some(element) = screen.sheet_element_mut(name, ElementType::Static) {
                    set_font_color(element, colour);
                }
            }
        }
    }
}

pub fn sheet_flag(
    panels: &mut Panels,
    resolved: Option<(super::cape_flag::Flag, Option<[u32; 2]>)>,
) {
    let Some(screen) = panels.screen_mut(CONTENT_CHARACTER) else {
        return;
    };

    let owner = screen
        .placed
        .ui
        .panel
        .elements
        .iter()
        .position(|element| screen.sheet_widgets.get("Flags") == Some(&element.instance))
        .map(ui::draw::Owner::Element);

    let state = resolved
        .as_ref()
        .map(|(flag, size)| match (owner, flag.texture_slot, *size) {
            (Some(owner), Some(texture_slot), Some(size)) => {
                super::cape_flag::WidgetState::External(super::cape_flag::ExternalImage {
                    owner,
                    texture_slot,
                    size,
                })
            }
            _ => super::cape_flag::WidgetState::Layer,
        });

    screen.cape_flag = state;

    let page = screen.page;

    let Some(element) = screen.sheet_element_mut("Flags", ElementType::Image) else {
        return;
    };

    if let Some((flag, _)) = resolved {
        element.set_tooltip_text(&flag.tooltip);
    }

    element.visible = state.is_some() && page == 0;
}

fn populate_character_details(screen: &mut Screen, entered: &Entered, hp: u32) {
    let status = &entered.status;

    let derived = &entered.derived;

    let speed = super::enter::move_speed(status) as i32;

    let tenths = |value: i32| format!("{}.{}%", value / 10, (value % 10).abs());

    let cells: [(usize, String); 16] = [
        (0, format!("{hp} / {}", status.max_hp)),
        (1, speed.to_string()),
        (2, format!("{} / {}", status.mp, status.max_mp)),
        (4, status.attack_damage.to_string()),
        (5, tenths(attack_speed(derived.attack_speed))),
        (6, status.defense.to_string()),
        (7, tenths(critical(derived.critical))),
        (8, format!("{}%", derived.value8)),
        (9, format!("{}%", derived.value9)),
        (10, tenths(i32::from(derived.words[2] as i16))),
        (11, tenths(i32::from(derived.words[3] as i16))),
        (12, (derived.words[0] as i16).to_string()),
        (13, (derived.words[1] as i16).to_string()),
        (14, tenths(i32::from(derived.words[4] as i16))),
        (15, tenths(i32::from(derived.words[5] as i16))),
        (16, format!("{}%", derived.mana_saving)),
    ];

    for (index, value) in cells {
        text(screen, &format!("Value{index}"), &value);
    }
}

pub fn attack_speed(stored: i16) -> i32 {
    i32::from(stored) + ATTACK_SPEED_BASE
}

pub const ATTACK_SPEED_BASE: i32 = 1000;

pub fn critical(stored: u8) -> i32 {
    i32::from(stored) * CRITICAL_SCALE
}

pub const CRITICAL_SCALE: i32 = 4;

pub const MASTERY_NAME_STRDEF: usize = 242;

pub const MASTERIES: usize = 4;

pub fn mastery_caps(
    level: u16,
    class: u8,
    evolved: bool,
    skill_mask: [u32; 2],
) -> [i32; MASTERIES] {
    let tier = if evolved {
        1 + u8::from(skill_mask[0] & formats::net::account::EVOLVED_BIT != 0)
    } else {
        0
    };

    let formula = (i32::from(level as i16) * 3 + 3) / 2;

    let base = if tier == 2 {
        MASTERY_CAP_MAX
    } else {
        formula.min(MASTERY_CAP_MAX)
    };

    let mut caps = [base; MASTERIES];

    if class == 0 && skill_mask[1] & (1 << 5) != 0 {
        caps[0] = 0x118;
    } else if class == 2 && skill_mask[1] & (1 << 9) != 0 {
        caps[0] = 0xE6;
    }

    caps[1] = if skill_mask[1] & (1 << 0) != 0 {
        0x140
    } else if skill_mask[0] & (1 << 7) != 0 {
        0xFF
    } else {
        caps[1]
    };

    caps[2] = if skill_mask[1] & (1 << 4) != 0 {
        0x140
    } else if skill_mask[0] & (1 << 15) != 0 {
        0xFF
    } else {
        caps[2]
    };

    caps[3] = if skill_mask[1] & (1 << 8) != 0 {
        0x140
    } else if skill_mask[0] & (1 << 23) != 0 {
        0xFF
    } else {
        caps[3]
    };

    if class == 3 && skill_mask[1] & (1 << 2) != 0 {
        caps[1] = 400;
    }

    caps
}

pub const MASTERY_CAP_MAX: i32 = 200;

fn cp_line(entered: &Entered, span: Option<(i64, i64)>) -> String {
    let percent = match span {
        Some((low, high)) if high > low => {
            (entered.exp.saturating_sub(low as u64) * 10 / (high - low) as u64) as i64
        }
        _ => 0,
    };

    format!("{} ({percent})", entered.cp)
}

pub fn inventory_created(screen: &mut Screen) {
    screen.inventory_widgets = InventoryWidgets {
        cells: std::array::from_fn(|i| {
            screen
                .element(&format!("Inven{i}"), ElementType::IconButton)
                .map(|e| e.instance)
        }),
        tabs: std::array::from_fn(|i| {
            screen
                .element(&format!("TabItem{i}"), ElementType::Button)
                .map(|e| e.instance)
        }),
        days: std::array::from_fn(|i| {
            screen
                .element(&format!("Day{i}"), ElementType::Static)
                .map(|e| e.instance)
        }),
        equipment: std::array::from_fn(|i| {
            screen
                .element(&format!("Equip{}", i + 1), ElementType::IconButton)
                .map(|e| e.instance)
        }),
        money: screen
            .element("Money", ElementType::Static)
            .map(|e| e.instance),
        cached_page: None,
    };

    for (prefix, range, group) in [
        (
            "Equip",
            EQUIP_FIRST..EQUIP_FIRST + EQUIP_CELLS,
            cell_group::EQUIPMENT,
        ),
        ("Inven", 0..INVENTORY_CELLS, cell_group::INVENTORY),
    ] {
        for cell in range {
            let Some(element) =
                screen.element_mut(&format!("{prefix}{cell}"), ElementType::IconButton)
            else {
                continue;
            };

            let Some(state) = element.set_icon_contents(None) else {
                continue;
            };

            state.pulse_fade = None;

            state.icon_slot_type = 2;

            state.icon_slot_index = cell as i16;

            state.cell_group = group;
        }
    }

    inventory_page(screen, 0);
}

pub fn inventory_page(screen: &mut Screen, tab: i32) {
    screen.page = tab;

    inventory_page_visibility(screen);

    for (index, bound) in screen.inventory_widgets.tabs.into_iter().enumerate() {
        if let Some(bound) = bound
            && let Some(button) = screen
                .placed
                .ui
                .panel
                .elements
                .iter_mut()
                .find(|e| e.instance == bound)
            && let Some(state) = button.widget.button_mut()
        {
            state.checked = index as i32 == tab;
        }
    }
}

pub fn inventory_command(screen: &mut Screen, name: &str) -> bool {
    let Some(tab) = (0..BAG_TABS).find(|i| format!("TabItem{i}") == name) else {
        return false;
    };

    inventory_page(screen, tab as i32);

    true
}

pub fn inventory_refresh(
    screen: &mut Screen,
    entered: &Entered,
    items: &ItemList,
    icons: &IconTable,
    strings: &Strings,
) {
    if let Some(bound) = screen.inventory_widgets.money
        && let Some(element) = screen
            .placed
            .ui
            .panel
            .elements
            .iter_mut()
            .find(|e| e.instance == bound)
    {
        element.set_display(&thousands(u64::from(entered.gold)));
    }

    inventory_bag_refresh(screen, entered, items, icons, strings);

    let resource = screen.placed.ui.resource.clone();

    for cell in 0..EQUIP_CELLS {
        let slot = EQUIP_FIRST + cell;

        let item = entered
            .equipment
            .items
            .get(slot)
            .copied()
            .unwrap_or_default();

        let shown = match (item.index as i16) > 0 && i32::from(item.index as i16) <= 0x38A3 {
            true => item,
            false => Default::default(),
        };

        if let Some(bound) = screen.inventory_widgets.equipment[cell] {
            let contents = ui::icon::set_icon(&shown, items, icons, &resource);

            if let Some(element) = screen
                .placed
                .ui
                .panel
                .elements
                .iter_mut()
                .find(|e| e.instance == bound)
                && let Some(state) = element.set_icon_contents(contents)
            {
                state.pulse_fade = None;
            }
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct InventoryWidgets {
    cells: [Option<ui::element::Instance>; 60],
    tabs: [Option<ui::element::Instance>; 4],
    days: [Option<ui::element::Instance>; 2],
    equipment: [Option<ui::element::Instance>; 17],
    money: Option<ui::element::Instance>,
    cached_page: Option<i32>,
}

impl Default for InventoryWidgets {
    fn default() -> Self {
        Self {
            cells: [None; 60],
            tabs: [None; 4],
            days: [None; 2],
            equipment: [None; 17],
            money: None,
            cached_page: None,
        }
    }
}

pub(crate) fn inventory_destination(
    screen: &Screen,
    instance: ui::element::Instance,
) -> Option<super::item_move::Coord> {
    if let Some(slot) = screen
        .inventory_widgets
        .equipment
        .iter()
        .position(|b| *b == Some(instance))
    {
        Some(super::item_move::Coord {
            container: 0,
            slot: slot as i32 + 1,
        })
    } else {
        screen
            .inventory_widgets
            .cells
            .iter()
            .position(|b| *b == Some(instance))
            .map(|slot| super::item_move::Coord {
                container: 1,
                slot: slot as i32,
            })
    }
}

pub(crate) fn inventory_source(
    screen: &Screen,
    at: super::item_move::Coord,
) -> Option<ui::element::Instance> {
    let slot = usize::try_from(at.slot).ok()?;

    match at.container {
        0 => screen
            .inventory_widgets
            .equipment
            .get(slot.checked_sub(1)?)
            .copied()
            .flatten(),
        1 => screen.inventory_widgets.cells.get(slot).copied().flatten(),
        _ => None,
    }
}

fn inventory_page_visibility(screen: &mut Screen) {
    if screen.inventory_widgets.cached_page == Some(screen.page) {
        return;
    }

    for (cell, bound) in screen.inventory_widgets.cells.into_iter().enumerate() {
        if let Some(bound) = bound
            && let Some(element) = screen
                .placed
                .ui
                .panel
                .elements
                .iter_mut()
                .find(|e| e.instance == bound)
        {
            element.visible = cell as i32 / BAG_CELLS as i32 == screen.page;
        }
    }

    screen.inventory_widgets.cached_page = Some(screen.page);
}

pub fn inventory_bag_refresh(
    screen: &mut Screen,
    entered: &Entered,
    items: &ItemList,
    icons: &IconTable,
    strings: &Strings,
) {
    inventory_page_visibility(screen);

    let resource = screen.placed.ui.resource.clone();

    for (cell, bound) in screen.inventory_widgets.cells.into_iter().enumerate() {
        if let Some(bound) = bound {
            let item = entered.carry.get(cell).copied().unwrap_or_default();

            let contents = ui::icon::set_icon(&item, items, icons, &resource);

            if let Some(element) = screen
                .placed
                .ui
                .panel
                .elements
                .iter_mut()
                .find(|e| e.instance == bound)
                && let Some(state) = element.set_icon_contents(contents)
            {
                state.pulse_fade = None;
            }
        }
    }

    for bag in 0..2 {
        let extension = entered.bag_extension(bag);

        let label = extension
            .map(|item| bag_expiry(&item, strings))
            .unwrap_or_default();

        if let Some(bound) = screen.inventory_widgets.days[bag]
            && let Some(element) = screen
                .placed
                .ui
                .panel
                .elements
                .iter_mut()
                .find(|e| e.instance == bound)
        {
            element.set_display(&label);
        }

        let skins = match extension {
            Some(_) => BAG_SKINS,
            None => BAG_LOCKED_SKINS,
        };

        if let Some(bound) = screen.inventory_widgets.tabs[bag + 2]
            && let Some(button) = screen
                .placed
                .ui
                .panel
                .elements
                .iter_mut()
                .find(|e| e.instance == bound)
        {
            for (layer, alias) in skins.iter().enumerate() {
                button.set_layer_texture_alias(layer, alias, &resource);
            }
        }
    }
}

pub fn cargo_created(screen: &mut Screen) {
    let resource = screen.placed.ui.resource.clone();

    let tabs = std::array::from_fn(|i| {
        Some(
            screen
                .placed
                .ui
                .panel
                .element_or_create(&format!("TabItem{i}"), ElementType::Button, &resource)
                .instance,
        )
    });

    let money = Some(
        screen
            .placed
            .ui
            .panel
            .element_or_create("Money", ElementType::Static, &resource)
            .instance,
    );

    let cells = std::array::from_fn(|i| {
        Some(
            screen
                .placed
                .ui
                .panel
                .element_or_create(&format!("Inven{i}"), ElementType::IconButton, &resource)
                .instance,
        )
    });

    screen.cargo_widgets = CargoWidgets { cells, tabs, money };

    for cell in 0..CARGO_CELLS {
        let Some(element) = cargo_element_mut(screen, screen.cargo_widgets.cells[cell]) else {
            continue;
        };

        let Some(state) = element.set_icon_contents(None) else {
            continue;
        };

        state.pulse_fade = None;

        state.icon_slot_type = 2;

        state.icon_slot_index = cell as i16;

        state.cell_group = cell_group::CARGO;
    }

    screen.page = 0;

    if let Some(button) =
        cargo_element_mut(screen, screen.cargo_widgets.tabs[0]).and_then(|e| e.widget.button_mut())
    {
        button.checked = true;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct CargoWidgets {
    cells: [Option<ui::element::Instance>; 120],
    tabs: [Option<ui::element::Instance>; 3],
    money: Option<ui::element::Instance>,
}

impl Default for CargoWidgets {
    fn default() -> Self {
        Self {
            cells: [None; 120],
            tabs: [None; 3],
            money: None,
        }
    }
}

fn cargo_element_mut(
    screen: &mut Screen,
    owner: Option<ui::element::Instance>,
) -> Option<&mut ui::Element> {
    let owner = owner?;

    screen
        .placed
        .ui
        .panel
        .elements
        .iter_mut()
        .find(|e| e.instance == owner)
}

pub(crate) fn cargo_destination(screen: &Screen, owner: ui::element::Instance) -> Option<usize> {
    screen
        .cargo_widgets
        .cells
        .iter()
        .position(|p| *p == Some(owner))
}

pub fn cargo_page(screen: &mut Screen, tab: i32) {
    screen.page = tab;

    for cell in 0..CARGO_CELLS {
        let visible = cell / CARGO_PAGE_CELLS == tab as usize;

        if let Some(element) = cargo_element_mut(screen, screen.cargo_widgets.cells[cell]) {
            element.visible = visible;
        }
    }

    for index in 0..CARGO_TABS {
        if let Some(button) = cargo_element_mut(screen, screen.cargo_widgets.tabs[index])
            && let Some(state) = button.widget.button_mut()
        {
            state.checked = index as i32 == tab;
        }
    }
}

pub fn cargo_command(screen: &mut Screen, name: &str) -> bool {
    let Some(tab) = (0..CARGO_TABS).find(|i| format!("TabItem{i}") == name) else {
        return false;
    };

    screen.page = tab as i32;

    true
}

pub fn cargo_refresh(screen: &mut Screen, entered: &Entered, items: &ItemList, icons: &IconTable) {
    if let Some(element) = cargo_element_mut(screen, screen.cargo_widgets.money) {
        element.set_display(&thousands(u64::from(entered.warehouse.gold)));
    }

    cargo_items_refresh(screen, entered, items, icons);
}

pub fn cargo_items_refresh(
    screen: &mut Screen,
    entered: &Entered,
    items: &ItemList,
    icons: &IconTable,
) {
    cargo_page(screen, screen.page);

    let resource = screen.placed.ui.resource.clone();

    for cell in 0..CARGO_CELLS {
        let item = entered
            .warehouse
            .cargo
            .get(cell)
            .copied()
            .unwrap_or_default();

        if let Some(element) = cargo_element_mut(screen, screen.cargo_widgets.cells[cell]) {
            let contents = ui::icon::set_icon(&item, items, icons, &resource);

            if let Some(button) = element.set_icon_contents(contents) {
                button.pulse_fade = None;

                if contents.is_none() && (1..=0x38A3).contains(&item.index) {
                    button.set_tooltip_binding(Some(ui::icon::TooltipBinding {
                        kind: 1,
                        index: i32::from(item.index),
                        item,
                        help_id: 0,
                    }));
                }
            }
        }
    }
}

pub fn shop_created(screen: &mut Screen) {
    screen.page = 0;

    let resource = screen.placed.ui.resource.clone();

    for cell in 0..SHOP_CELLS {
        let element = screen.placed.ui.panel.element_or_create(
            &format!("Inven{cell}"),
            ElementType::IconButton,
            &resource,
        );

        element.tooltip.clear();

        element.tooltip_key.clear();

        let Some(state) = element.set_icon_contents(None) else {
            continue;
        };

        state.collection = None;

        state.pulse_fade = None;

        state.icon_slot_type = 2;

        state.icon_slot_index = cell as i16;

        state.cell_group = cell_group::SHOP;
    }
}

pub fn shop_refresh(
    screen: &mut Screen,
    stock: &[formats::net::account::Item],
    items: &ItemList,
    icons: &IconTable,
) {
    screen.page = 0;

    let resource = screen.placed.ui.resource.clone();

    for cell in 0..SHOP_STOCK_CELLS {
        let item = stock.get(cell).copied().unwrap_or_default();

        let shown = match (item.index as i16) > 0 && i32::from(item.index as i16) <= 0x38A3 {
            true => item,
            false => Default::default(),
        };

        set_cell_icon(
            screen,
            &format!("Inven{cell}"),
            &shown,
            items,
            icons,
            &resource,
        );
    }

    shop_page_picture(screen, 8);
}

fn shop_page_picture(screen: &mut Screen, slot: i32) {
    if let Some(element) = screen.element_mut("Inven39", ElementType::IconButton) {
        element.set_icon_contents(ui::icon::set_picture_icon(slot));
    }
}

pub fn shop_clear_stock(screen: &mut Screen) {
    for cell in 0..27 {
        if let Some(e) = screen.element_mut(&format!("Inven{cell}"), ElementType::IconButton) {
            e.tooltip.clear();

            e.tooltip_key.clear();

            if let Some(b) = e.set_icon_contents(None) {
                b.collection = None;

                b.pulse_fade = None;
            }
        }
    }
}

pub fn shop_sell_page(
    screen: &mut Screen,
    rows: &[formats::net::shop::SellRow],
    items: &ItemList,
    icons: &IconTable,
) -> usize {
    let resource = screen.placed.ui.resource.clone();

    if screen.page == 0 {
        for cell in 0..SHOP_STOCK_CELLS {
            set_cell_icon(
                screen,
                &format!("Inven{cell}"),
                &Default::default(),
                items,
                icons,
                &resource,
            );
        }
    }

    screen.page = 1;

    let mut drawn = 0;

    for cell in 0..SHOP_SELL_CELLS {
        let row = rows.get(cell).copied().unwrap_or_default();

        let shown = match row.drawn() {
            true => {
                drawn += 1;

                row.item
            }
            false => Default::default(),
        };

        set_cell_icon(
            screen,
            &format!("Inven{cell}"),
            &shown,
            items,
            icons,
            &resource,
        );
    }

    shop_page_picture(screen, 9);

    drawn
}

pub fn arena_refresh(panels: &mut Panels, fill: &arena::PanelFill) -> bool {
    if let Some(screen) = panels.screen_mut(panels::CONTENT_ARENA) {
        if let Some(button) = screen.element_mut("RoyalArena", ElementType::Button) {
            button.display = fill.name.clone();

            button.enabled = fill.enabled;
        }

        if let Some(state) = screen.element_mut("RoyalArenaState", ElementType::Static) {
            state.display = fill.state.clone();
        }
    }

    match panels.is_open(panels::CONTENT_ARENA) {
        true => false,
        false => panels.set_open(panels::CONTENT_ARENA, true).is_some(),
    }
}

pub fn party_refresh(screen: &mut Screen, party: &super::party::State) {
    party_refresh_in_arena(screen, party, false);
}

pub fn party_refresh_in_arena(
    screen: &mut Screen,
    party: &super::party::State,
    arena_visible: bool,
) {
    clear_list(screen, "List");

    party_bounds(screen, party.rows.len());

    let Some(list) = screen.element_mut("List", ElementType::ListView) else {
        return;
    };

    let Some(state) = list.widget.list_mut() else {
        return;
    };

    for (at, row) in party.rows.iter().enumerate() {
        let visual = party
            .row_visual(at)
            .expect("the row used to obtain this index");

        let index = state.add_item("");

        let vitals = party.row_vitals(at).expect("retained roster row");

        let max = vitals.max_hp.max(1);

        let hp = vitals.hp.clamp(0, max);

        for child in &mut state.items[index].children {
            match child.name.as_str() {
                "Name" => {
                    child.set_display(&row.name);

                    set_font_color(child, visual.name_color);
                }
                "Lv" => child.set_display(&if arena_visible {
                    "-".into()
                } else {
                    format!("{}", i32::from(vitals.level) + 1)
                }),
                "HP" => {
                    if let Some(gauge) = child.widget.gauge_mut() {
                        gauge.range = [0, max];

                        gauge.position = hp;
                    }
                }
                name if name.starts_with("Affect") => child.visible = false,
                _ => {}
            }
        }
    }
}

fn party_bounds(screen: &mut Screen, members: usize) {
    let Some(list) = screen.element_mut("List", ElementType::ListView) else {
        return;
    };

    let Some(state) = list.widget.list() else {
        return;
    };

    let height = state.item_size[1]
        .saturating_mul(i32::try_from(members).unwrap_or(i32::MAX))
        .saturating_add(1);

    list.visible = members != 0;

    if members != 0 {
        list.height = height;

        list.refresh_list_view_layout();
    }

    let panel_height = list.y.saturating_add(height);

    let panel = &mut screen.placed.ui.panel;

    panel.bottom = panel.top.saturating_add(panel_height);
}

pub fn party_controls(screen: &mut Screen, members: usize, auto_join: bool) {
    if let Some(element) = screen.element_mut("AutoJoin", ElementType::CheckBox)
        && let Some(state) = element.widget.button_mut()
    {
        state.checked = auto_join;
    }

    for element in &mut screen.placed.ui.panel.elements {
        if element.tagname == "Auto" {
            element.visible = members == 0;
        }
    }

    show(screen, "Exit", members != 0);
}

pub fn party_live_refresh(
    screen: &mut Screen,
    party: &mut super::party::State,
    actors: &super::actors::ActorScene,
    arena_visible: bool,
) {
    if !screen.open || !screen.placed.ui.panel.visible {
        return;
    }

    let Some(list) = screen.element_mut("List", ElementType::ListView) else {
        return;
    };

    if !list.visible {
        return;
    }

    let Some(state) = list.widget.list_mut() else {
        return;
    };

    for (at, listed) in state.items.iter_mut().enumerate().take(party.rows.len()) {
        let live = super::party::live_row(actors, party.rows[at].id);

        let cached = party.row_vitals_mut(at).expect("retained roster row");

        if let Some(live) = live {
            for child in &mut listed.children {
                match child.name.as_str() {
                    "Lv" => {
                        if cached.level != live.level {
                            cached.level = live.level;

                            child.set_display(&format!("{}", i32::from(live.level) + 1));
                        }

                        if arena_visible {
                            child.set_display("-");
                        }
                    }
                    "HP" => {
                        if let Some(gauge) = child.widget.gauge_mut()
                            && (cached.hp != live.hp || cached.max_hp != live.max_hp)
                        {
                            cached.hp = live.hp;

                            cached.max_hp = live.max_hp;

                            gauge.set_range_and_pos(0, live.max_hp.max(1), live.hp);
                        }
                    }
                    _ => {}
                }
            }
        }

        let affects = live
            .map(|live| live.affects)
            .unwrap_or([None; super::party::AFFECT_CELLS]);

        for cell in 0..super::party::AFFECT_CELLS {
            let name = format!("Affect{cell}");

            let Some(child) = listed
                .children
                .iter_mut()
                .find(|child| child.name == name && child.element_type == ElementType::IconButton)
            else {
                continue;
            };

            child.visible = affects[cell].is_some();

            {
                child.set_icon_contents(
                    affects[cell].and_then(|affect| ui::icon::set_affect_icon(affect.kind, -1, 0)),
                );
            }
        }
    }
}

pub fn skill_master_created(screen: &mut Screen) {
    for cell in 0..skill_master::BUTTONS {
        let name = skill_master::button_name(cell);

        let Some(element) = screen.element_mut(&name, ElementType::IconButton) else {
            continue;
        };

        let Some(state) = element.set_icon_contents(None) else {
            continue;
        };

        state.cell_group = cell_group::SKILL;
    }
}

pub fn clear_cell(
    screen: &mut Screen,
    name: &str,
    items: &ItemList,
    icons: &IconTable,
    resource: &ui::Resource,
) {
    set_cell_icon(screen, name, &Default::default(), items, icons, resource);
}

pub(crate) fn set_cell_icon(
    screen: &mut Screen,
    name: &str,
    item: &formats::net::account::Item,
    items: &ItemList,
    icons: &IconTable,
    resource: &ui::Resource,
) {
    let contents = ui::icon::set_icon(item, items, icons, resource);

    let element = if screen.content == super::refine::CONTENT {
        screen.refine_element_mut(name, ElementType::IconButton)
    } else {
        screen.element_mut(name, ElementType::IconButton)
    };

    if let Some(element) = element
        && let Some(state) = element.set_icon_contents(contents)
    {
        state.pulse_fade = None;
    }
}

fn bag_expiry(item: &formats::net::account::Item, strings: &Strings) -> String {
    let of = |id: u8| {
        item.effects
            .iter()
            .rev()
            .find(|e| e.id == id)
            .map_or(0, |e| u32::from(e.value))
    };

    let (days, months) = (of(BAG_DAY_EFFECT), of(BAG_MONTH_EFFECT));

    if days == 0 {
        return String::new();
    }

    let record = |n: usize| match strings.strdef.as_ref() {
        Some(table) => table.get(n).to_string(),
        None => String::new(),
    };

    let count = |n: u32, record: String| match n {
        0 => String::new(),
        n => record.replacen("%d", &n.to_string(), 1),
    };

    format!(
        "{}{}",
        count(months, record(BAG_MONTH_STRDEF)),
        count(days, record(BAG_DAY_STRDEF))
    )
}

pub fn skill_created(screen: &mut Screen) {
    for cell in 0..SKILL_CELLS {
        let name = skill_cell_name(cell);

        let Some(element) = screen.element_mut(&name, ElementType::IconButton) else {
            continue;
        };

        let Some(state) = element.set_icon_contents(None) else {
            continue;
        };

        state.cell_group = cell_group::SKILL;
    }
}

pub fn skill_refresh(screen: &mut Screen, entered: &Entered, items: &ItemList, strings: &Strings) {
    if !screen.open {
        return;
    }

    text(
        screen,
        "SkillPoint",
        &entered.derived.skill_points.to_string(),
    );

    let class = usize::from(entered.class);

    let labels = skill_master::GROUP_STRDEF
        .get(class)
        .unwrap_or(&skill_master::GROUP_STRDEF[0]);

    for (group, record) in labels.iter().enumerate() {
        let text_value = strings.translate(&format!("ss_{record}"));

        text(screen, &format!("SkillName{}", group + 1), &text_value);
    }

    let class = class as i32;

    for bit in 0..SKILL_CLASS_BITS {
        let learned = entered.skill_mask[0] & (1u32 << bit) != 0;

        let id = SKILL_CLASS_BASE + bit as i32 + class * super::enter::SKILLS_PER_CLASS;

        set_skill_cell(screen, skill_class_cell(bit), learned.then_some(id), items);
    }

    for bit in 0..SKILL_MASTERY_BITS {
        let learned = entered.skill_mask[1] & (1u32 << bit) != 0;

        let id = SKILL_MASTERY_BASE + class * SKILL_MASTERY_BITS as i32 + bit as i32;

        set_skill_cell(
            screen,
            skill_mastery_cell(bit),
            learned.then_some(id),
            items,
        );
    }

    for bit in 0..SKILL_SEPHIRA_BITS {
        let mask = (1u32 << bit).rotate_left(SKILL_SEPHIRA_ROTATE);

        let learned = entered.skill_mask[0] & mask != 0;

        let id = SKILL_SEPHIRA_BASE + bit as i32;

        set_skill_cell(
            screen,
            skill_sephira_cell(bit),
            learned.then_some(id),
            items,
        );
    }
}

fn set_skill_cell(screen: &mut Screen, cell: usize, id: Option<i32>, items: &ItemList) {
    let contents = id.and_then(|id| ui::icon::set_skill_icon(id, items));

    if let Some(element) = screen.element_mut(&skill_cell_name(cell), ElementType::IconButton) {
        element.set_icon_contents(contents);
    }
}

pub fn help_created(screen: &mut Screen) {
    for name in HELP_TABS {
        if let Some(state) = screen
            .element_mut(name, ElementType::Button)
            .and_then(|element| element.widget.button_mut())
        {
            state.toggle = true;
        }
    }

    if let Some(state) = screen
        .element_mut("List", ElementType::ListView)
        .and_then(|element| element.widget.list_mut())
    {
        state.ignore_selection = true;
    }
}

pub fn help_page(screen: &mut Screen, page: i32, lines: &[ColorLine]) {
    screen.page = page;

    let statics = (0..HELP_IMAGES as i32).contains(&page);

    let list_page = (3..=super::minipopup::MEMO_HELP_TAB).contains(&page);

    show(screen, "List", list_page);

    for name in ["Next", "Back"] {
        show(screen, name, statics);
    }

    for image in 0..HELP_IMAGES {
        show(
            screen,
            &format!("Img{image}"),
            statics && page == image as i32,
        );
    }

    for line in 0..HELP_LINES {
        let name = format!("Text{line}");

        show(screen, &name, statics);

        let value = match statics {
            true => lines.get(line).cloned().unwrap_or_default(),
            false => ColorLine::default(),
        };

        text(screen, &name, &value.text);

        if statics && let Some(element) = screen.element_mut(&name, ElementType::Static) {
            set_font_color(element, value.color);
        }
    }

    clear_list(screen, "List");

    if !list_page {
        return;
    }

    let rows = lines.to_vec();

    let Some(list) = screen.element_mut("List", ElementType::ListView) else {
        return;
    };

    let rect = list.rect();

    let Some(state) = list.widget.list_mut() else {
        return;
    };

    for row in rows {
        let index = state.add_item("");

        for child in &mut state.items[index].children {
            match child.name.as_str() {
                "Msg" => {
                    child.set_display(&row.text);

                    set_font_color(child, row.color);
                }
                _ => child.visible = false,
            }
        }
    }

    if page == super::minipopup::MEMO_HELP_TAB {
        state.scroll = 0xFF_FFFF;

        state.scroll = state.clamped_scroll(state.content_rect(rect));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpCommand {
    Refilled,
    Notice,
}

pub fn help_command(
    screen: &mut Screen,
    name: &str,
    files: &[Vec<ColorLine>],
    memo: &[ColorLine],
) -> Option<HelpCommand> {
    let page = match name {
        "Next" => match screen.page < 2 {
            true => screen.page + 1,
            false => screen.page,
        },
        "Back" => (screen.page - 1).max(0),
        name => match HELP_TABS.iter().position(|t| *t == name) {
            Some(tab) => HELP_BUTTON_PAGES[tab],
            None => return None,
        },
    };

    if name == "Tab4" {
        screen.page = page;

        return Some(HelpCommand::Notice);
    }

    let lines = match page {
        super::minipopup::MEMO_HELP_TAB => memo,
        _ => usize::try_from(page)
            .ok()
            .and_then(|page| files.get(page))
            .map_or(&[][..], Vec::as_slice),
    };

    help_page(screen, page, lines);

    Some(HelpCommand::Refilled)
}

pub fn help_lines(bytes: &[u8]) -> Vec<ColorLine> {
    help_lines_with_codepage(bytes, formats::text::Codepage::Windows1252)
}

fn help_lines_with_codepage(bytes: &[u8], cp: formats::text::Codepage) -> Vec<ColorLine> {
    let text = formats::text::codepage::decode(bytes, cp);

    text.lines().filter_map(color_line).collect()
}

pub fn quest_subjects(bytes: &[u8]) -> Vec<Quest> {
    quest_subjects_with_codepage(bytes, formats::text::Codepage::Windows1252)
}

fn quest_subjects_with_codepage(bytes: &[u8], cp: formats::text::Codepage) -> Vec<Quest> {
    let text = formats::text::codepage::decode(bytes, cp);

    let digit = |value: Option<char>| match value {
        Some(c) if c.is_ascii_digit() => c as i32 - '0' as i32,
        Some(c) if c.is_ascii_uppercase() => c as i32 - 0x37,
        _ => 0,
    };

    let masks = formats::net::quest::class_masks(bytes);

    text.lines()
        .filter_map(|line| {
            let columns: Vec<&str> = line.trim_end_matches(['\r', '\n']).split('\t').collect();

            if columns.len() < QUEST_COLUMNS {
                return None;
            }

            let mut flags = columns[2].chars();

            Some(Quest {
                class_mask: 0,
                level: parse_leading_int(columns[0]),
                flag: (digit(flags.next()), digit(flags.next())),
                title: columns[3].to_string(),
                lines: vec![ColorLine::default(); QUEST_LINES],
            })
        })
        .enumerate()
        .map(|(row, mut quest)| {
            quest.class_mask = masks.get(row).copied().unwrap_or(0);

            quest
        })
        .collect()
}

pub fn quest_contents(bytes: &[u8], quests: &mut [Quest]) {
    quest_contents_with_codepage(bytes, quests, formats::text::Codepage::Windows1252);
}

fn quest_contents_with_codepage(bytes: &[u8], quests: &mut [Quest], cp: formats::text::Codepage) {
    let text = formats::text::codepage::decode(bytes, cp);

    let mut index = 0usize;

    for line in text.lines() {
        let Some(line) = color_line(line) else {
            continue;
        };

        let Some(quest) = quests.get_mut(index / QUEST_LINES) else {
            break;
        };

        quest.lines[index % QUEST_LINES] = line;

        index += 1;
    }
}

pub fn load_quests(game: &assets::GameDir) -> Vec<Vec<Quest>> {
    QUEST_SUBJECT_FILES
        .iter()
        .zip(QUEST_CONTENT_FILES)
        .map(|(subjects, contents)| {
            let mut quests = match crate::ui::read_language(game, subjects) {
                Ok((bytes, cp)) => quest_subjects_with_codepage(&bytes, cp),
                Err(_) => Vec::new(),
            };

            if let Ok((bytes, cp)) = crate::ui::read_language(game, contents) {
                quest_contents_with_codepage(&bytes, &mut quests, cp);
            }

            quests
        })
        .collect()
}

pub fn quest_color(quest: &Quest, entered: &Entered) -> u32 {
    let tier = if entered.equipment.evolved() {
        1 + u8::from(entered.skill_mask[0] & formats::net::account::EVOLVED_BIT != 0)
    } else {
        0
    };

    if quest.class_mask != 0 && quest.class_mask & (1 << tier) == 0 {
        return QUEST_GREY;
    }

    let (byte, bit) = quest.flag;

    if (1..=11).contains(&byte)
        && u32::from(entered.quest_flags[byte as usize]) & (1u32 << (bit & 31)) != 0
    {
        return QUEST_GREEN;
    }

    let level = entered.status.shown_level() as i32;

    let body = i32::from(entered.equipment.body().index);

    if level < quest.level {
        return QUEST_GREY;
    }

    if quest.level > QUEST_EVOLVED_LEVEL && body % CLASS_DECADE < EVOLVED_DIGIT {
        return QUEST_GREY;
    }

    if entered.equipment.evolved() && entered.skill_mask[0] & 0x40000000 != 0 {
        return QUEST_GREY;
    }

    QUEST_WHITE
}

pub fn quest_created(screen: &mut Screen) {
    screen.page = -1;

    for name in QUEST_TABS {
        if let Some(button) = screen.element_mut(name, ElementType::Button)
            && let Some(state) = button.widget.button_mut()
        {
            state.toggle = true;
        }
    }

    if let Some(list) = screen.element_mut("ContentList", ElementType::ListView)
        && let Some(state) = list.widget.list_mut()
    {
        state.ignore_selection = true;
    }
}

pub fn quest_page(screen: &mut Screen, tab: i32, quests: &[Vec<Quest>], entered: &Entered) {
    screen.page = tab;

    if !screen.open {
        return;
    }

    for (index, name) in QUEST_TABS.iter().enumerate() {
        if let Some(button) = screen.element_mut(name, ElementType::Button)
            && let Some(state) = button.widget.button_mut()
        {
            state.checked = index as i32 == tab;
        }
    }

    clear_list(screen, "TitleList");

    clear_list(screen, "ContentList");

    let rows: Vec<(String, String, u32, bool)> = quests
        .get(tab.max(0) as usize)
        .map(|tab| {
            tab.iter()
                .map(|quest| {
                    (
                        quest.title.clone(),
                        thousands(quest.level as u64),
                        quest_color(quest, entered),
                        quest.level != 0,
                    )
                })
                .collect()
        })
        .unwrap_or_default();

    let Some(list) = screen.element_mut("TitleList", ElementType::ListView) else {
        return;
    };

    let Some(state) = list.widget.list_mut() else {
        return;
    };

    for (title, level, color, has_level) in rows {
        let row = state.add_item("");

        state.items[row].payload = row as i64;

        for child in &mut state.items[row].children {
            match child.name.as_str() {
                "Title" => {
                    child.set_display(&title);

                    set_font_color(child, color);
                }
                "Lv" => {
                    child.visible = has_level;

                    if has_level {
                        child.set_display(&level);
                    }

                    set_font_color(child, color);
                }
                "L" => {
                    child.visible = has_level;

                    set_font_color(child, color);
                }
                _ => child.visible = false,
            }
        }
    }
}

pub fn quest_select(screen: &mut Screen, row: usize, quests: &[Vec<Quest>]) {
    let Some(quest) = usize::try_from(screen.page)
        .ok()
        .and_then(|tab| quests.get(tab))
        .and_then(|tab| tab.get(row))
    else {
        return;
    };

    let lines = quest.lines.clone();

    clear_list(screen, "ContentList");

    if lines.is_empty() {
        return;
    }

    let Some(list) = screen.element_mut("ContentList", ElementType::ListView) else {
        return;
    };

    let Some(state) = list.widget.list_mut() else {
        return;
    };

    for line in lines {
        let index = state.add_item("");

        state.items[index].payload = row as i64;

        for child in &mut state.items[index].children {
            match child.name.as_str() {
                "Msg" => {
                    child.set_display(&line.text);

                    set_font_color(child, line.color);
                }
                _ => child.visible = false,
            }
        }
    }
}

pub fn quest_command(
    screen: &mut Screen,
    name: &str,
    quests: &[Vec<Quest>],
    entered: &Entered,
) -> bool {
    let Some(tab) = QUEST_TABS.iter().position(|t| *t == name) else {
        return false;
    };

    quest_page(screen, tab as i32, quests, entered);

    true
}

pub fn portal_air_move(screen: &mut Screen, quests: &[Vec<Quest>], strings: &Strings) {
    for (name, record) in portal::AIR_MOVE_LABELS {
        let value = strings.translate(&format!("ss_{record}"));

        if let Some(element) = screen.element_mut(name, ElementType::Static) {
            element.set_display(&value);

            if name == portal::AIR_MOVE_LABELS[0].0 {
                set_font_color(element, portal::AIR_MOVE_HEADER_COLOR);
            }
        }
    }

    text(screen, portal::LIST, "");

    let rows = portal::air_move_rows(quests, strings);

    clear_list(screen, portal::LIST);

    let Some(list) = screen.element_mut(portal::LIST, ElementType::ListView) else {
        return;
    };

    let Some(state) = list.widget.list_mut() else {
        return;
    };

    for row in rows {
        let index = state.add_item("");

        for child in &mut state.items[index].children {
            match child.name.as_str() {
                portal::ROW_NAME => child.set_display(&row.name),
                portal::ROW_POS => child.set_display(&row.pos),
                _ => {}
            }
        }
    }
}

pub fn portal_scroll(
    screen: &mut Screen,
    rows: &[formats::text::potal_pos::Row],
    strings: &Strings,
    page: i32,
) {
    let page = if (0..=5).contains(&page) { page } else { 0 };

    text(
        screen,
        "Title",
        &strings.translate(&format!("ss_{}", 310 + page)),
    );

    if let Some(title) = screen.element_mut("Title", ElementType::Static) {
        set_font_color(title, 0xffff_ffff);
    }

    for (name, record) in portal::SCROLL_LABELS {
        let value = strings.translate(&format!("ss_{record}"));

        if let Some(element) = screen.element_mut(name, ElementType::Static) {
            element.set_display(&value);
        }
    }

    text(screen, portal::LIST, "");

    clear_list(screen, portal::LIST);

    let Some(list) = screen.element_mut(portal::LIST, ElementType::ListView) else {
        return;
    };

    let Some(state) = list.widget.list_mut() else {
        return;
    };

    for row in rows {
        let index = state.add_item("");

        state.items[index].payload = (index + 1) as i64;

        for child in &mut state.items[index].children {
            match child.name.as_str() {
                portal::ROW_POS => child.set_display(&row.zone),
                portal::ROW_NAME => child.set_display(&row.name),
                portal::ROW_MSG => child.set_display(&row.coords),
                _ => {}
            }
        }
    }
}

pub fn portal_selected(screen: &Screen) -> Option<u32> {
    let list = screen.element(portal::LIST, ElementType::ListView)?;

    let state = list.widget.list()?;

    let row = u32::try_from(state.selected).ok()?;

    (usize::try_from(row).ok()? < state.items.len()).then_some(row)
}

fn clear_list(screen: &mut Screen, name: &str) {
    if let Some(list) = screen.element_mut(name, ElementType::ListView)
        && let Some(state) = list.widget.list_mut()
    {
        state.clear_items();
    }
}

fn set_font_color(element: &mut ui::element::Element, argb: u32) {
    for layer in &mut element.layers {
        layer.font_color.set_all(argb);
    }
}

fn parse_leading_int(value: &str) -> i32 {
    let text = value.trim_start();

    let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();

    digits.parse().unwrap_or(0)
}

pub fn party_created(screen: &mut Screen) {
    party_controls(screen, 0, false);

    party_bounds(screen, 0);
}

pub fn server_popup_refresh(screen: &mut Screen, rows: &[super::listener::Channel]) {
    screen
        .placed
        .ui
        .panel
        .set_visible_by_tagname("loginserver", true);

    let Some(list) = screen
        .placed
        .ui
        .panel
        .elements
        .iter_mut()
        .find(|e| e.name == super::listener::CHANNEL_VIEW)
        .and_then(|e| e.widget.list_mut())
    else {
        return;
    };

    super::listener::fill_channel_list(list, rows, super::listener::CrownRule::Hidden);
}

pub fn server_popup_channel(screen: &Screen, row: usize) -> Option<i32> {
    let list = screen
        .placed
        .ui
        .panel
        .elements
        .iter()
        .find(|element| element.name == super::listener::CHANNEL_VIEW)
        .and_then(|element| element.widget.list())?;

    list.items.get(row)?;

    i32::try_from(row).ok()?.checked_add(1)
}

pub fn store_refresh(screen: &mut Screen, entered: &Entered) {
    if !screen.open {
        return;
    }

    let amount = entered.gold as i32;

    let magnitude = thousands(u64::from(amount.unsigned_abs()));

    let money = if amount < 0 {
        format!("-{magnitude}")
    } else {
        magnitude
    };

    text(screen, "Money", &money);
}

pub fn store_created(screen: &mut Screen) {
    let resource = screen.placed.ui.resource.clone();

    for (name, kind) in [
        ("Name", ElementType::Static),
        ("Title", ElementType::Static),
        ("Money", ElementType::Static),
        ("Btn", ElementType::Button),
        ("CloseBtn", ElementType::Button),
    ] {
        screen
            .placed
            .ui
            .panel
            .element_or_create(name, kind, &resource);
    }

    for cell in 0..super::store::CELLS {
        let element = screen.placed.ui.panel.element_or_create(
            &format!("Inven{cell}"),
            ElementType::IconButton,
            &resource,
        );

        element.tooltip.clear();

        element.tooltip_key.clear();

        let Some(state) = element.set_icon_contents(None) else {
            continue;
        };

        state.collection = None;

        state.pulse_fade = None;

        state.icon_slot_type = 2;

        state.icon_slot_index = cell as i16;
    }
}

pub fn store_owner_fill(
    screen: &mut Screen,
    entered: &Entered,
    title: &str,
    items: &ItemList,
    icons: &IconTable,
) {
    text(screen, "Name", &entered.name());

    text(screen, "Title", title);

    text(screen, "Money", &thousands(u64::from(entered.gold)));

    let resource = screen.placed.ui.resource.clone();

    for cell in 0..super::store::CELLS {
        clear_cell(screen, &format!("Inven{cell}"), items, icons, &resource);

        store_exhibition(screen, cell, false);
    }

    show(screen, "Money", true);

    show(screen, "Btn", true);

    show(screen, "CloseBtn", false);
}

pub fn coin_input_fill(screen: &mut Screen, body: &str) {
    coin_input_option(screen, "Div", false, false);

    coin_input_option(screen, "Expo", false, false);

    text(screen, "msg", body);

    if let Some(element) = screen.element_mut("edit", ElementType::ImeEditBox)
        && let Some(edit) = element.widget.edit_mut()
    {
        edit.text.clear();

        edit.preedit = None;

        edit.caret = 0;

        edit.anchor = 0;

        edit.scroll = 0;

        edit.insert = true;
    }
}

pub fn coin_input_option(screen: &mut Screen, prefix: &str, visible: bool, checked: bool) {
    show(screen, &format!("{prefix}Chk"), visible);

    show(screen, &format!("{prefix}Lbl"), visible);

    show(screen, &format!("{prefix}Mark"), visible && checked);
}

pub fn cargo_lock(screen: &mut Screen, source_slot: usize, locked: bool) -> bool {
    let bound = screen
        .cargo_widgets
        .cells
        .get(source_slot)
        .copied()
        .flatten();

    let Some(element) = cargo_element_mut(screen, bound) else {
        return false;
    };

    let Some(state) = element.widget.button_mut() else {
        return false;
    };

    state.icon_locked = locked;

    true
}

pub fn inventory_lock(screen: &mut Screen, source_slot: usize, locked: bool) -> bool {
    let Some(bound) = screen
        .inventory_widgets
        .cells
        .get(source_slot)
        .copied()
        .flatten()
    else {
        return false;
    };

    let Some(element) = screen
        .placed
        .ui
        .panel
        .elements
        .iter_mut()
        .find(|e| e.instance == bound)
    else {
        return false;
    };

    let Some(state) = element.widget.button_mut() else {
        return false;
    };

    state.icon_locked = locked;

    true
}

pub fn combine_staged(
    screen: &mut Screen,
    cell: usize,
    item: &formats::net::account::Item,
    items: &ItemList,
    icons: &IconTable,
) {
    let bound = screen
        .combine_widgets
        .materials
        .get(cell)
        .copied()
        .flatten();

    combine_icon(screen, bound, item, items, icons);
}

pub fn combine_unstaged(screen: &mut Screen, cell: usize, items: &ItemList, icons: &IconTable) {
    combine_staged(screen, cell, &Default::default(), items, icons);
}

pub fn option_stone_staged(
    screen: &mut Screen,
    cell: usize,
    item: &formats::net::account::Item,
    items: &ItemList,
    icons: &IconTable,
) {
    combine_staged(screen, cell, item, items, icons);

    combine_line(screen, cell, true);
}

pub fn option_stone_unstaged(
    screen: &mut Screen,
    cell: usize,
    items: &ItemList,
    icons: &IconTable,
) {
    combine_unstaged(screen, cell, items, icons);

    combine_line(screen, cell, false);
}

pub fn option_stone_preview(
    screen: &mut Screen,
    preview: Option<super::combine::OptionStonePreview>,
    items: &ItemList,
    icons: &IconTable,
) {
    let outputs = preview.map_or([Default::default(); 2], |p| [p.piece, p.stone]);

    for (bound, item) in screen.combine_widgets.results.into_iter().zip(outputs) {
        combine_icon(screen, bound, &item, items, icons);
    }

    combine_line(screen, 3, preview.is_some_and(|p| !p.stone.is_empty()));
}

pub fn option_stone_reset(screen: &mut Screen, items: &ItemList, icons: &IconTable) {
    for cell in 0..3 {
        option_stone_unstaged(screen, cell, items, icons);
    }

    for bound in screen.combine_widgets.results {
        combine_icon(screen, bound, &Default::default(), items, icons);
    }
}

pub fn store_stocked(
    screen: &mut Screen,
    slot: usize,
    item: &formats::net::account::Item,
    price: i32,
    items: &ItemList,
    icons: &IconTable,
) {
    store_exhibition(
        screen,
        slot,
        !item.is_empty() && super::store::display_only(price),
    );

    let name = format!("Inven{slot}");

    let resource = screen.placed.ui.resource.clone();

    set_cell_icon(screen, &name, item, items, icons, &resource);

    if let Some(element) = screen.element_mut(&name, ElementType::IconButton)
        && let Some(state) = element.widget.button_mut()
        && let Some(contents) = state.icon_contents.as_mut()
    {
        contents.asking_price = price;
    }
}

pub fn store_announced(screen: &mut Screen) {
    show(screen, "Btn", false);

    show(screen, "CloseBtn", true);
}

pub fn store_fill(
    screen: &mut Screen,
    contents: &formats::net::autotrade::Contents,
    seller: &str,
    mine: bool,
    items: &ItemList,
    icons: &IconTable,
) -> usize {
    let resource = screen.placed.ui.resource.clone();

    text(screen, "Title", &contents.title());

    text(screen, "Name", seller);

    let mut filled = 0;

    for (cell, item, price) in contents.panel_slots().collect::<Vec<_>>() {
        store_exhibition(
            screen,
            cell,
            !item.is_empty() && super::store::display_only(price),
        );

        let name = format!("Inven{cell}");

        set_cell_icon(screen, &name, &item, items, icons, &resource);

        if item.is_empty() {
            continue;
        }

        filled += 1;

        if let Some(element) = screen.element_mut(&name, ElementType::IconButton)
            && let Some(state) = element.widget.button_mut()
            && let Some(contents) = state.icon_contents.as_mut()
        {
            contents.asking_price = price;
        }
    }

    show(screen, "Btn", false);

    show(screen, "Money", mine);

    show(screen, "CloseBtn", mine);

    filled
}

pub fn trade_created(screen: &mut Screen) {
    let resource = screen.placed.ui.resource.clone();

    for (name, kind) in [
        ("Name", ElementType::Static),
        ("NameMy", ElementType::Static),
        ("Money", ElementType::Static),
        ("MoneyMy", ElementType::Static),
        ("Check", ElementType::CheckBox),
        ("CheckMy", ElementType::CheckBox),
    ] {
        screen
            .placed
            .ui
            .panel
            .element_or_create(name, kind, &resource);
    }

    for prefix in ["Inven", "InvenMy"] {
        for slot in 0..15 {
            let element = screen.placed.ui.panel.element_or_create(
                &format!("{prefix}{slot}"),
                ElementType::IconButton,
                &resource,
            );

            if let Some(state) = element.set_icon_contents(None) {
                state.icon_slot_type = 2;

                state.icon_slot_index = slot as i16;
            }
        }
    }
}

pub fn trade_fill(
    screen: &mut Screen,
    session: &super::trade::Session,
    items: &ItemList,
    icons: &IconTable,
) {
    if !screen.open {
        return;
    }

    let resource = screen.placed.ui.resource.clone();

    for (prefix, offer) in [("Inven", &session.theirs), ("InvenMy", &session.mine)] {
        for (slot, item) in offer.items.iter().enumerate() {
            set_cell_icon(
                screen,
                &format!("{prefix}{slot}"),
                item,
                items,
                icons,
                &resource,
            );
        }
    }

    text(screen, "Name", &session.partner_name);

    text(screen, "NameMy", &session.my_name);

    text(screen, "Money", &thousands(u64::from(session.theirs.gold)));

    text(screen, "MoneyMy", &thousands(u64::from(session.mine.gold)));

    trade_checks(screen, session);
}

pub fn trade_checks(screen: &mut Screen, session: &super::trade::Session) {
    for (name, checked) in [
        ("Check", session.theirs.ready != 0),
        ("CheckMy", session.ready_control),
    ] {
        if let Some(state) = screen
            .element_mut(name, ElementType::CheckBox)
            .and_then(|e| e.widget.button_mut())
        {
            state.checked = checked;
        }
    }
}

pub fn inventory_broadcast(panels: &mut Panels, entered: &Entered) {
    let Panels {
        screens,
        strings,
        items,
        icons,
        ..
    } = panels;

    let strings = &*strings;

    for screen in screens.iter_mut() {
        if screen.content == CONTENT_INVENTORY {
            inventory_refresh(screen, entered, items, icons, strings);
        }
    }
}

pub fn refresh_status_panels(panels: &mut Panels, entered: &Entered) {
    sheet_broadcast(panels, entered, 10);

    let Panels {
        screens,
        strings,
        items,
        ..
    } = panels;

    let strings = &*strings;

    for screen in screens.iter_mut() {
        match screen.content {
            CONTENT_SKILL => skill_refresh(screen, entered, items, strings),
            CONTENT_STORE => store_refresh(screen, entered),
            _ => {}
        }
    }
}

pub fn sheet_broadcast(panels: &mut Panels, entered: &Entered, event: i32) {
    if !matches!(event, 10 | 11) || !panels.field_close_owner_present {
        return;
    }

    let Panels {
        screens,
        strings,
        sheet_live_pending,
        ..
    } = panels;

    if let Some(screen) = screens
        .iter_mut()
        .find(|s| s.content == CONTENT_CHARACTER && s.open)
    {
        sheet_refresh_current(screen, strings, entered);

        *sheet_live_pending = true;
    }
}

pub fn skill_opened(panels: &mut Panels, entered: &Entered) {
    let Panels {
        screens,
        strings,
        items,
        ..
    } = panels;

    let Some(screen) = screens
        .iter_mut()
        .find(|screen| screen.content == CONTENT_SKILL)
    else {
        return;
    };

    skill_refresh(screen, entered, items, strings);
}

pub fn refresh_all(panels: &mut Panels, entered: &Entered) {
    let level = i32::from(entered.status.level as i16);

    if level == panels.analytics_next_level {
        panels.analytics.level_up(entered);
    }

    panels.analytics_next_level = level + 1;

    {
        let Panels {
            screens,
            strings,
            items,
            icons,
            ..
        } = panels;

        let strings = &*strings;

        for screen in screens.iter_mut() {
            match screen.content {
                CONTENT_CHARACTER => sheet_refresh(screen, strings, entered),
                CONTENT_INVENTORY => inventory_refresh(screen, entered, items, icons, strings),
                CONTENT_CARGO => cargo_refresh(screen, entered, items, icons),
                _ => {}
            }
        }
    }

    refresh_status_panels(panels, entered);
}

pub fn refresh_opened(panels: &mut Panels, entered: &Entered) {
    let cargo = std::mem::take(&mut panels.cargo_open_pending);

    let inventory = std::mem::take(&mut panels.inventory_open_pending);

    let sheet = std::mem::take(&mut panels.sheet_open_pending);

    if !cargo && !inventory && !sheet {
        return;
    }

    let player_present = panels.field_close_owner_present;

    let Panels {
        screens,
        items,
        icons,
        strings,
        ..
    } = panels;

    for screen in screens.iter_mut().filter(|s| s.open) {
        if cargo && screen.content == CONTENT_CARGO {
            cargo_refresh(screen, entered, items, icons);

            cargo_page(screen, screen.page);
        } else if inventory && screen.content == CONTENT_INVENTORY {
            inventory_refresh(screen, entered, items, icons, strings);
        } else if sheet && screen.content == CONTENT_CHARACTER && player_present {
            sheet_refresh_current(screen, strings, entered);
        }
    }
}

pub fn create_all(panels: &mut Panels) {
    let strings = panels.strings.clone();

    for screen in panels.screens.iter_mut() {
        match screen.content {
            super::panels::CONTENT_ARENA => {
                if let Some(button) = screen.element_mut("RoyalArena", ElementType::Button) {
                    button.set_tooltip("im_10034", &strings);
                }
            }
            CONTENT_CHARACTER => {
                sheet_bound(screen);

                sheet_created(screen);

                panels.spend_repeat.reset();
            }
            CONTENT_INVENTORY => {
                inventory_created(screen);
            }
            CONTENT_CARGO => {
                cargo_created(screen);

                cargo_page(screen, 0);
            }
            CONTENT_MISSION_MIX => mission_created(screen),
            super::mix::CONTENT => mix_created(screen),
            super::panels::CONTENT_ITEM_MIX4 | super::panels::CONTENT_OPTION_STONE => {
                combine_created(screen, &strings)
            }
            super::refine::CONTENT => {
                refine_created(screen, &strings);

                refine_opened(screen, &strings);
            }
            super::collection::CONTENT => {
                if let Some(button) =
                    screen.element_mut(super::collection::SIGN_UP, ElementType::Button)
                {
                    button.enabled = false;
                }
            }
            CONTENT_ITEM_SHOP => shop_created(screen),
            CONTENT_STORE => store_created(screen),
            CONTENT_SKILL_MASTER => skill_master_created(screen),
            CONTENT_SKILL => skill_created(screen),
            CONTENT_HELP => help_created(screen),
            CONTENT_QUEST => quest_created(screen),
            CONTENT_PARTY => party_created(screen),
            super::trade::CONTENT => trade_created(screen),
            _ => {}
        }
    }
}

pub fn build_all(panels: &mut Panels, entered: &Entered) {
    create_all(panels);

    refresh_all(panels, entered);
}

pub fn load_help(game: &assets::GameDir) -> Vec<Vec<ColorLine>> {
    HELP_FILES
        .iter()
        .map(|file| match crate::ui::read_language(game, file) {
            Ok((bytes, cp)) => help_lines_with_codepage(&bytes, cp),
            Err(_) => Vec::new(),
        })
        .collect()
}

pub fn mission_created(screen: &mut Screen) {
    mix_created(screen);
}

pub fn mission_refresh(
    screen: &mut Screen,
    state: &super::mission::State,
    tables: &super::mission::Tables,
) {
    for cell in 0..formats::data::mission::OPTIONS {
        let slot = state.slot.get(cell);

        let name = format!("Inven{cell}");

        let contents = slot.and_then(|slot| {
            let (sheet, sheet_uv) = tables.icon_sheet(slot.id)?;

            Some(ui::icon::IconContents {
                kind: 5,
                item: slot.carried,
                index: slot.id,
                count: 0,
                refine: -1,
                sheet,
                sheet_uv,
                grade: Default::default(),
                overlay: None,
                asking_price: -1,
                sweep: None,
                blink_off: false,
            })
        });

        if let Some(element) = screen.element_mut(&name, ElementType::IconButton)
            && let Some(button) = element.set_icon_contents(contents)
        {
            button.set_tooltip_binding(slot.map(|s| ui::icon::TooltipBinding {
                kind: 5,
                index: s.id,
                item: s.carried,
                help_id: 0,
            }));

            button.pulse_fade = None;

            button.checked = slot.is_some_and(|s| state.selected == Some(s.id));

            button.icon_slot_index = slot.map_or(cell as i16, |s| s.id as i16);

            if slot.is_some() {
                button.toggle = true;
            }
        }

        show(screen, &name, slot.is_some());
    }
}

pub fn mission_clear(screen: &mut Screen) {
    for (prefix, count) in [("Inven", 24), ("Mat", 8)] {
        for cell in 0..count {
            if let Some(button) = screen
                .element_mut(&format!("{prefix}{cell}"), ElementType::IconButton)
                .and_then(|e| e.set_icon_contents(None))
            {
                button.pulse_fade = None;

                if prefix == "Inven" {
                    button.icon_slot_index = -1;
                }
            }
        }
    }

    for cell in 0..8 {
        text(screen, &format!("MatText{cell}"), "");
    }

    text(screen, "Money", "");
}

pub fn mission_details(
    screen: &mut Screen,
    details: &super::mission::Details,
    items: &ItemList,
    icons: &IconTable,
) {
    let resource = screen.placed.ui.resource.clone();

    for cell in 0..8 {
        let item = &details.material[cell];

        set_cell_icon(screen, &format!("Mat{cell}"), item, items, icons, &resource);

        let name = if item.is_empty() {
            "".to_string()
        } else {
            items.name(usize::from(item.index)).unwrap_or_default()
        };

        let (mut label, color) = match details.counts[cell] {
            Some((have, need)) => (
                format!("{name}<n> {have} / {need}"),
                if have >= need {
                    0xFFFF_FFFF
                } else {
                    0xFFAA_0000
                },
            ),
            None => (name, 0xFFAA_0000),
        };

        if let Some(caption) = &details.captions[cell] {
            label.clone_from(caption);
        }

        let key = format!("MatText{cell}");

        text(screen, &key, &label);

        if let Some(element) = screen.element_mut(&key, ElementType::Static) {
            set_font_color(element, color);
        }
    }

    text(
        screen,
        "Money",
        &details
            .money
            .map(|n| thousands(n.max(0) as u64))
            .unwrap_or_default(),
    );
}

pub fn mix_clear_materials(screen: &mut Screen) {
    for cell in 0..8 {
        if let Some(e) = screen.element_mut(&format!("Mat{cell}"), ElementType::IconButton)
            && let Some(b) = e.set_icon_contents(None)
        {
            b.pulse_fade = None;
        }

        text(screen, &format!("MatText{cell}"), "");
    }

    text(screen, super::mix::MONEY, "");
}

pub fn mix_created(screen: &mut Screen) {
    screen.mix_recipe_owners = std::array::from_fn(|cell| {
        screen
            .element(&format!("Inven{cell}"), ElementType::IconButton)
            .map(|e| e.instance)
    });

    for (family, count, group) in [
        ("Inven", super::mix::RECIPE_CELLS, cell_group::MIX),
        ("Mat", super::mix::MAT_CELLS, cell_group::NONE),
    ] {
        for cell in 0..count {
            let Some(element) =
                screen.element_mut(&format!("{family}{cell}"), ElementType::IconButton)
            else {
                continue;
            };

            let Some(state) = element.set_icon_contents(None) else {
                continue;
            };

            state.pulse_fade = None;

            state.icon_slot_type = 2;

            state.cell_group = group;

            state.icon_slot_index = cell as i16;
        }
    }

    for cell in 0..super::mix::MAT_CELLS {
        text(screen, &format!("MatText{cell}"), "");
    }
}

pub fn combine_created(screen: &mut Screen, strings: &Strings) {
    let option = screen.content == super::panels::CONTENT_OPTION_STONE;

    screen.combine_widgets = CombineWidgets {
        materials: std::array::from_fn(|i| {
            screen
                .element(&format!("Mat{i}"), ElementType::IconButton)
                .map(|e| e.instance)
        }),
        results: std::array::from_fn(|i| {
            option
                .then(|| screen.element(&format!("Result{i}"), ElementType::IconButton))
                .flatten()
                .map(|e| e.instance)
        }),
        lines: std::array::from_fn(|i| {
            option
                .then(|| screen.element(&format!("Mat{i}Line"), ElementType::Image))
                .flatten()
                .map(|e| e.instance)
        }),
    };

    for (family, count) in [("Mat", 3), ("Result", if option { 2 } else { 0 })] {
        for cell in 0..count {
            let Some(element) =
                screen.element_mut(&format!("{family}{cell}"), ElementType::IconButton)
            else {
                continue;
            };

            let Some(state) = element.set_icon_contents(None) else {
                continue;
            };

            state.pulse_fade = None;

            state.icon_slot_type = 2;

            state.cell_group = 0;

            state.icon_slot_index = cell;
        }
    }

    if option {
        for cell in 0..4 {
            combine_line(screen, cell, false);
        }
    } else {
        text(
            screen,
            "Money",
            &format!("{} {}", strings.translate("uu_278"), thousands(30_000_000)),
        );
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct CombineWidgets {
    pub materials: [Option<ui::element::Instance>; 3],
    results: [Option<ui::element::Instance>; 2],
    lines: [Option<ui::element::Instance>; 4],
}

fn combine_icon(
    screen: &mut Screen,
    bound: Option<ui::element::Instance>,
    item: &formats::net::account::Item,
    items: &ItemList,
    icons: &IconTable,
) {
    let Some(bound) = bound else {
        return;
    };

    let contents = ui::icon::set_icon(item, items, icons, &screen.placed.ui.resource);

    if let Some(element) = screen
        .placed
        .ui
        .panel
        .elements
        .iter_mut()
        .find(|e| e.instance == bound)
        && let Some(state) = element.set_icon_contents(contents)
    {
        state.pulse_fade = None;
    }
}

fn combine_line(screen: &mut Screen, cell: usize, visible: bool) {
    let Some(bound) = screen.combine_widgets.lines.get(cell).copied().flatten() else {
        return;
    };

    if let Some(element) = screen
        .placed
        .ui
        .panel
        .elements
        .iter_mut()
        .find(|e| e.instance == bound)
    {
        element.visible = visible;
    }
}

pub fn refine_created(screen: &mut Screen, strings: &Strings) {
    refine_delay_created(screen);

    let resource = screen.placed.ui.resource.clone();

    let bind = |screen: &mut Screen, name: &str, kind, key: &str| {
        let owner = screen
            .placed
            .ui
            .panel
            .element_or_create(name, kind, &resource)
            .instance;

        screen.refine_widgets.insert(key.to_owned(), owner);
    };

    for name in ["RateString", "RateValue", "ResultSanc", "ItemName", "Money"] {
        bind(screen, name, ElementType::Static, name);
    }

    bind(
        screen,
        "ResultSancEdit",
        ElementType::EditBox,
        "ResultSancEdit",
    );

    bind(screen, "Auto", ElementType::CheckBox, "Auto");

    bind(screen, "Refine", ElementType::Button, "Refine");

    bind(screen, "Stop", ElementType::Button, "Stop");

    for cell in 0..7 {
        let name = format!("Mat{cell}");

        bind(screen, &name, ElementType::IconButton, &name);

        if let Some(element) = screen.refine_element_mut(&name, ElementType::IconButton)
            && let Some(state) = element.set_icon_contents(None)
        {
            state.pulse_fade = None;

            state.icon_slot_type = 2;

            state.icon_slot_index = cell as i16;

            state.cell_group = 0;
        }

        let label = format!("MatText{cell}");

        bind(screen, &label, ElementType::Static, &label);

        text(screen, &label, "");
    }

    bind(screen, "ResultItem", ElementType::IconButton, "ResultItem");

    bind(screen, "Sancup", ElementType::Button, "SancUp");

    bind(screen, "SancDown", ElementType::Button, "SancDown");

    text_any(
        screen,
        "Stop",
        &strings.translate(super::refine::AUTO_CAPTION),
    );
}

fn refine_delay_created(screen: &mut Screen) {
    let panel = &mut screen.placed.ui.panel;

    for (template, name, x, y, width) in [
        ("SancDown", "AutoDelayDown", 149, 376, 12),
        ("ResultSancEdit", "AutoDelayValue", 163, 370, 40),
        ("SancUp", "AutoDelayUp", 207, 376, 12),
        ("ItemName", "AutoDelayLabel", 134, 354, 85),
    ] {
        if panel.elements.iter().any(|e| e.name == name) {
            continue;
        }

        let Some(template) = panel.elements.iter().find(|e| e.name == template) else {
            continue;
        };

        let mut element = template.clone_template();

        element.name = name.to_owned();

        element.name_hash = ui::hash::name_hash(name);

        element.x = x;

        element.y = y;

        element.width = width;

        element.visible = true;

        if name == "AutoDelayLabel" {
            element.set_display("Intervalo (ms)");
        }

        panel.elements.push(element);
    }

    refine_delay(screen, super::refine::AutoDelay::default().milliseconds());
}

pub fn refine_delay(screen: &mut Screen, milliseconds: u32) {
    if let Some(element) = screen.element_mut("AutoDelayValue", ElementType::EditBox) {
        let value = milliseconds.to_string();

        element.set_display(&value);

        if let Some(edit) = element.widget.edit_mut() {
            edit.text = value;

            edit.caret = edit.text.len() as i32;

            edit.anchor = edit.caret;

            edit.scroll = 0;
        }
    }
}

pub fn refine_opened(screen: &mut Screen, strings: &Strings) {
    for cell in 0..7 {
        if let Some(element) =
            screen.refine_element_mut(&format!("Mat{cell}"), ElementType::IconButton)
            && let Some(state) = element.set_icon_contents(None)
        {
            state.pulse_fade = None;
        }

        text(screen, &format!("MatText{cell}"), "");
    }

    for name in ["ItemName", "Money", "RateValue", "RateString"] {
        text(screen, name, "");
    }

    refine_target_editor(screen, "");

    if let Some(state) = screen
        .refine_element_mut("ResultItem", ElementType::IconButton)
        .and_then(|e| e.set_icon_contents(None))
    {
        state.pulse_fade = None;
    }

    refine_auto_mirror(screen, false);

    refine_controls(screen, false, true, strings);
}

pub fn refine_closed(screen: &mut Screen, strings: &Strings) {
    for cell in 0..super::refine::CELLS {
        text(screen, &format!("MatText{cell}"), "");
    }

    refine_target_editor(screen, "");

    refine_auto_mirror(screen, false);

    refine_stop_caption(screen, false, strings);
}

pub fn mix_refresh(
    screen: &mut Screen,
    browser: &super::mix::Browser,
    list: &formats::data::mix_list::MixList,
    help: &super::mix::HelpTables,
    items: &ItemList,
    icons: &IconTable,
    strings: &Strings,
) {
    let resource = screen.placed.ui.resource.clone();

    let drawn = browser.drawn();

    for cell in 0..super::mix::RECIPE_CELLS {
        let recipe = drawn.get(cell).and_then(|&index| list.recipe(index));

        let item = recipe.and_then(super::mix::cell_item).unwrap_or_default();

        let contents = if recipe.is_some_and(|r| r.result_item == 0) {
            drawn.get(cell).and_then(|&id| help.icon(id))
        } else {
            ui::icon::set_icon(&item, items, icons, &resource).map(|mut icon| {
                icon.kind = 2;

                icon
            })
        };

        if let Some(bound) = screen.mix_recipe_owners[cell]
            && let Some(element) = screen
                .placed
                .ui
                .panel
                .elements
                .iter_mut()
                .find(|e| e.instance == bound)
        {
            if let Some(state) = element.set_icon_contents(contents) {
                state.pulse_fade = None;
            }

            let Some(state) = element.widget.button_mut() else {
                continue;
            };

            state.icon_slot_index = drawn.get(cell).map_or(cell as i16, |&index| index as i16);

            if let Some(&id) = drawn.get(cell).filter(|_| recipe.is_some()) {
                state.set_tooltip_binding(Some(ui::icon::TooltipBinding {
                    kind: 2,
                    index: i32::from(item.index),
                    item,
                    help_id: id as i32,
                }));
            }

            element.visible = recipe.is_some();
        }
    }

    for cell in 0..super::mix::MAT_CELLS {
        let shown = browser.needs.get(cell);

        let item = shown.and_then(|shown| shown.item).unwrap_or_default();

        set_cell_icon(
            screen,
            &format!("Mat{cell}"),
            &item,
            items,
            icons,
            &resource,
        );

        if let Some(shown) = shown
            && let Some(icon) = screen
                .element_mut(&format!("Mat{cell}"), ElementType::IconButton)
                .and_then(|e| e.widget.button_mut())
                .and_then(|b| b.icon_contents.as_mut())
        {
            icon.kind = shown.icon_kind;
        }

        let name = format!("MatText{cell}");

        let (label, argb) = match shown {
            Some(shown) => super::mix::mat_label(shown, list, items, strings),
            None => (String::new(), super::mix::MAT_LABEL_ARGB.0),
        };

        text(screen, &name, &label);

        if let Some(element) = screen.element_mut(&name, ElementType::Static) {
            set_font_color(element, argb);
        }
    }

    let gold = browser
        .picked
        .or_else(|| browser.cost_visible.then_some(0))
        .and_then(|index| list.recipe(index))
        .map(|recipe| thousands(recipe.gold_cost.max(0) as u64))
        .unwrap_or_default();

    text(screen, super::mix::MONEY, &gold);
}

#[allow(clippy::too_many_arguments)]
pub fn collection_refresh(
    screen: &mut Screen,
    book: &super::collection::Book,
    costume_item: u16,
    filter: super::collection::Filter,
    backpack: &[formats::net::account::Item],
    items: &ItemList,
    icons: &IconTable,
    strings: &Strings,
) {
    let resource = screen.placed.ui.resource.clone();

    if let Some(button) = screen.element_mut(super::collection::SIGN_UP, ElementType::Button) {
        button.enabled = false;
    }

    let split = super::collection::split_for(costume_item);

    let rows = super::collection::rows(&book.table, book.progress.as_ref(), split, filter);

    let Some(list) = screen
        .element_mut(super::collection::LIST_VIEW, ElementType::ListView)
        .and_then(|e| e.widget.list_mut())
    else {
        return;
    };

    list.clear_items();

    for row in &rows {
        let Some(entry) = book.table.get(row.index) else {
            continue;
        };

        let states = super::collection::cells(entry, book.progress.as_ref(), backpack, items);

        let complete = row.complete.unwrap_or(false);

        let item = list.add_item("");

        let listed = &mut list.items[item];

        listed.payload = row.index as i64;

        for child in &mut listed.children {
            let name = child.name.clone();

            if name.eq_ignore_ascii_case(super::collection::COUNT_LABEL) {
                match complete {
                    true => child.visible = false,
                    false => child.set_display(
                        &super::collection::rebuild_count(entry, book.progress.as_ref()).label(),
                    ),
                }

                continue;
            }

            if let Some(reward) = super::collection::REWARD_LABELS
                .iter()
                .position(|l| name.eq_ignore_ascii_case(l))
            {
                let (effect, value) = if reward == 0 {
                    (entry.primary_reward_effect, entry.primary_reward_value)
                } else {
                    (entry.secondary_reward_effect, entry.secondary_reward_value)
                };

                if reward == 1 && effect == 0 {
                    child.visible = false;
                } else {
                    child.visible = true;

                    child.set_display(&super::collection::reward_label(effect, value, strings));
                }

                if reward == 0 && entry.secondary_reward_effect != 0 {
                    child.y -= 10;
                }

                if complete {
                    set_font_color(child, super::collection::COMPLETE_ARGB);
                }

                continue;
            }

            if name.eq_ignore_ascii_case(super::collection::RESULT_IMAGE) {
                child.visible = complete;

                continue;
            }

            let Some(cell) = name
                .strip_prefix("Item_")
                .or_else(|| name.strip_prefix("item_"))
                .and_then(|n| n.parse::<usize>().ok())
                .filter(|&n| n < super::collection::ITEM_CELLS)
            else {
                continue;
            };

            child.visible = entry.need[cell].index != 0;

            if entry.need[cell].index != 0 {
                let need = super::combine::read_item(&entry.need[cell].to_bytes());

                child.set_icon_contents(ui::icon::set_icon(&need, items, icons, &resource).map(
                    |mut contents| {
                        contents.kind = 7;

                        contents.count = 0;

                        contents
                    },
                ));
            }

            if let Some(state) = child.widget.button_mut() {
                state.icon_slot_index = item as i16;

                if entry.need[cell].index != 0 {
                    state.toggle = true;

                    state.collection = Some(ui::icon::CollectionIcon::resolve(
                        states[cell] as i8,
                        &resource,
                    ));
                }
            }
        }
    }
}

pub fn guild_bind(screen: &mut Screen) {
    use super::guild_panel as guild;

    let resource = screen.placed.ui.resource.clone();

    let panel = &mut screen.placed.ui.panel;

    match screen.content {
        guild::CONTENT => {
            for name in ["Info", "Member"] {
                panel.element_or_create(name, ElementType::Button, &resource);
            }
        }
        guild::PAGE_MEMBER => {
            screen.guild_member_list = Some(
                panel
                    .element_or_create(guild::MEMBER_LIST, ElementType::ListView, &resource)
                    .instance,
            );
        }
        guild::PAGE_INFO | guild::NOTICE_EDIT => {
            let info = screen.content == guild::PAGE_INFO;

            let name = if info {
                guild::NOTICE
            } else {
                guild::NOTICE_EDIT_FIELD
            };

            let pad = panel.element_or_create(name, ElementType::ImeNotePad, &resource);

            if let Some(pad) = pad.widget.edit_mut().and_then(|edit| edit.pad.as_mut()) {
                pad.scrollbar_width = 16;

                if info {
                    pad.read_only = true;
                }
            }

            if info {
                panel
                    .element_or_create(guild::CREATE_BUTTON, ElementType::Button, &resource)
                    .visible = true;

                panel
                    .element_or_create(guild::NOTICE_EDIT_BUTTON, ElementType::Button, &resource)
                    .visible = false;

                for name in guild::INFO_VALUES {
                    panel
                        .element_or_create(name, ElementType::Static, &resource)
                        .set_display("");
                }

                for index in guild::INFO_LABELS {
                    panel
                        .element_or_create(&format!("Info{index}"), ElementType::Static, &resource)
                        .visible = false;
                }
            }
        }
        _ => {}
    }
}

pub fn guild_info_refresh(screen: &mut Screen, guild: &super::guild_panel::Guild) {
    let resource = screen.placed.ui.resource.clone();

    for label in super::guild_panel::info_labels(guild) {
        if label.name == super::guild_panel::NOTICE && !guild.session.in_a_guild() {
            continue;
        }

        let kind = if label.name == super::guild_panel::NOTICE {
            ElementType::ImeNotePad
        } else {
            ElementType::Static
        };

        screen
            .placed
            .ui
            .panel
            .element_or_create(label.name, kind, &resource)
            .set_display(&label.text);
    }

    for (name, visible) in super::guild_panel::info_visibility(guild) {
        let kind = match name.as_str() {
            super::guild_panel::NOTICE => ElementType::ImeNotePad,
            super::guild_panel::NOTICE_EDIT_BUTTON | super::guild_panel::CREATE_BUTTON => {
                ElementType::Button
            }
            _ => ElementType::Static,
        };

        screen
            .placed
            .ui
            .panel
            .element_or_create(&name, kind, &resource)
            .visible = visible;
    }
}

fn text_any(screen: &mut Screen, name: &str, value: &str) {
    if screen.content == super::refine::CONTENT {
        if let Some(owner) = screen.refine_widgets.get(name).copied()
            && let Some(e) = screen
                .placed
                .ui
                .panel
                .elements
                .iter_mut()
                .find(|e| e.instance == owner)
        {
            e.set_display(value);
        }

        return;
    }

    if let Some(element) = screen
        .placed
        .ui
        .panel
        .elements
        .iter_mut()
        .find(|e| e.name.eq_ignore_ascii_case(name))
    {
        element.set_display(value);
    }
}

pub fn guild_member_refresh(
    screen: &mut Screen,
    guild: &super::guild_panel::Guild,
    strings: &Strings,
) {
    let rows = super::guild_panel::member_rows(guild);

    let translate = |key: &str| strings.translate(key);

    let Some(list) = screen.guild_member_list_mut() else {
        return;
    };

    let Some(state) = list.widget.list_mut() else {
        return;
    };

    state.clear_items();

    for row in rows {
        let index = state.add_item("");

        state.items[index].payload = index as i64;

        for child in &mut state.items[index].children {
            match child.name.as_str() {
                "UserName" => child.set_display(&row.user_name),
                "GuildLevel" => {
                    child.set_display(&row.guild_level.map(&translate).unwrap_or_default());
                }
                "MemberNotice" => child.set_display(&row.member_notice),
                "ConnDate" => child.set_display(&row.conn_date.text(&translate)),
                "Joindate" => child.set_display(&row.join_date),
                "GuildInfluence" => child.set_display(&row.guild_influence),
                _ => {}
            }
        }
    }
}

pub fn guild_popup_refresh(
    screen: &mut Screen,
    guild: &super::guild_panel::Guild,
    strings: &Strings,
) -> bool {
    let entries = super::guild_panel::entries(guild);

    let title = guild
        .picked
        .as_ref()
        .map(|p| p.name.clone())
        .unwrap_or_default();

    let Some(base) = screen.guild_popup_base.as_ref() else {
        return false;
    };

    let mut panel = base.clone();

    panel.elements = base
        .elements
        .iter()
        .map(ui::Element::clone_template)
        .collect();

    panel.visible = true;

    screen.placed.ui.panel = panel;

    if let Some(title_widget) =
        screen.element_mut(super::guild_panel::POPUP_TITLE, ElementType::Static)
    {
        title_widget.set_display(&title);
    }

    if entries.is_empty() {
        return false;
    }

    let panel = &mut screen.placed.ui.panel;

    let Some(template) = panel
        .elements
        .iter()
        .find(|e| {
            e.name == super::guild_panel::POPUP_TEMPLATE && e.element_type == ElementType::Button
        })
        .cloned()
    else {
        return false;
    };

    panel
        .elements
        .retain(|e| !e.name.starts_with(super::guild_panel::POPUP_BUTTON));

    let mut y = template.y;

    for entry in &entries {
        let name = format!("{}{}", super::guild_panel::POPUP_BUTTON, entry.tag());

        let caption = strings.translate(entry.caption());

        let mut element = template.clone_template();

        element.name_hash = ui::hash::name_hash(&name);

        element.name = name;

        element.x = template.x;

        element.y = y;

        element.width = template.width;

        element.height = template.height;

        element.visible = true;

        element.set_display(&caption);

        for layer in &mut element.layers {
            layer.font_index = 2;
        }

        for layer in element.layers.iter_mut().take(2) {
            layer.texture_color.set_all(0);
        }

        y += template.height;

        panel.elements.push(element);
    }

    show(screen, super::guild_panel::POPUP_TEMPLATE, false);

    let panel = &mut screen.placed.ui.panel;

    let width = panel.width();

    let height = panel.height() + template.y + (entries.len() as i32) * (template.height - 5)
        - template.height;

    panel.bottom = panel.top + height;

    panel.popup = true;

    panel.keep_popup_on_click = false;

    if let Some(background) = panel.elements.iter_mut().find(|e| {
        e.name == super::guild_panel::POPUP_BACKGROUND && e.element_type == ElementType::Image
    }) {
        background.width = width + super::guild_panel::POPUP_BACKGROUND_MARGIN;

        background.height = height + super::guild_panel::POPUP_BACKGROUND_MARGIN;
    }

    true
}

pub fn refine_refresh(screen: &mut Screen, refine: &super::refine::Refine, strings: &Strings) {
    text(
        screen,
        "ResultSanc",
        &format!("{} -> {}", refine.current, refine.target),
    );

    refine_target_editor(screen, &refine.target.to_string());

    refine_auto_mirror(screen, refine.auto);

    text(screen, "ItemName", "");

    refine_stop_caption(screen, refine.auto, strings);
}

fn refine_target_editor(screen: &mut Screen, value: &str) {
    if let Some(element) = screen.refine_element_mut("ResultSancEdit", ElementType::EditBox) {
        element.set_display(value);

        if let Some(edit) = element.widget.edit_mut() {
            edit.text = value.to_owned();

            edit.preedit = None;

            edit.caret = value.encode_utf16().count() as i32;

            edit.anchor = edit.caret;

            edit.scroll = 0;
        }
    }
}

pub fn refine_result(
    screen: &mut Screen,
    refine: &super::refine::Refine,
    items: &ItemList,
    icons: &IconTable,
) {
    let resource = screen.placed.ui.resource.clone();

    set_cell_icon(
        screen,
        "ResultItem",
        &refine.result,
        items,
        icons,
        &resource,
    );

    if let Some(icon) = screen
        .refine_element_mut("ResultItem", ElementType::IconButton)
        .and_then(|e| e.widget.button_mut())
        .and_then(|b| b.icon_contents.as_mut())
    {
        icon.count = 0;
    }

    refine_piece_name(screen, refine, items);
}

pub fn refine_recipe_icons(
    screen: &mut Screen,
    fill: &super::refine::RecipeFill,
    items: &ItemList,
    icons: &IconTable,
) {
    let resource = screen.placed.ui.resource.clone();

    for (cell, need) in fill.cells.iter().take(7).enumerate() {
        let name = format!("Mat{cell}");

        set_cell_icon(screen, &name, &need.item, items, icons, &resource);

        if need.bare
            && let Some(icon) = screen
                .refine_element_mut(&name, ElementType::IconButton)
                .and_then(|element| element.widget.button_mut())
                .and_then(|state| state.icon_contents.as_mut())
        {
            icon.kind = 2;

            icon.refine = -1;
        }
    }
}

pub fn refine_recipe_labels(
    screen: &mut Screen,
    refine: &super::refine::Refine,
    list: &formats::data::mix_list::MixList,
    items: &ItemList,
    strings: &Strings,
) {
    for label in super::refine::cleared_labels() {
        text(screen, &label, "");

        if let Some(element) = screen.refine_element_mut(&label, ElementType::Static) {
            set_font_color(element, super::refine::MAT_LABEL_ARGB);
        }
    }

    let cost = usize::try_from(refine.mode)
        .ok()
        .and_then(|mode| list.recipe(mode))
        .map_or(0, |row| row.gold_cost.max(0));

    text(screen, "Money", &thousands(cost as u64));

    if let Some(fill) = &refine.recipe {
        for (cell, need) in fill.cells.iter().take(7).enumerate() {
            let name = format!("MatText{cell}");

            let (label, color) = super::refine::recipe_label(need, list, items, strings);

            text(screen, &name, &label);

            if let Some(element) = screen.refine_element_mut(&name, ElementType::Static) {
                set_font_color(element, color);
            }
        }
    }
}

pub fn refine_powder_name(
    screen: &mut Screen,
    item: &formats::net::account::Item,
    items: &ItemList,
) {
    text(
        screen,
        "MatText1",
        &items.name(usize::from(item.index)).unwrap_or_default(),
    );

    if let Some(element) = screen.refine_element_mut("MatText1", ElementType::Static) {
        set_font_color(
            element,
            formats::data::item_effect::Effects::new(items, &[]).name_color(item),
        );
    }
}

fn refine_piece_name(screen: &mut Screen, refine: &super::refine::Refine, items: &ItemList) {
    if refine.piece.selected && !refine.piece.preview.is_empty() {
        let item = &refine.piece.preview;

        let effects = formats::data::item_effect::Effects::new(items, &[]);

        let grade = char::from_u32((effects.total_scaled(item, 0x57) + 0x40) as u32).unwrap_or('@');

        let name = items.name(usize::from(item.index)).unwrap_or_default();

        text(
            screen,
            "ItemName",
            &format!("{name} +{} [{grade}]", refine.current),
        );

        if let Some(element) = screen.refine_element_mut("ItemName", ElementType::Static) {
            set_font_color(element, effects.name_color(item));
        }
    }
}

pub fn refine_auto_mirror(screen: &mut Screen, checked: bool) {
    if let Some(button) = screen
        .refine_element_mut("Auto", ElementType::CheckBox)
        .and_then(|e| e.widget.button_mut())
    {
        button.checked = checked;
    }
}

pub fn refine_stop_caption(screen: &mut Screen, checked: bool, strings: &Strings) {
    let key = if checked {
        super::refine::STOP_ON_CAPTION
    } else {
        super::refine::STOP_OFF_CAPTION
    };

    text_any(screen, "Refine", &strings.translate(key));
}

pub fn refine_controls(screen: &mut Screen, checked: bool, show_refine: bool, strings: &Strings) {
    show(screen, "Refine", show_refine);

    refine_stop_caption(screen, checked, strings);

    show(screen, "Stop", !show_refine);

    if !show_refine {
        text_any(
            screen,
            "Stop",
            &strings.translate(super::refine::AUTO_CAPTION),
        );
    }
}

fn refine_material_button_mut(
    screen: &mut Screen,
    cell: usize,
) -> Option<&mut ui::widget::ButtonState> {
    screen
        .refine_element_mut(&format!("Mat{cell}"), ElementType::IconButton)?
        .widget
        .button_mut()
}

pub fn refine_pulse_begin(screen: &mut Screen) {
    for cell in 0..super::refine::CELLS {
        let Some(button) = refine_material_button_mut(screen, cell) else {
            break;
        };

        if button
            .icon_contents
            .is_none_or(|contents| contents.kind == 0)
        {
            break;
        }

        button.pulse_highlight = true;
    }
}

pub fn refine_pulse_halt(screen: &mut Screen) {
    for cell in 0..super::refine::CELLS {
        let Some(button) = refine_material_button_mut(screen, cell) else {
            break;
        };

        if button
            .icon_contents
            .is_none_or(|contents| contents.kind == 0)
        {
            break;
        }

        button.pulse_highlight = false;
    }
}

pub fn refine_pulse_tick(
    screen: &mut Screen,
    stamp_ms: u32,
    now_ms: u32,
    delay: super::refine::AutoDelay,
) {
    if stamp_ms == 0 {
        if let Some(button) = refine_material_button_mut(screen, 0) {
            button.pulse_highlight = false;
        }

        return;
    }

    let elapsed = now_ms.wrapping_sub(stamp_ms);

    let duration = delay.milliseconds();

    let fade = elapsed.min(duration) as f32 / duration as f32;

    for cell in 0..super::refine::CELLS {
        let Some(button) = refine_material_button_mut(screen, cell) else {
            break;
        };

        if button
            .icon_contents
            .is_none_or(|contents| contents.kind == 0)
        {
            break;
        }

        button.pulse_fade = Some(fade);

        button.pulse_highlight =
            elapsed < duration && elapsed <= ui::input::POPULATED_PREFIX_HIGHLIGHT_MS;
    }
}

#[cfg(test)]
mod language_tests {

    use super::*;
    use formats::text::{Codepage, codepage};

    #[test]
    fn malformed_help_prefixes_do_not_split_unicode_codepoints() {
        assert!(color_line("FFFFFą synthetic").is_none());

        assert!(color_line("FFFFFFą synthetic").is_none());

        assert_eq!(color_line("FFFFFF 한글").unwrap().text, "한글");
    }

    #[test]
    fn help_and_quest_text_decode_the_resource_codepage() {
        let cp = Codepage::Windows1250;

        let help = codepage::encode("FFFFFF Zażółć gęślą jaźń\r\n", cp);

        assert_eq!(
            help_lines_with_codepage(&help, cp)[0].text,
            "Zażółć gęślą jaźń"
        );

        let subjects = codepage::encode("1\t0\t00\tŻółć\r\n", cp);

        let mut quests = quest_subjects_with_codepage(&subjects, cp);

        assert_eq!(quests[0].title, "Żółć");

        quest_contents_with_codepage(&help, &mut quests, cp);

        assert_eq!(quests[0].lines[0].text, "Zażółć gęślą jaźń");

        assert_eq!(help_lines(b"FFFFFF Price \x80\r\n")[0].text, "Price €");
    }
}

mod language;

pub(super) use language::{copy_text as copy_language_text, refresh_language};
