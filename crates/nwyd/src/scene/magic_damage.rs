use super::enter::Entered;
use formats::data::item_effect::{Effects, effect};
use formats::data::skill_data::Skill;
use formats::mesh::appearance::refine_level;
use formats::net::account::{Equipment, Status};
use formats::net::enter::{QUICK_SLOTS, selected};

const MAIN_HAND: usize = 6;

const OFF_HAND: usize = 7;

const SPECIAL_GRADE_2C: i16 = 0x2C;

const SPECIAL_GRADE_2D: i16 = 0x2D;

const SPECIAL_GRADE_2F: i16 = 0x2F;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Display {
    Invalid,
    Value { shown: i32, colour: u32 },
}

pub const NEGATIVE_COLOUR: u32 = 0xFFAA_FFAA;

pub const NONNEGATIVE_COLOUR: u32 = 0xFFBB_BBFF;

pub fn display(result: Option<i32>) -> Display {
    match result {
        None => Display::Invalid,
        Some(value) if value < 0 => Display::Value {
            shown: value.wrapping_neg(),
            colour: NEGATIVE_COLOUR,
        },
        Some(value) => Display::Value {
            shown: value,
            colour: NONNEGATIVE_COLOUR,
        },
    }
}

pub fn selected_code(bar: &[u8; QUICK_SLOTS], armed: u8) -> Option<u8> {
    selected(bar, armed).filter(|&code| code as i8 >= 0)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct Input {
    pub level: i16,
    pub class: i8,
    pub strength: u16,
    pub intelligence: u16,
    pub mastery: [u16; 4],
    pub skill_mask: [u32; 2],
    pub magic_bonus: i16,
    pub attack_damage: i32,
    pub account_body: i16,
    pub weapon_bonus: i32,
}

impl Input {
    pub fn of(entered: &Entered, weapon_bonus: i32) -> Input {
        let Status {
            level,
            attack_damage,
            stat,
            mastery,
            ..
        } = entered.status;

        Input {
            level: level as i16,
            class: entered.class as i8,
            strength: stat[0],
            intelligence: stat[1],
            mastery,
            skill_mask: entered.skill_mask,
            magic_bonus: entered.derived.magic_bonus,
            attack_damage: attack_damage as i32,
            account_body: entered.account_body as i16,
            weapon_bonus,
        }
    }
}

pub fn weapon_bonus(equipment: &Equipment, effects: &Effects<'_>) -> i32 {
    let main = equipment.items[MAIN_HAND];

    let off = equipment.items[OFF_HAND];

    let main_row = effects.items.get(usize::from(main.index));

    let off_row = effects.items.get(usize::from(off.index));

    let main_grade = main_row.as_ref().map_or(0, |row| row.grade);

    let off_grade = off_row.as_ref().map_or(0, |row| row.grade);

    let main_damage = effects.total_scaled(&main, effect::DAMAGE);

    let mut off_damage = effects.total_scaled(&off, effect::DAMAGE);

    if main_grade == SPECIAL_GRADE_2F && off_grade == SPECIAL_GRADE_2D {
        off_damage = 0;
    }

    let mut total = if off_damage < main_damage {
        main_damage.wrapping_add(off_damage / 3)
    } else {
        off_damage.wrapping_add(main_damage / 3)
    };

    let main_mask = main_row.as_ref().map_or(0, |row| row.equip_mask);

    if matches!(main_mask, 0x40 | 0xC0)
        && !matches!(main_grade, SPECIAL_GRADE_2C | SPECIAL_GRADE_2F)
        && off_grade != SPECIAL_GRADE_2F
        && refine_level(&main, effects.items) > 8
    {
        total = total.wrapping_add(40);
    }

    let off_mask = off_row.as_ref().map_or(0, |row| row.equip_mask);

    if matches!(off_mask, 0x40 | 0xC0)
        && !matches!(off_grade, SPECIAL_GRADE_2C | SPECIAL_GRADE_2F)
        && main_grade != SPECIAL_GRADE_2C
        && refine_level(&off, effects.items) > 8
    {
        total = total.wrapping_add(40);
    }

    total
}

pub fn damage(code: u8, skill: &Skill, weather: i32, input: Input) -> i32 {
    let code = i32::from(code);

    let group = (code % 24) / 8;

    let element = (code / 8) % 3;

    let mastery = i32::from(input.mastery[group as usize + 1]);

    let base = skill.base_damage;

    let coefficient = skill.damage_coefficient;

    let kind = skill.damage_kind;

    if kind == 0 {
        let value = match code {
            0x0B | 0x2D => add(coefficient, mastery / 10),
            0x0D => add(coefficient, mul(mastery, 3)),
            0x29 => add(mastery / 25, 2),
            0x2B => add(add(mastery / 3, 15), coefficient),
            0x2C => mul(add(mastery / 3, 15), 5),
            _ => 0,
        };

        return if code == 0x0B && input.skill_mask[1] & 0x80 != 0 {
            add(value, value / 10)
        } else {
            value
        };
    }

    if !(1..=5).contains(&kind) {
        if kind == 6 {
            let value = add(mul(mastery, 3) / 2, base);

            return if code == 0x1D && input.skill_mask[0] & 0x80 != 0 {
                percent(value, 120)
            } else {
                value
            };
        }

        return if kind == 11 {
            base
        } else {
            mul(i32::from(input.magic_bonus), 2)
        };
    }

    let body_high = i32::from(input.account_body) % 10 >= 6;

    let cap = if body_high && input.skill_mask[0] & 0x4000_0000 != 0 {
        500
    } else {
        400
    };

    let level = i32::from(input.level).min(cap);

    let strength = i32::from(input.strength);

    let intelligence = i32::from(input.intelligence);

    let weapon = input.weapon_bonus;

    let mut value = if code == 0x61 {
        add(mul(level, 15), base)
    } else {
        match input.class {
            0 if group == 1 => {
                let level = if body_high { level } else { level / 2 };

                wrapping_sum_four(level, base, mul(add(strength, weapon), 3), mastery)
            }
            0 => {
                let level = if body_high { level } else { level / 2 };

                wrapping_sum_four(
                    add(intelligence / 40, intelligence / 4),
                    base,
                    mastery,
                    add(level, weapon),
                )
            }
            1 | 2 => {
                let level = if body_high { level } else { level / 2 };

                let mastery = if body_high { mul(mastery, 2) } else { mastery };

                wrapping_sum_four(
                    add(intelligence / 3, intelligence / 30),
                    base,
                    mastery,
                    level,
                )
            }
            3 if code == 0x4F => input.attack_damage,
            3 => wrapping_sum_four(level / 2, base, mul(add(strength, weapon), 3), mastery),
            _ => 0,
        }
    };

    value = match (weather, kind) {
        (1, 2) => percent(value, 90),
        (1, 5) => percent(value, 130),
        (2, 3) => percent(value, 120),
        _ => value,
    };

    if code == 0x4F {
        return value;
    }

    if code != 0x61 {
        if !((input.class == 0 && group == 1) || input.class == 3) {
            value = percent(value, add(mul(i32::from(input.magic_bonus), 4), 100));
        }

        value = mul(value, 5) / 4;
    }

    let element_bit = 1u32 << (element * 8 + 7);

    if input.skill_mask[0] & element_bit == 0 {
        return value;
    }

    match (input.class, element) {
        (0, 0 | 2) => percent(value, 115),
        (0, 1) => percent(value, 120),
        (1, 0) | (2, 0) | (3, 0 | 1) => percent(value, 110),
        (1, 1 | 2) => percent(value, 115),
        (3, 2) => percent(value, 120),
        _ => value,
    }
}

fn add(a: i32, b: i32) -> i32 {
    a.wrapping_add(b)
}

fn wrapping_sum_four(a: i32, b: i32, c: i32, d: i32) -> i32 {
    add(add(a, b), add(c, d))
}

fn mul(a: i32, b: i32) -> i32 {
    a.wrapping_mul(b)
}

fn percent(value: i32, multiplier: i32) -> i32 {
    mul(value, multiplier) / 100
}
