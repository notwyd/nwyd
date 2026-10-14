use crate::ui::PanelUi;
use formats::data::item_list::ItemList;
use formats::mesh::appearance;
use formats::net::account::Block;
use formats::net::exp;
use formats::ui::pane::ElementType;
use ui::Strings;

pub const ELEMENTS: [&str; 9] = [
    "Name", "Class", "City", "Level", "Gold", "Exp", "Exp2", "Start", "Start2",
];

pub const ELEMENT_FAMILIES: [&str; 3] = ["Stat%d", "Master%d", "MText%d"];

pub const FAMILY_CELLS: usize = 4;

pub const ELEMENT_TYPE: ElementType = ElementType::Static;

pub const ELEMENT_FLAG: (&str, ElementType) = ("Flag", ElementType::Image);

pub const CLASS_STRDEF: [usize; 4] = [121, 122, 123, 124];

pub const MONSTER_STRDEF: usize = 294;

pub const MONSTER_BODY: i16 = 0x28;

pub const MASTERY_STRDEF_BASE: usize = 242;

pub const TOWN_STRDEF: [usize; 5] = [125, 126, 173, 321, 127];

pub const TOWN_BOUNDS: [(i32, i32, i32, i32); 5] = [
    (2052, 2052, 2171, 2163),
    (2432, 1536, 2675, 1767),
    (2448, 1966, 2476, 2024),
    (3500, 3100, 3700, 3200),
    (1036, 1700, 1075, 1775),
];

pub const CITY_TAG: u8 = 0x1C;

pub fn town_at(x: i32, z: i32) -> usize {
    TOWN_BOUNDS
        .iter()
        .position(|&(min_x, min_z, max_x, max_z)| {
            (min_x..=max_x).contains(&x) && (min_z..=max_z).contains(&z)
        })
        .unwrap_or(TOWN_BOUNDS.len())
}

pub fn separated(value: i64, plus: bool) -> String {
    let digits = value.to_string();

    let (sign, digits) = match digits.strip_prefix('-') {
        Some(rest) => ("-".to_string(), rest.to_string()),
        None => (if plus { "+".to_string() } else { String::new() }, digits),
    };

    if digits.is_empty() || digits.len() > 0x31 {
        return String::new();
    }

    let mut out = sign;

    let n = digits.len();

    for (i, c) in digits.chars().enumerate() {
        out.push(c);

        let remaining = n - i;

        if remaining >= 2 && remaining % 3 == 1 {
            out.push(',');
        }
    }

    out
}

pub fn class_of(body: i16) -> (usize, Option<i16>) {
    let r = body % 10;

    let clamp = |c: i16| match (0..=3).contains(&c) {
        true => c as usize,
        false => 0,
    };

    match r {
        1 => (clamp(body / 10), Some(body)),
        0 | 2..=5 => (2, None),
        _ => (clamp(r - 6), Some(body)),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelInfo {
    pub name: String,
    pub class: String,
    pub city: String,
    pub level: String,
    pub gold: String,
    pub exp: String,
    pub exp2: String,
    pub start: String,
    pub start2: String,
    pub stat: [String; 4],
    pub master: [String; 4],
    pub mtext: [String; 4],
    pub flag: bool,
    pub gm_body_substitute: Option<i16>,
}

pub fn fill(block: &Block, slot: usize, items: &ItemList, strings: &Strings) -> Option<SelInfo> {
    let character = block.slots.get(slot)?;

    let equip = &character.equipment;

    let status = &character.status;

    let body = equip.body().index as i16;

    let (class, substitute) = class_of(body);

    let strdef = |id: usize| strings.translate(&format!("ss_{id}"));

    let class_name = match body < MONSTER_BODY {
        true => strdef(CLASS_STRDEF[class]),
        false => strdef(MONSTER_STRDEF),
    };

    let tag = equip.items[0].effects[2];

    let city = match (tag.id == CITY_TAG, tag.value) {
        (true, value) if value != 0 => value.to_string(),
        _ => String::new(),
    };

    let town = town_at(i32::from(character.x), i32::from(character.z));

    let start = match TOWN_STRDEF.get(town) {
        Some(&id) => strdef(id),
        None => format!("[{}, {}]", character.x, character.z),
    };

    let look = appearance::Look::for_character(equip, items);

    let flag = look
        .as_ref()
        .and_then(|l| l.cape.as_ref())
        .is_some_and(|cape| cape.mesh != GUILD_MARK_HIDDEN);

    Some(SelInfo {
        name: character.name(),
        class: class_name,
        city,
        level: status.shown_level().to_string(),
        gold: separated(i64::from(character.gold), false),
        exp: separated(character.exp as i64, false),
        exp2: exp::to_next(status.level, equip.is_transcendent())
            .map(|v| separated(v, false))
            .unwrap_or_default(),
        start,
        start2: String::new(),
        stat: status.stat.map(|v| v.to_string()),
        master: status.mastery.map(|v| v.to_string()),
        mtext: std::array::from_fn(|row| strdef(MASTERY_STRDEF_BASE + class * 4 + row)),
        flag,
        gm_body_substitute: substitute,
    })
}

pub fn relocalize(info: &mut SelInfo, block: &Block, slot: usize, strings: &Strings) {
    let Some(character) = block.slots.get(slot) else {
        return;
    };

    let body = character.equipment.body().index as i16;

    let (class, _) = class_of(body);

    let strdef = |id: usize| strings.translate(&format!("ss_{id}"));

    info.class = strdef(if body < MONSTER_BODY {
        CLASS_STRDEF[class]
    } else {
        MONSTER_STRDEF
    });

    let town = town_at(i32::from(character.x), i32::from(character.z));

    info.start = match TOWN_STRDEF.get(town) {
        Some(&id) => strdef(id),
        None => format!("[{}, {}]", character.x, character.z),
    };

    info.mtext = std::array::from_fn(|row| strdef(MASTERY_STRDEF_BASE + class * 4 + row));
}

pub const GUILD_MARK_HIDDEN: i16 = 0x13;

pub fn bind(ui: &mut PanelUi) {
    let PanelUi {
        panel, resource, ..
    } = ui;

    for name in ELEMENTS {
        panel.element_or_create(name, ELEMENT_TYPE, resource);
    }

    panel.element_or_create(ELEMENT_FLAG.0, ELEMENT_FLAG.1, resource);

    for i in 0..4 {
        for family in ELEMENT_FAMILIES {
            panel.element_or_create(
                &family.replace("%d", &i.to_string()),
                ELEMENT_TYPE,
                resource,
            );
        }
    }
}

pub fn apply(info: &SelInfo, ui: &mut PanelUi) {
    let PanelUi {
        panel, resource, ..
    } = ui;

    let mut text = |name: &str, value: &str| {
        panel
            .element_or_create(name, ELEMENT_TYPE, resource)
            .set_display(value);
    };

    text(ELEMENTS[0], &info.name);

    text(ELEMENTS[1], &info.class);

    text(ELEMENTS[3], &info.level);

    text(ELEMENTS[4], &info.gold);

    text(ELEMENTS[5], &info.exp);

    text(ELEMENTS[6], &info.exp2);

    for (family, values) in [
        (ELEMENT_FAMILIES[0], &info.stat),
        (ELEMENT_FAMILIES[1], &info.master),
    ] {
        for (cell, value) in values.iter().enumerate().take(FAMILY_CELLS) {
            text(&family.replace("%d", &cell.to_string()), value);
        }
    }

    text(ELEMENTS[2], &info.city);

    text(ELEMENTS[7], &info.start);

    text(ELEMENTS[8], &info.start2);

    for (cell, name) in info.mtext.iter().enumerate().take(FAMILY_CELLS) {
        text(&ELEMENT_FAMILIES[2].replace("%d", &cell.to_string()), name);
    }

    let (name, kind) = ELEMENT_FLAG;

    panel.element_or_create(name, kind, resource).visible = info.flag;
}
