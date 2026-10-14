use formats::data::event_coupon::{self, Coupon};
use formats::data::item_effect::{
    Effects, PetState, ROWS, Wearer, effect, quality, stack_size, tier_bonus,
};
use formats::data::item_help::{self, Help};
use formats::data::item_list::{Entry, ItemList, item_type};
use formats::data::mount::MountData;
use formats::data::skill_data::{self, SkillData};
use formats::mesh::appearance::{is_costume, refine_level, refine_uncompressed};
use formats::net::account::Item;
use ui::Strings;
use ui::tooltip::Row;

pub const PLAIN: u32 = 0xFFFF_FFFF;

pub const UNMET: u32 = 0xFFFF_0000;

pub const NOTICE: u32 = 0xFFFF_FFAA;

pub const SPECIAL: u32 = 0xFFFF_FF00;

pub const NEUTRAL: u32 = quality::NEUTRAL;

pub const BONUS: u32 = 0xFF88_AAFF;

pub const SHUT: u32 = 0xFFAA_FFAA;

pub const PLACEHOLDER: &str = "????????";

pub mod strdef {

    pub const NON_UPGRADABLE: usize = 48;

    pub const PRICE: usize = 57;

    pub const SELLING_PRICE: usize = 58;

    pub const TWO_HANDS: usize = 56;

    pub const DAMAGE_INCREASE: usize = 79;

    pub const DEFENSE: usize = 80;

    pub const MAGIC_ATTACK: usize = 104;

    pub const CLASS_LABEL: usize = 73;

    pub const CLASS_FIRST: usize = 121;

    pub const TAX_RATE: usize = 146;

    pub const SKILL_DELAY: usize = 151;

    pub const LEVEL: usize = 167;

    pub const DEAD: usize = 168;

    pub const INCUBATE: usize = 170;

    pub const CAN_ENTER: usize = 280;

    pub const CANNOT_ENTER: usize = 281;

    pub const DROP_RATE: usize = 195;

    pub const PIERCING: usize = 196;

    pub const EXP: usize = 197;

    pub const ABSORB: usize = 198;

    pub const DAY: usize = 291;

    pub const HOUR_SHORT: usize = 292;

    pub const MINUTE: usize = 293;

    pub const GOLD: usize = 284;

    pub const MILEAGE: usize = 385;

    pub const MONTH: usize = 296;

    pub const YEAR: usize = 297;

    pub const HOUR: usize = 298;

    pub const EXPIRE: usize = 299;

    pub const PET_PVP_DAMAGE: usize = 439;

    pub const PET_PVP_ARMOR: usize = 440;

    pub const REPURCHASE: usize = 486;

    pub const FAME: usize = 487;

    pub const NO_LIMIT: usize = 507;

    pub const APPEARANCE: usize = 531;

    pub const QUALITY_TIER: usize = 536;

    pub const SLOT_FIRST: usize = 544;

    pub const SKILL_RANGE: usize = 49;

    pub const MANA_SPENT: usize = 50;

    pub const SKILL_DAMAGE: usize = 51;

    pub const SKILL_PROPERTY: usize = 52;

    pub const REQUIRED_POINTS: usize = 53;

    pub const REQUIRED_LEVEL: usize = 54;

    pub const SPECIAL_POINT: usize = 55;

    pub const SKILL_POINT: usize = 59;

    pub const MANA_ALL: usize = 231;

    pub const PASSIVE: usize = 139;

    pub const STRENGTH: usize = 100;

    pub const SEALED_ITEM: usize = 554;

    pub const SEALED_COUNT: usize = 555;

    pub const CRYSTALS: usize = 358;
}

pub struct Tables<'a> {
    pub items: &'a ItemList,
    pub mounts: &'a [MountData],
    pub help: &'a [Help],
    pub strings: &'a Strings,
    pub skills: Option<&'a SkillData>,
    pub coupons: &'a [Coupon],
}

impl Tables<'_> {
    fn effects(&self) -> Effects<'_> {
        Effects::new(self.items, self.mounts)
    }

    fn text(&self, record: usize) -> String {
        match self.strings.strdef.as_ref() {
            Some(table) => table.get(record).to_string(),
            None => String::new(),
        }
    }

    fn number(&self, record: usize, value: i64) -> String {
        self.filled(record, &[value])
    }

    fn filled(&self, record: usize, values: &[i64]) -> String {
        let mut text = self.text(record);

        for value in values {
            text = text.replacen("%d", &value.to_string(), 1);
        }

        text.replace("%%", "%")
    }

    fn tenths(&self, record: usize, value: i32) -> String {
        self.filled(record, &[i64::from(value / 10), i64::from(value % 10)])
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Content {
    pub title: Row,
    pub rows: Vec<Row>,
    pub price: Option<Row>,
    pub price_gap: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Context {
    pub price: Price,
    pub zone_clock: Option<i64>,
    pub owner_content: Option<i32>,
    pub learner: Learner,
    pub currency: Currency,
    pub tax_percent: i32,
    pub sealed: Option<Sealed>,
    pub buy_back: [i32; 10],
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Learner {
    pub class: i32,
    pub level: i32,
    pub mastery: [i32; 4],
    pub mana_saving: i32,
    pub skill_points: i32,
    pub live_damage: Option<i32>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Currency {
    #[default]
    Gold,
    Mileage,
    Fame,
}

pub use formats::net::kept::SealedCharacter as Sealed;

const EXPIRY_ONLY: [(i32, i32); 2] = [(3000, 3015), (3050, 3099)];

const COUNTDOWN_ONLY: (i32, i32) = (3900, 3969);

const NOTHING_BETWEEN: (i32, i32) = (4106, 4109);

pub const MARK_OF_BAHAMUT: i32 = 0x15E3;

pub const SPIRITS_SEAL: i32 = 0x0D73;

pub const CONTRACTS_SEAL: i32 = 0x0D74;

pub fn build(item: &Item, tables: &Tables, wearer: &Wearer, context: &Context) -> Content {
    build_for(i32::from(item.index as i16), item, tables, wearer, context)
}

pub fn build_for(
    id: i32,
    item: &Item,
    tables: &Tables,
    wearer: &Wearer,
    context: &Context,
) -> Content {
    let in_range = (id as u32).wrapping_sub(1) <= 0x38A2;

    let entry = in_range.then(|| usize::try_from(id).ok().and_then(|id| tables.items.get(id)));

    let Some(Some(entry)) = entry else {
        return Content {
            title: Row::new(PLACEHOLDER, NEUTRAL),
            rows: Vec::new(),
            price: None,
            price_gap: false,
        };
    };

    match id {
        MARK_OF_BAHAMUT => return mark_of_bahamut(item, &entry, tables),
        SPIRITS_SEAL => return spirits_seal(item, &entry, tables, context),
        CONTRACTS_SEAL => return contracts_seal(item, &entry, tables, context),
        _ => {}
    }

    if skill_data::is_skill_book(id) {
        return skill_book(item, &entry, tables, context);
    }

    if help_count(id, tables) != 0 {
        return help_card(item, &entry, tables);
    }

    general(id, item, &entry, tables, wearer, context)
}

fn general(
    dispatch_id: i32,
    item: &Item,
    entry: &Entry,
    tables: &Tables,
    wearer: &Wearer,
    context: &Context,
) -> Content {
    let id = i32::from(item.index as i16);

    let title = name_row(dispatch_id, item, entry, tables);

    let mut rows = Vec::new();

    if ZONE_ENTRY_IDS.contains(&id) {
        if let Some(clock) = context.zone_clock {
            rows.extend(zone_entry_rows(id, clock, tables));
        }
    } else if EXPIRY_ONLY.iter().any(|&(lo, hi)| (lo..=hi).contains(&id)) {
        rows.push(expiry_row(item, tables));

        rows.extend(damage_only_rows(item, tables));
    } else if (COUNTDOWN_ONLY.0..=COUNTDOWN_ONLY.1).contains(&id) {
        rows.push(countdown_row(item, tables));

        rows.extend(resist_only_rows(item, tables));
    } else if (NOTHING_BETWEEN.0..=NOTHING_BETWEEN.1).contains(&id) {
    } else {
        if takes_an_expiry_line(item, entry, tables.items) {
            rows.push(expiry_row(item, tables));
        }

        rows.extend(base_rows(item, entry, tables, wearer));

        rows.extend(additional_rows(item, entry, tables));

        rows.extend(refine_bonus_row(item, entry, tables));
    }

    rows.extend(pet_pvp_rows(item, entry, tables));

    rows.extend(appearance_row(item, entry, tables));

    if let Some(row) = trade_row(item, tables) {
        rows.push(row);
    }

    rows.extend(tier_rows(item, entry, tables));

    rows.extend(help_rows(id, tables));

    Content {
        title,
        rows,
        price: price_row(item, entry, tables, context),
        price_gap: context.price.needs_price_spacing(),
    }
}

pub const ZONE_ENTRY_IDS: [i32; 8] = [0xCFC, 0xCFD, 0xCFE, 0xD00, 0xD01, 0xD3E, 0xD3F, 0xD40];

pub fn zone_entry_rows(id: i32, seconds: i64, tables: &Tables) -> Vec<Row> {
    const TWENTY: i64 = 20 * 60;

    const THIRTY: i64 = 30 * 60;

    let (target, period) = match id {
        0xCFC | 0xD3E => ((seconds / TWENTY + 1) * TWENTY, 20),
        0xCFD | 0xD3F => (seconds / TWENTY * TWENTY + 25 * 60, 20),
        0xCFE | 0xD40 => (seconds / TWENTY * TWENTY + THIRTY, 20),
        0xD00 | 0xD01 => ((seconds / THIRTY + 1) * THIRTY, 30),
        _ => (0, 20),
    };

    let remaining = target - seconds;

    let minutes = remaining / 60 % period;

    if minutes < period - 4 {
        vec![
            Row::new(format!("{minutes:02} : {:02}", remaining % 60), SHUT),
            Row::new(tables.text(strdef::CANNOT_ENTER), SHUT),
        ]
    } else {
        vec![
            Row::new("0 : 0", NOTICE),
            Row::new(tables.text(strdef::CAN_ENTER), PLAIN),
        ]
    }
}

const PLAIN_NAME_IDS: [(i32, i32); 4] =
    [(0x19B, 0x19B), (400, 0x199), (0x1AC, 0x1B3), (0x2A8, 0x2B3)];

const NOTICE_NAME_IDS: [i32; 5] = [0x19C, 0x19D, 0x102D, 0x1A3, 0x1A4];

const NOTICE_OVERRIDE_IDS: [i32; 3] = [0x2F1, 0x301, 0x6BE];

const OPTION_STONE: i32 = 0x1911;

const COUNTED_TYPE: i16 = 0x37;

const COUNTED_IDS: (i32, i32) = (0x956, 0x973);

fn name_row(id: i32, item: &Item, entry: &Entry, tables: &Tables) -> Row {
    let fx = tables.effects();

    let name = entry.name.clone();

    let refine = refine_level(item, tables.items) as i32;

    let mask = fx.total_scaled(item, effect::EQUIP_MASK);

    let grade = grade_letter(&fx, item);

    if PLAIN_NAME_IDS
        .iter()
        .any(|&(lo, hi)| (lo..=hi).contains(&id))
    {
        return Row::new(name, NEUTRAL);
    }

    if NOTICE_NAME_IDS.contains(&id) || (refine > 0 && mask == 0) {
        return Row::new(name, NOTICE);
    }

    let full = item
        .effects
        .iter()
        .all(|e| e.id != 0 && e.id != b'+' && !(0x73..=0x7E).contains(&e.id));

    if full && mask != 0 && mask != 0x4000 && i32::from(item.index as i16) != OPTION_STONE {
        let text = tables.text(strdef::NON_UPGRADABLE).replacen("%s", &name, 1);

        return Row::new(decorate(text, grade), fx.name_color(item));
    }

    if refine > 0 && mask > 0 {
        let percent = refine_percent(item, tables.items, refine);

        let mut text = if percent < 1 {
            format!("{name} +{refine}")
        } else {
            format!("{name} +{refine} (+{percent}%)")
        };

        let counted = if grade.is_none() {
            entry.item_type == COUNTED_TYPE
        } else {
            (COUNTED_IDS.0..=COUNTED_IDS.1).contains(&id)
        };

        if counted {
            text = format!("{name} +{refine} ({})", stack_count(item, tables.items));
        }

        return Row::new(decorate(text, grade), fx.name_color(item));
    }

    if i32::from(item.index as i16) == OPTION_STONE {
        return option_stone_row(item, entry, tables, mask, grade);
    }

    let mut color = fx.name_color(item);

    let bare = item.effects.iter().all(|e| e.id == 0 || e.id == b';');

    if bare && NOTICE_OVERRIDE_IDS.contains(&id) {
        color = NOTICE;
    }

    Row::new(decorate(name, grade), color)
}

fn option_stone_row(
    item: &Item,
    entry: &Entry,
    tables: &Tables,
    mask: i32,
    grade: Option<char>,
) -> Row {
    let fx = tables.effects();

    let color = fx.name_color(item);

    let name = entry.name.clone();

    let armed = (item.reserved >> 8) as u8 != 0;

    if mask == 0 || !armed {
        return Row::new(name, color);
    }

    let word = match mask {
        0x02 => 0,
        0x04 => 1,
        0x08 => 2,
        0x10 => 3,
        0x20 => 4,
        0x40 | 0xC0 => 5,
        _ => return Row::new(String::new(), color),
    };

    let slot = tables.text(strdef::SLOT_FIRST + word);

    Row::new(format!("{name} ({slot}[{}])", grade.unwrap_or('@')), color)
}

fn decorate(text: String, grade: Option<char>) -> String {
    match grade {
        Some(letter) => format!("{text} [{letter}]"),
        None => text,
    }
}

fn grade_letter(fx: &Effects<'_>, item: &Item) -> Option<char> {
    let value = fx.total_scaled(item, effect::GRADE_LETTER);

    let letter = u8::try_from(value).ok()?.wrapping_add(b'@');

    (letter >= b'A').then_some(char::from(letter))
}

fn refine_percent(item: &Item, items: &ItemList, refine: i32) -> i32 {
    const LADDER: [i32; 9] = [5, 5, 5, 4, 4, 3, 3, 2, 1];

    let tens = refine_tens(item, items);

    let step = usize::try_from(refine)
        .ok()
        .and_then(|k| LADDER.get(k))
        .copied()
        .unwrap_or(0);

    step * tens
}

fn refine_tens(item: &Item, items: &ItemList) -> i32 {
    let id = i32::from(item.index as i16);

    let Some(entry) = usize::try_from(id).ok().and_then(|id| items.get(id)) else {
        return 0;
    };

    if item_type::is_mount_backed(entry.item_type) || (0xC80..0xCE4).contains(&id) {
        return 0;
    }

    let byte = item
        .effects
        .iter()
        .find(|e| e.id == b'+')
        .or_else(|| item.effects.iter().find(|e| (0x73..=0x7E).contains(&e.id)))
        .map_or(0u8, |e| e.value);

    if byte > 0xD2 {
        return 0;
    }

    i32::from(byte / 10)
}

fn stack_count(item: &Item, items: &ItemList) -> i32 {
    ui::icon::stack_count(item, items)
}

fn timer_part(item: &Item, letter: u8) -> i64 {
    item.effects
        .iter()
        .rev()
        .find(|e| e.id == letter)
        .map_or(0, |e| i64::from(e.value))
}

fn expiry_row(item: &Item, tables: &Tables) -> Row {
    let part = |record: usize, value: i64, bias: i64| {
        if value == 0 {
            String::new()
        } else {
            tables.number(record, value + bias)
        }
    };

    let year = part(strdef::YEAR, timer_part(item, b'm'), 2000);

    let month = part(strdef::MONTH, timer_part(item, b'n'), 0);

    let day = part(strdef::DAY, timer_part(item, b'j'), 0);

    let hour = tables.number(strdef::HOUR, 0);

    Row::new(
        format!(
            "{} {year} {month} {day} 0{hour}",
            tables.text(strdef::EXPIRE)
        ),
        NOTICE,
    )
}

fn countdown_row(item: &Item, tables: &Tables) -> Row {
    let part = |record: usize, letter: u8| {
        let value = timer_part(item, letter);

        if value == 0 {
            String::new()
        } else {
            tables.number(record, value)
        }
    };

    let text = format!(
        "{} {} {}",
        part(strdef::DAY, b'j'),
        part(strdef::HOUR_SHORT, b'k'),
        part(strdef::MINUTE, b'l')
    );

    Row::new(text, NOTICE)
}

fn damage_only_rows(item: &Item, tables: &Tables) -> Vec<Row> {
    let fx = tables.effects();

    if fx.total_scaled(item, effect::EQUIP_MASK) == 0x20 {
        return Vec::new();
    }

    ROWS.iter()
        .skip(1)
        .filter(|row| row.id == effect::DAMAGE)
        .map(|row| {
            let label = tables.text(row.strdef as usize);

            Row::new(format!("{label} : {}", fx.template(item, row.id)), PLAIN)
        })
        .collect()
}

fn resist_only_rows(item: &Item, tables: &Tables) -> Vec<Row> {
    let fx = tables.effects();

    ROWS.iter()
        .filter(|row| row.id == 0x5B)
        .filter_map(|row| {
            let value = fx.template(item, row.id);

            (value != 0).then(|| {
                Row::new(
                    format!("{} : {value}%", tables.text(row.strdef as usize)),
                    PLAIN,
                )
            })
        })
        .collect()
}

fn base_rows(item: &Item, entry: &Entry, tables: &Tables, wearer: &Wearer) -> Vec<Row> {
    let fx = tables.effects();

    let id = i32::from(item.index as i16);

    let mask = fx.total_scaled(item, effect::EQUIP_MASK);

    let requirements = fx.requirements(item, wearer);

    let mut out = Vec::new();

    for (index, row) in ROWS.iter().enumerate() {
        if matches!(row.id, effect::PVP_DAMAGE | effect::PVP_ARMOR) {
            continue;
        }

        let value = fx.template(item, row.id);

        let label = tables.text(row.strdef as usize);

        if row.id == effect::PET_PROGRESS
            && value < 1
            && matches!(
                entry.item_type,
                item_type::PET_STAGE1 | item_type::PET_STAGE2
            )
        {
            if item.effects[1].id != 0 {
                out.push(Row::new(tables.text(strdef::DEAD), PLAIN));
            }

            continue;
        }

        if row.id == effect::PET_QUALITY && entry.item_type == item_type::PET_STAGE2 {
            out.push(Row::new(
                format!(
                    "{} : {}",
                    tables.text(strdef::QUALITY_TIER),
                    item.effects[2].value
                ),
                PLAIN,
            ));

            continue;
        }

        if value == 0 {
            continue;
        }

        let color = match requirements.by_row(index) {
            Some(true) => PLAIN,
            Some(false) => UNMET,
            None => PLAIN,
        };

        match (index, row.id) {
            (0, _) => {
                if fx.total_scaled(item, 0x70) == 1 && entry.grade > 0x28 {
                    continue;
                }

                if (0x105E..=0x1068).contains(&id) {
                    continue;
                }

                out.push(Row::new(
                    format!("{label} : {}", class_words(value, tables)),
                    color,
                ));
            }
            (1, _) => {
                let text = format!("{label} : {}", value + 1);

                out.push(Row::new(
                    two_handed(text, item, tables, 1, mask, value, 1),
                    color,
                ));
            }
            (_, effect::DAMAGE) if mask == 0x20 => {}
            (_, effect::CRITICAL | effect::DEFENSE_ALT | effect::DAMAGE_GRADED)
            | (_, effect::MAGIC_ATTACK_GRADED) => {}
            (_, effect::DAMAGE) => out.push(Row::new(pet_pair(label, value, item, entry), PLAIN)),
            (_, effect::MAGIC_ATTACK) => {
                out.push(Row::new(pet_percent(label, value, item, entry), PLAIN));
            }
            (_, effect::DEFENSE) => out.push(Row::new(format!("{label} : {value}"), PLAIN)),
            (_, 0x5B) => out.push(Row::new(format!("{label} : {value}%"), PLAIN)),
            (_, effect::ATTACK_SPEED | 0x1C) => {
                out.push(Row::new(
                    format!("{label} : {}.{}%", value / 10, value % 10),
                    PLAIN,
                ));
            }
            (_, effect::MAX_HP_RATE | effect::MAX_MANA_RATE) => {
                out.push(Row::new(format!("{label} : {value}%"), PLAIN));
            }
            (_, 0x40..=0x42) => out.push(Row::new(label, SPECIAL)),
            (_, effect::PET_MATURITY) => {
                out.push(Row::new(maturity(label, value, item, entry, tables), PLAIN));
            }
            (_, effect::PET_QUALITY) => {
                out.push(Row::new(
                    format!(
                        "{} : {}",
                        tables.text(strdef::QUALITY_TIER),
                        item.effects[2].value
                    ),
                    PLAIN,
                ));
            }
            (_, effect::PET_PROGRESS) => {
                if value > 64_000 {
                    continue;
                }

                let text = if value < 0 {
                    tables.text(strdef::DEAD)
                } else {
                    format!("{label} : {value}")
                };

                out.push(Row::new(text, PLAIN));
            }
            (_, effect::EVASION) => {
                out.push(Row::new(
                    format!("{label} : {}.{}%", value / 10, value % 10),
                    PLAIN,
                ));
            }
            (_, 0x54) if entry.item_type == item_type::PET_EGG && value < 1 => {
                out.push(Row::new(tables.text(strdef::INCUBATE), PLAIN));
            }
            _ => {
                let text = format!("{label} : {value}");

                out.push(Row::new(
                    two_handed(text, item, tables, index, mask, value, 0),
                    color,
                ));
            }
        }
    }

    out
}

fn class_words(mask: i32, tables: &Tables) -> String {
    if mask == 0xFF {
        return tables.text(strdef::NO_LIMIT);
    }

    (0..4)
        .filter(|bit| mask & (1 << bit) != 0)
        .map(|bit| tables.text(strdef::CLASS_FIRST + bit as usize))
        .collect()
}

fn two_handed(
    text: String,
    item: &Item,
    tables: &Tables,
    row: usize,
    mask: i32,
    value: i32,
    bump: i32,
) -> String {
    if mask != 0xC0 || !(1..=5).contains(&row) {
        return text;
    }

    let extra = tables.effects().total_scaled(item, 0x15);

    if extra % 10 <= 1 {
        return text;
    }

    let factor = match extra / 10 {
        0 => 130,
        6 => 150,
        _ => 100,
    };

    let scaled = factor * value / 100 + bump;

    format!(
        "{text} {}",
        tables.number(strdef::TWO_HANDS, i64::from(scaled))
    )
}

fn pet_pair(label: String, value: i32, item: &Item, entry: &Entry) -> String {
    let bonus = item.effects[2].value;

    if entry.item_type == item_type::PET_STAGE2 && bonus != 0 {
        return format!(
            "{label} : {value}({})",
            (i32::from(bonus) + 100) * value / 100
        );
    }

    format!("{label} : {value}")
}

fn pet_percent(label: String, value: i32, item: &Item, entry: &Entry) -> String {
    let bonus = item.effects[2].value;

    if entry.item_type == item_type::PET_STAGE2 && bonus != 0 {
        return format!(
            "{label} : {value}%({}%)",
            (i32::from(bonus) + 100) * value / 100
        );
    }

    format!("{label} : {value}%")
}

fn maturity(label: String, value: i32, item: &Item, entry: &Entry, tables: &Tables) -> String {
    let id = i32::from(item.index as i16);

    if entry.item_type == item_type::PET_STAGE2 || (0xB90..3000).contains(&id) {
        return format!("{} : {value}", tables.text(strdef::LEVEL));
    }

    let cap = usize::try_from(entry.mount_index)
        .ok()
        .and_then(|m| tables.mounts.get(m))
        .map_or(100, |row| row.max_maturity + 1);

    format!("{label} : {value}/{cap}")
}

fn needs_alias(id: u8, mask: i32) -> bool {
    matches!(
        id,
        effect::CRITICAL | effect::DEFENSE_ALT | effect::PVP_DAMAGE | effect::PVP_ARMOR
    ) || (mask == 0x20 && id == effect::DAMAGE)
}

fn additional_rows(item: &Item, _entry: &Entry, tables: &Tables) -> Vec<Row> {
    let fx = tables.effects();

    let id = i32::from(item.index as i16);

    let mask = fx.total_scaled(item, effect::EQUIP_MASK);

    let mut out = Vec::new();

    for row in ROWS {
        let asked = match row.id {
            effect::MAX_HP_RATE => effect::MAX_HP_RATE_ALT,
            effect::MAX_MANA_RATE => effect::MAX_MANA_RATE_ALT,
            other => other,
        };

        let (scaled, raw) = if needs_alias(asked, mask) {
            (fx.total_scaled(item, asked), fx.total(item, asked))
        } else {
            (fx.slot_scaled(item, asked), fx.slot(item, asked))
        };

        if scaled == 0 {
            continue;
        }

        if asked == effect::DAMAGE_BOOT && mask == 0x20 && id != OPTION_STONE {
            continue;
        }

        if asked == effect::DAMAGE_GRADED && !matches!(mask, 0x40 | 0xC0) && id != OPTION_STONE {
            continue;
        }

        if asked == 0x3F {
            continue;
        }

        let label = tables.text(row.strdef as usize);

        let text = match asked {
            effect::CRITICAL
            | effect::ATTACK_SPEED
            | 0x1C
            | effect::PVP_DAMAGE
            | effect::PVP_ARMOR => tenths(&label, raw, scaled),
            effect::EVASION => {
                format!(
                    "{label} : {}.{}% ({}.{}%)",
                    raw / 10,
                    raw % 10,
                    scaled / 10,
                    scaled % 10
                )
            }
            effect::MAGIC_ATTACK
            | effect::MAX_HP_RATE_ALT
            | effect::MAX_MANA_RATE_ALT
            | effect::MAGIC_ATTACK_GRADED => percent(&label, raw, scaled),
            _ => plain(&label, raw, scaled),
        };

        let color = if matches!(asked, effect::PVP_DAMAGE | effect::PVP_ARMOR) {
            SPECIAL
        } else {
            formats::data::item_effect::quality_color(mask as u32, asked, raw)
        };

        out.push(Row::new(text, color));
    }

    out
}

fn tenths(label: &str, raw: i32, scaled: i32) -> String {
    if raw == scaled {
        format!("{label} : {}.{}%", scaled / 10, scaled % 10)
    } else {
        format!(
            "{label} : {}.{}% ({}.{}%)",
            raw / 10,
            raw % 10,
            scaled / 10,
            scaled % 10
        )
    }
}

fn percent(label: &str, raw: i32, scaled: i32) -> String {
    if raw == scaled {
        format!("{label} : {scaled}%")
    } else {
        format!("{label} : {raw}% ({scaled}%)")
    }
}

fn plain(label: &str, raw: i32, scaled: i32) -> String {
    if raw == scaled {
        format!("{label} : {scaled}")
    } else {
        format!("{label} : {raw} ({scaled})")
    }
}

fn refine_bonus_row(item: &Item, entry: &Entry, tables: &Tables) -> Option<Row> {
    if refine_level(item, tables.items) <= 8 {
        return None;
    }

    let text = match tables.effects().total_scaled(item, effect::EQUIP_MASK) {
        0x04 | 0x08 | 0x80 => format!("{} : 25", tables.text(strdef::DEFENSE)),
        0x10 => format!("{} : 1", tables.text(strdef::SKILL_DELAY)),
        0x40 | 0xC0 if matches!(entry.grade, 0x2C | 0x2F) => {
            format!("{} : 16%", tables.text(strdef::MAGIC_ATTACK))
        }
        0x40 | 0xC0 => format!("{} : 40", tables.text(strdef::DAMAGE_INCREASE)),
        _ => return None,
    };

    Some(Row::new(text, BONUS))
}

fn pet_pvp_rows(item: &Item, entry: &Entry, tables: &Tables) -> Vec<Row> {
    if entry.item_type != item_type::PET_STAGE2 {
        return Vec::new();
    }

    let fx = tables.effects();

    let mut damage = fx.total_scaled(item, effect::PVP_DAMAGE);

    let mut armor = fx.total_scaled(item, effect::PVP_ARMOR);

    let level = i32::from(PetState::of(item).maturity);

    if level > 120 {
        armor += level - 120;

        damage += (level - 120) * 2;
    }

    let mut out = Vec::new();

    if damage > 0 {
        out.push(Row::new(
            tables.tenths(strdef::PET_PVP_DAMAGE, damage),
            SPECIAL,
        ));
    }

    if armor > 0 {
        out.push(Row::new(
            tables.tenths(strdef::PET_PVP_ARMOR, armor),
            SPECIAL,
        ));
    }

    out
}

fn appearance_row(item: &Item, entry: &Entry, tables: &Tables) -> Option<Row> {
    let mask = tables.effects().total_scaled(item, effect::EQUIP_MASK);

    if mask == 0 || item.appearance == 0 {
        return None;
    }

    let wearable = matches!(mask, 0x02 | 0x04 | 0x08 | 0x10 | 0x20 | 0x40 | 0x80 | 0xC0);

    if !wearable && entry.item_type != item_type::PET_STAGE2 {
        return None;
    }

    let looks_like = usize::try_from(i32::from(item.appearance as i16))
        .ok()
        .and_then(|id| tables.items.get(id))
        .map_or(String::new(), |e| e.name);

    let text = tables
        .text(strdef::APPEARANCE)
        .replacen("%s", &looks_like, 1);

    Some(Row::new(text, PLAIN))
}

fn tier_rows(item: &Item, entry: &Entry, tables: &Tables) -> Vec<Row> {
    let bonus = tier_bonus(
        entry,
        refine_level(item, tables.items),
        refine_uncompressed(item, tables.items),
    );

    let mut out = Vec::new();

    if bonus.piercing != 0 {
        let text = format!("{} : {}", tables.text(strdef::PIERCING), bonus.piercing);

        out.push(Row::new(text, BONUS));
    }

    if bonus.absorb != 0 {
        let text = format!("{} : {}", tables.text(strdef::ABSORB), bonus.absorb);

        out.push(Row::new(text, BONUS));
    }

    if bonus.drop_rate != 0 {
        out.push(Row::new(
            tables.number(strdef::DROP_RATE, i64::from(bonus.drop_rate)),
            BONUS,
        ));
    }

    if bonus.exp != 0 {
        out.push(Row::new(
            tables.number(strdef::EXP, i64::from(bonus.exp)),
            BONUS,
        ));
    }

    out
}

fn trade_row(item: &Item, tables: &Tables) -> Option<Row> {
    let record = tables.effects().trade(item).strdef()?;

    Some(Row::new(tables.text(record as usize), UNMET))
}

fn help_rows(id: i32, tables: &Tables) -> Vec<Row> {
    let Some(help) = usize::try_from(id).ok().and_then(|id| tables.help.get(id)) else {
        return Vec::new();
    };

    if help.color[0] == 0 {
        return Vec::new();
    }

    let Some(first) = help.line.iter().position(|line| !line.is_empty()) else {
        return Vec::new();
    };

    let mut out = Vec::new();

    for index in first..help.line.len() {
        if !help_line_shown(help, index) {
            continue;
        }

        out.push(Row::new(help.line[index].clone(), help_color(help, index)));
    }

    out
}

fn help_line_shown(help: &Help, index: usize) -> bool {
    index < 2
        || ((!help.line[index - 1].is_empty() || !help.line[index].is_empty())
            && (index != 8 || !help.line[8].is_empty()))
}

fn help_color(help: &Help, index: usize) -> u32 {
    help.argb(index)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Price {
    #[default]
    None,
    Blocked,
    Gold(i64),
    Selling,
    SkillPoint,
    Buying {
        tax_percent: i32,
    },
    Repurchase {
        tax_percent: i32,
    },
    Fame,
    Mileage,
    Coupon,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Shop {
    pub open: bool,
    pub owner: Option<i32>,
    pub asking: i64,
    pub in_game: bool,
    pub currency: Currency,
    pub repurchase: bool,
    pub coupon: bool,
    pub tax_percent: i32,
    pub buy_back: [i32; 10],
}

pub const OWNER_INVENTORY: i32 = 0x17;

pub const OWNER_ITEM_SHOP: i32 = 0x1B;

pub const OWNER_AUTO_TRADE: i32 = 0x1C;

impl Price {
    pub fn choose(shop: &Shop) -> Price {
        if shop.owner == Some(OWNER_AUTO_TRADE) {
            return Price::Gold(shop.asking);
        }

        if !shop.open {
            return Price::None;
        }

        match shop.owner {
            Some(OWNER_INVENTORY) => Price::Selling,
            Some(OWNER_ITEM_SHOP) => {
                if !shop.in_game {
                    return Price::Blocked;
                }

                match shop.currency {
                    Currency::Fame => Price::Fame,
                    Currency::Mileage => Price::Mileage,
                    Currency::Gold if shop.repurchase => Price::Repurchase {
                        tax_percent: shop.tax_percent,
                    },
                    Currency::Gold if shop.coupon => Price::Coupon,
                    Currency::Gold => Price::Buying {
                        tax_percent: shop.tax_percent,
                    },
                }
            }
            _ => Price::SkillPoint,
        }
    }

    pub fn needs_price_spacing(self) -> bool {
        self != Price::None
    }
}

const PRICELESS: [i32; 2] = [4998, 4999];

fn price_row(item: &Item, entry: &Entry, tables: &Tables, context: &Context) -> Option<Row> {
    let id = i32::from(item.index as i16);

    let price = context.price;

    let text = match price {
        Price::None | Price::Blocked => return None,
        Price::Gold(amount) if amount >= 2_000_000_000 => {
            let amount = amount - 2_000_000_000;

            if (1..1_000_000).contains(&amount) {
                tables.number(1839, amount)
            } else {
                tables.number(1838, 0)
            }
        }
        Price::Gold(amount) => gold(amount, tables),
        Price::Selling => {
            tables.number(strdef::SELLING_PRICE, i64::from(selling_price(item, entry)))
        }
        Price::SkillPoint => {
            tables.number(strdef::SKILL_POINT, i64::from(scroll_points(id, tables)?))
        }
        Price::Fame => tables.number(strdef::FAME, i64::from(entry.price)),
        Price::Mileage => tables.number(strdef::MILEAGE, i64::from(entry.price)),
        Price::Buying { tax_percent } => {
            let total = buying_price(item, entry, tax_percent);

            let line = tables.number(strdef::PRICE, i64::from(total));

            format!("{line} ({}:{tax_percent}%)", tables.text(strdef::TAX_RATE))
        }
        Price::Repurchase { tax_percent } => match context.buy_back.contains(&id) {
            true => {
                let value = repurchase_price(item, entry, tax_percent);

                tables.number(strdef::REPURCHASE, i64::from(value))
            }
            false => String::new(),
        },
        Price::Coupon => coupon_line(id, tables),
    };

    let blanked = PRICELESS.contains(&id)
        && matches!(
            price,
            Price::Buying { .. } | Price::Repurchase { .. } | Price::Coupon
        );

    Some(Row::new(if blanked { String::new() } else { text }, PLAIN))
}

fn scroll_points(id: i32, tables: &Tables) -> Option<i32> {
    let row = skill_data::scroll_skill(id)?;

    Some(
        tables
            .skills
            .and_then(|t| t.get(row))
            .map_or(0, |s| s.skill_points()),
    )
}

fn coupon_line(id: i32, tables: &Tables) -> String {
    let Some((count, currency)) = event_coupon::lookup(tables.coupons, id) else {
        return String::new();
    };

    let name = usize::try_from(currency)
        .ok()
        .and_then(|c| tables.items.get(c))
        .map(|e| e.name.clone())
        .unwrap_or_default();

    format!("{name} : {count}")
}

fn gold(amount: i64, tables: &Tables) -> String {
    let unit = tables.text(strdef::GOLD);

    if amount >= 1_000_000_000 {
        let b = amount / 1_000_000_000;

        let m = (amount % 1_000_000_000) / 1_000_000;

        let k = (amount % 1_000_000) / 1_000;

        format!("{b},{m:03},{k:03},{:03} {unit}", amount % 1_000)
    } else if amount >= 1_000_000 {
        let m = amount / 1_000_000;

        let k = (amount % 1_000_000) / 1_000;

        format!("{m},{k:03},{:03} {unit}", amount % 1_000)
    } else if amount >= 1_000 {
        format!("{},{:03} {unit}", amount / 1_000, amount % 1_000)
    } else {
        format!("{amount} {unit}")
    }
}

pub fn quartered_price(price: i32) -> i32 {
    let value = (f64::from(price) * 0.25) as i32;

    if value >= 5001 {
        if value <= 10_000 {
            value * 2 / 3
        } else {
            value / 2
        }
    } else if value > 10_000 {
        value / 2
    } else {
        value
    }
}

pub fn selling_price(item: &Item, entry: &Entry) -> i32 {
    let id = i32::from(item.index as i16);

    let mut value = entry.price;

    if matches!(id, 0x19C | 0x19D) {
        if value != 0 {
            value = value / 4 * 8 / 9;
        }
    } else {
        value = quartered_price(value);
    }

    if entry.max_stack != 0 {
        value *= stack_size(item);
    }

    value
}

pub fn buying_price(item: &Item, entry: &Entry, tax_percent: i32) -> i32 {
    let total = stack_size(item) * entry.price;

    let tax = (f64::from(total) * (f64::from(tax_percent) / 100.0)) as i32;

    total + tax
}

pub fn repurchase_price(item: &Item, entry: &Entry, tax_percent: i32) -> i32 {
    let price = entry.price;

    let mut value = if price < 100_000 {
        (tax_percent + 100) * price / 100
    } else {
        (tax_percent + 100) * (price / 100)
    };

    let id = i32::from(item.index as i16);

    if matches!(id, 0x19C | 0x19D | 0x1A3 | 0x1A4) {
        value *= stack_size(item).max(1);
    }

    value
}

pub fn takes_an_expiry_line(item: &Item, entry: &Entry, items: &ItemList) -> bool {
    let id = i32::from(item.index as i16);

    if entry.item_type == item_type::MOUNT && id != 0xF99 && id != 0xF9A {
        return true;
    }

    is_costume(id, items) || (0x189D..=0x1900).contains(&id)
}

fn help_count(id: i32, tables: &Tables) -> u16 {
    usize::try_from(id)
        .ok()
        .and_then(|id| tables.help.get(id))
        .map_or(0, |h| h.count)
}

pub fn help_card(item: &Item, entry: &Entry, tables: &Tables) -> Content {
    let mut rows = Vec::new();

    if let Some(row) = trade_row(item, tables) {
        rows.push(row);
    }

    rows.extend(help_card_rows(item, tables));

    Content {
        title: Row::new(entry.name.clone(), NOTICE),
        rows,
        price: None,
        price_gap: false,
    }
}

fn help_card_rows(item: &Item, tables: &Tables) -> Vec<Row> {
    let id = i32::from(item.index as i16);

    let Some(help) = usize::try_from(id).ok().and_then(|id| tables.help.get(id)) else {
        return Vec::new();
    };

    if help.color[0] == 0 || !matches!(help.count, 0..=2) {
        return Vec::new();
    }

    let Some(first) = help.line.iter().position(|line| !line.is_empty()) else {
        return Vec::new();
    };

    let mut out = Vec::new();

    for index in first..help.line.len() {
        if !help_line_shown(help, index) {
            continue;
        }

        let line = &help.line[index];

        let substitute = help.count != 0 && index < 7 && line.contains("%d");

        let text = match substitute {
            false => line.clone(),
            true => match (help.count, help_substitution(item, index)) {
                (1, 0) => String::new(),
                (_, value) => line
                    .replacen("%d", &value.to_string(), 1)
                    .replace("%%", "%"),
            },
        };

        if text.is_empty() {
            continue;
        }

        out.push(Row::new(text, help_color(help, index)));
    }

    out
}

fn help_substitution(item: &Item, index: usize) -> i32 {
    match index {
        0 => i32::from(item.effects[0].id),
        1 => i32::from(item.effects[0].value),
        2 => i32::from(item.effects[1].id),
        3 => i32::from(item.effects[1].value),
        4 => i32::from(item.effects[2].id),
        5 => i32::from(item.effects[2].value),
        _ => i32::from(item.appearance as i16),
    }
}

pub fn pet_card(item: &Item, tables: &Tables) -> Content {
    let name = usize::try_from(i32::from(item.index as i16))
        .ok()
        .and_then(|id| tables.items.get(id))
        .map_or(String::new(), |e| e.name);

    let title = Row::new(name, tables.effects().name_color(item));

    let level = u32::from(item.effects[1].id);

    let text = format!("{} : {}", tables.text(strdef::LEVEL), level);

    Content {
        title,
        rows: vec![Row::new(text, PLAIN)],
        price: None,
        price_gap: false,
    }
}

pub fn mark_of_bahamut(item: &Item, entry: &Entry, tables: &Tables) -> Content {
    let mut rows = vec![Row::comment("")];

    if let Some(row) = trade_row(item, tables) {
        rows.push(row);
    }

    Content {
        title: refined_name(item, entry, tables),
        rows,
        price: None,
        price_gap: false,
    }
}

fn refined_name(item: &Item, entry: &Entry, tables: &Tables) -> Row {
    let refine = refine_level(item, tables.items);

    let text = match refine {
        0 => entry.name.clone(),
        n => format!("{}+{n}", entry.name),
    };

    Row::new(text, NOTICE)
}

pub fn contracts_seal(item: &Item, entry: &Entry, tables: &Tables, context: &Context) -> Content {
    let mut rows = help_rows(i32::from(item.index as i16), tables);

    rows.push(Row::comment(""));

    let held = sealed_id(item);

    if held != 0 {
        let template = usize::try_from(held)
            .ok()
            .and_then(|id| tables.items.get(id));

        rows.push(Row::new(
            template.as_ref().map_or(String::new(), |e| e.name.clone()),
            HELD,
        ));

        let price = i64::from(template.map_or(0, |e| e.price));

        rows.push(Row::new(sealed_price(held, price, context, tables), PLAIN));
    }

    if let Some(row) = trade_row(item, tables) {
        rows.push(row);
    }

    Content {
        title: refined_name(item, entry, tables),
        rows,
        price: None,
        price_gap: false,
    }
}

fn sealed_id(item: &Item) -> i32 {
    i32::from(item.effects[0].value) << 8 | i32::from(item.effects[1].value)
}

const FLAT_TAX_IDS: [i32; 2] = [0xFAA, 0xFAB];

const FLAT_TAX_RANGE: (i32, i32) = (0xFBA, 0xFBD);

fn sealed_price(held: i32, price: i64, context: &Context, tables: &Tables) -> String {
    let flat =
        FLAT_TAX_IDS.contains(&held) || (FLAT_TAX_RANGE.0..=FLAT_TAX_RANGE.1).contains(&held);

    if flat {
        return taxed(tables, price + price / 100, 1);
    }

    match context.currency {
        Currency::Fame => tables.number(strdef::FAME, price),
        Currency::Mileage => tables.number(strdef::MILEAGE, price),
        Currency::Gold => {
            let tax = context.tax_percent;

            #[allow(clippy::cast_possible_truncation, clippy::cast_precision_loss)]
            let scaled = (price as f32 * (tax as f32 / 100.0)) as i64;

            taxed(tables, scaled + price, tax)
        }
    }
}

fn taxed(tables: &Tables, amount: i64, percent: i32) -> String {
    format!(
        "{} ({}:{percent}%)",
        tables.number(strdef::PRICE, amount),
        tables.text(strdef::TAX_RATE)
    )
}

pub fn spirits_seal(item: &Item, entry: &Entry, tables: &Tables, context: &Context) -> Content {
    let title = refined_name(item, entry, tables);

    let mut rows = match (item.effects[0].id == SEALED_MARKER, context.sealed) {
        (true, Some(sealed)) => sealed_rows(&sealed, tables),
        (true, None) => Vec::new(),
        (false, _) => help_rows(i32::from(item.index as i16), tables),
    };

    rows.push(Row::comment(""));

    if let Some(row) = trade_row(item, tables) {
        rows.push(row);
    }

    Content {
        title,
        rows,
        price: None,
        price_gap: false,
    }
}

pub const SEALED_MARKER: u8 = b';';

pub const HELD: u32 = 0xFFFF_BBFF;

pub const SEALED_CLASS: u32 = 0xFFAA_FFFF;

fn sealed_rows(sealed: &Sealed, tables: &Tables) -> Vec<Row> {
    let class = tables.text(strdef::CLASS_FIRST + class_of(sealed.body));

    let label = tables.text(strdef::CLASS_LABEL);

    let text = match sealed.body % 10 {
        n if n >= 6 && sealed.body / 10 >= 0 => {
            let sub = tables.text(strdef::CLASS_FIRST + (n as usize - 6));

            format!("{label} : {sub}[{class}]")
        }
        n if n >= 6 => {
            format!(
                "{label} : {}",
                tables.text(strdef::CLASS_FIRST + (n as usize - 6))
            )
        }
        _ => format!("{label} : {class}"),
    };

    let mut out = vec![Row::new(text, SEALED_CLASS)];

    let level = tables.text(strdef::LEVEL);

    out.push(Row::new(
        match sealed.max_level {
            -1 => format!("{level} : {}", sealed.level + 1),
            cap => format!("{level} : {} / {}", sealed.level + 1, cap + 1),
        },
        PLAIN,
    ));

    for (n, value) in sealed.stat.iter().enumerate() {
        let label = tables.text(strdef::STRENGTH + n);

        out.push(Row::new(format!("{label} : {value}"), PLAIN));
    }

    let held = tables.text(strdef::SEALED_ITEM);

    let count = tables.text(strdef::SEALED_COUNT);

    match usize::try_from(sealed.held_item).ok().filter(|&id| id != 0) {
        None => {
            out.push(Row::new(format!("{held} : -"), PLAIN));

            out.push(Row::new(format!("{count} : -"), PLAIN));
        }
        Some(id) => {
            let name = tables.items.get(id).map_or(String::new(), |e| e.name);

            out.push(Row::new(format!("{held} : {name}"), PLAIN));

            out.push(Row::new(format!("{count} : {}", sealed.held_count), PLAIN));
        }
    }

    let flags = sealed.crystals;

    let mut crystals = u32::from(flags & 1 != 0);

    for (n, mask) in [(2u32, 0x11u32), (3, 0x111), (4, 0x1111)] {
        if flags & mask != 0 {
            crystals = n;
        }
    }

    out.push(Row::new(
        format!("{} : {crystals}", tables.text(strdef::CRYSTALS)),
        PLAIN,
    ));

    out.extend(sealed_skill_rows(sealed, tables));

    out
}

fn sealed_skill_rows(sealed: &Sealed, tables: &Tables) -> Vec<Row> {
    let name = |slot: usize| -> String {
        let n = sealed.skills[slot];

        match (1..=0x6D).contains(&n) {
            false => String::new(),
            true => usize::try_from(SKILL_NAME_BASE + n)
                .ok()
                .and_then(|id| tables.items.get(id))
                .map_or(String::new(), |e| e.name),
        }
    };

    [[0, 1, 2], [3, 4, 5], [6, 7, 8]]
        .into_iter()
        .map(|group| format!("{} {} {}", name(group[0]), name(group[1]), name(group[2])))
        .filter(|text| text.len() > 2)
        .map(|text| Row::new(text, HELD))
        .collect()
}

const SKILL_NAME_BASE: i32 = 5000;

fn class_of(body: i32) -> usize {
    usize::try_from(body / 10).unwrap_or(0).min(3)
}

pub fn skill_book(item: &Item, entry: &Entry, tables: &Tables, context: &Context) -> Content {
    let title = Row::new(entry.name.clone(), tables.effects().name_color(item));

    let id = i32::from(item.index as i16);

    let mut rows = Vec::new();

    if skill_data::is_skill_book(id) {
        rows.extend(skill_rows(item, id, tables, context));
    }

    rows.extend(skill_book_pvp_rows(item, entry, tables));

    rows.extend(skill_book_tier_rows(item, entry, tables));

    rows.extend(skill_book_help_rows(id, tables));

    if let Some(row) = skill_point_row(entry, tables, context) {
        rows.push(row);
    }

    Content {
        title,
        rows,
        price: None,
        price_gap: false,
    }
}

fn skill_rows(item: &Item, id: i32, tables: &Tables, context: &Context) -> Vec<Row> {
    let index = skill_data::skill_index(id);

    let skill = tables
        .skills
        .and_then(|t| usize::try_from(index).ok().and_then(|i| t.get(i)))
        .copied()
        .unwrap_or_default();

    let mut out = vec![Row::new(
        tables.number(strdef::SKILL_RANGE, i64::from(skill.range())),
        PLAIN,
    )];

    out.push(Row::new(
        match index {
            0x16 | 0x1F => tables.text(strdef::MANA_ALL),
            _ => {
                let base = tables.number(strdef::MANA_SPENT, i64::from(skill.cost));

                match skill_mastery(index, context) {
                    Some(mastery) => {
                        let live = skill.cost_at(mastery, context.learner.mana_saving);

                        match live {
                            0 => base,
                            n => format!("{base} ( {n} )"),
                        }
                    }
                    None => base,
                }
            }
        },
        PLAIN,
    ));

    let damage = tables.number(strdef::SKILL_DAMAGE, i64::from(skill.damage()));

    out.push(Row::new(
        match context.learner.live_damage.filter(|&n| n != 0) {
            Some(n) => format!("{damage} ( {n} )"),
            None => damage,
        },
        PLAIN,
    ));

    let property = match index == skill_data::POISON_SKILL {
        true => skill_data::PROPERTIES.len() - 1,
        false => usize::try_from(skill.property())
            .unwrap_or(0)
            .min(skill_data::PROPERTIES.len() - 1),
    };

    let text = tables.text(strdef::SKILL_PROPERTY).replacen(
        "%s",
        &tables.text(skill_data::PROPERTIES[property]),
        1,
    );

    out.push(Row::new(text, PLAIN));

    out.extend(required_point_rows(item, id, tables, context));

    out.extend(learn_rows(item, id, tables, context));

    if skill.is_passive() {
        out.push(Row::new(tables.text(strdef::PASSIVE), SHUT));
    }

    out
}

fn skill_mastery(index: i32, context: &Context) -> Option<i32> {
    if !(0..0x60).contains(&index) {
        return None;
    }

    let which = usize::try_from(index % 0x18 / 8 + 1).ok()?;

    context.learner.mastery.get(which).copied()
}

fn required_point_rows(item: &Item, id: i32, tables: &Tables, context: &Context) -> Vec<Row> {
    let fx = tables.effects();

    let group = skill_data::skill_group(id).map(|g| tables.text(skill_data::GROUPS[g]));

    let Some(group) = group else {
        return Vec::new();
    };

    let mut out = Vec::new();

    for (n, row) in ROWS[3..6].iter().enumerate() {
        let value = fx.total_scaled(item, row.id);

        if value <= 0 {
            continue;
        }

        let text = tables
            .text(strdef::REQUIRED_POINTS)
            .replacen("%s", &group, 1)
            .replacen("%d", &value.to_string(), 1);

        let met = value <= context.learner.mastery.get(n + 1).copied().unwrap_or(0);

        out.push(Row::new(text, if met { PLAIN } else { UNMET }));
    }

    out
}

fn learn_rows(item: &Item, id: i32, tables: &Tables, context: &Context) -> Vec<Row> {
    let fx = tables.effects();

    let mut out = Vec::new();

    let mask = fx.total_scaled(item, ROWS[0].id);

    if mask >= 0 && mask != 0xFF {
        let mut text = format!("{} : ", tables.text(ROWS[0].strdef as usize));

        let mut met = true;

        for bit in 0..4 {
            if mask >> bit & 1 == 0 {
                continue;
            }

            met &= context.learner.class == bit;

            text.push_str(&tables.text(strdef::CLASS_FIRST + bit as usize));
        }

        out.push(Row::new(text, if met { PLAIN } else { UNMET }));
    }

    if context.owner_content != Some(LEARN_SKILL_CONTENT) {
        return out;
    }

    let level = fx.total_scaled(item, ROWS[1].id);

    if level >= 0 {
        let text = tables.number(strdef::REQUIRED_LEVEL, i64::from(level) + 1);

        let met = level <= context.learner.level;

        out.push(Row::new(text, if met { PLAIN } else { UNMET }));
    }

    let points = fx.total_scaled(item, ROWS[2].id);

    if points != 0
        && let Some(group) = skill_data::skill_group_by_eight(id)
    {
        let text = tables
            .text(strdef::SPECIAL_POINT)
            .replacen("%s", &tables.text(skill_data::GROUPS[group]), 1)
            .replacen("%d", &points.to_string(), 1);

        out.push(Row::new(text, PLAIN));
    }

    out
}

pub const LEARN_SKILL_CONTENT: i32 = 0x16;

fn skill_book_pvp_rows(item: &Item, entry: &Entry, tables: &Tables) -> Vec<Row> {
    let fx = tables.effects();

    let mut damage = fx.total_scaled(item, effect::PVP_DAMAGE);

    let mut armor = fx.total_scaled(item, effect::PVP_ARMOR);

    let level = i32::from(PetState::of(item).maturity);

    if entry.item_type == item_type::PET_STAGE2 && level > 120 {
        armor += level - 120;

        damage += (level - 120) * 2;
    }

    let mut out = Vec::new();

    if damage > 0 {
        out.push(Row::new(
            tables.tenths(strdef::PET_PVP_DAMAGE, damage),
            SPECIAL,
        ));
    }

    if armor > 0 {
        out.push(Row::new(
            tables.tenths(strdef::PET_PVP_ARMOR, armor),
            SPECIAL,
        ));
    }

    out
}

fn skill_book_tier_rows(item: &Item, entry: &Entry, tables: &Tables) -> Vec<Row> {
    let tier = i32::from(entry.tier);

    let refine = i32::try_from(refine_level(item, tables.items)).unwrap_or(0);

    let pays =
        (5..=10).contains(&tier) || (0x14..=0x16).contains(&tier) || (0x18..=0x1A).contains(&tier);

    if refine > 9 {
        let step = if pays && (5..=8).contains(&tier) {
            2
        } else {
            1
        };

        let over = (refine - 9).max(0);

        let text = match uncompressed_selector(item, tables.items) {
            0 => Some(tables.number(strdef::DROP_RATE, i64::from(step * 8))),
            1 => Some(labelled(tables, strdef::PIERCING, over * step * 40)),
            2 => Some(tables.number(strdef::EXP, i64::from(step * 2))),
            _ => Some(labelled(tables, strdef::ABSORB, over * step * 40)),
        };

        return text.into_iter().map(|t| Row::new(t, BONUS)).collect();
    }

    if !pays {
        return Vec::new();
    }

    let base = match tier {
        t if t >= 0x1A => 40,
        t if t >= 0x17 => 30,
        t if t >= 0x14 => 15,
        _ => 10,
    };

    let scaled = if refine > 8 { base } else { base * refine / 10 };

    let amount = scaled + base;

    match tier {
        5 => vec![Row::new(tables.number(strdef::DROP_RATE, 8), BONUS)],
        6 => vec![Row::new(labelled(tables, strdef::PIERCING, 40), BONUS)],
        7 => vec![Row::new(tables.number(strdef::EXP, 2), BONUS)],
        8 => vec![Row::new(labelled(tables, strdef::ABSORB, 40), BONUS)],
        9 | 0x14 | 0x18 => {
            vec![Row::new(labelled(tables, strdef::PIERCING, amount), BONUS)]
        }
        10 | 0x15 => vec![Row::new(labelled(tables, strdef::ABSORB, amount), BONUS)],
        0x16 | 0x19 | 0x1A => {
            vec![
                Row::new(labelled(tables, strdef::PIERCING, amount), BONUS),
                Row::new(labelled(tables, strdef::ABSORB, amount), BONUS),
            ]
        }
        _ => Vec::new(),
    }
}

fn labelled(tables: &Tables, record: usize, value: i32) -> String {
    format!("{} : {value}", tables.text(record))
}

fn uncompressed_selector(item: &Item, items: &ItemList) -> i32 {
    let u = i32::try_from(refine_uncompressed(item, items)).unwrap_or(0);

    (u - 10).rem_euclid(4)
}

fn skill_book_help_rows(id: i32, tables: &Tables) -> Vec<Row> {
    let Some(help) = usize::try_from(id).ok().and_then(|id| tables.help.get(id)) else {
        return Vec::new();
    };

    if help.color[0] == 0 {
        return Vec::new();
    }

    let Some(first) = help.line.iter().position(|line| !line.is_empty()) else {
        return Vec::new();
    };

    (first..item_help::LINES)
        .map(|index| Row::new(help.line[index].clone(), help_color(help, index)))
        .collect()
}

fn skill_point_row(entry: &Entry, tables: &Tables, context: &Context) -> Option<Row> {
    if context.owner_content != Some(LEARN_SKILL_CONTENT) {
        return None;
    }

    let cost = tables
        .skills
        .and_then(|t| usize::try_from(entry.mesh_sub).ok().and_then(|i| t.get(i)))
        .map_or(0, |s| s.skill_points);

    let met = cost <= context.learner.skill_points;

    Some(Row::new(
        tables.number(strdef::SKILL_POINT, i64::from(cost)),
        if met { PLAIN } else { UNMET },
    ))
}

pub fn configured_expiry(
    item: &Item,
    config: &formats::net::client_config::Config,
    strings: &Strings,
    calendar: crate::clock::Calendar,
    portuguese: bool,
) -> Option<Row> {
    let days = item.reserved.swap_bytes();

    if !(1..6500).contains(&item.index) || !(2192..=5844).contains(&days) {
        return None;
    }

    if !config
        .get("itemexpire.ids")?
        .split(|c: char| !c.is_ascii_digit())
        .filter_map(|s| s.parse::<u16>().ok())
        .any(|id| id == item.index)
    {
        return None;
    }

    let template = strings.translate("ss_1663");

    let mut bytes = template.bytes();

    let mut substitutions = 0;

    while let Some(b) = bytes.next() {
        if b == b'%' {
            match bytes.next() {
                Some(b's') => substitutions += 1,
                Some(b'%') => {}
                _ => return None,
            }
        }
    }

    if substitutions != 1 {
        return None;
    }

    fn date(day: i64) -> (i64, i64, i64) {
        let z = day + 719468;

        let era = z.div_euclid(146097);

        let doe = z.rem_euclid(146097);

        let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;

        let y = yoe + era * 400;

        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);

        let mp = (5 * doy + 2) / 153;

        let d = doy - (153 * mp + 2) / 5 + 1;

        let m = mp + if mp < 10 { 3 } else { -9 };

        (y + i64::from(m <= 2), m, d)
    }

    let expiry_day = 18262 + i64::from(days);

    let utc = calendar.unix_seconds.div_euclid(86400);

    let today = [utc, utc - 1, utc + 1]
        .into_iter()
        .find(|&day| date(day).2 == i64::from(calendar.day_of_month))
        .unwrap_or(utc);

    let color = match expiry_day - today {
        ..=2 => 0xfff40000,
        3..=7 => 0xfffca800,
        _ => 0xfffdfda9,
    };

    let (y, m, d) = date(expiry_day);

    let value = if portuguese {
        format!("{d:02}/{m:02}/{y:04}")
    } else {
        format!("{y:04}/{m:02}/{d:02}")
    };

    let mut text = String::new();

    let mut chars = template.chars();

    while let Some(c) = chars.next() {
        if c != '%' {
            text.push(c);
        } else {
            match chars.next() {
                Some('s') => text.push_str(&value),
                Some('%') => text.push('%'),
                _ => unreachable!(),
            }
        }
    }

    Some(Row::new(text, color))
}
