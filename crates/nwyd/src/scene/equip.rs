use crate::scene::enter::Entered;
use crate::scene::item_move::{Coord, container, item_at};
use formats::data::item_effect::{self, Effects, effect};
use formats::data::item_list::{ItemList, equip};
use formats::net::account::Item;

pub const MAIN_HAND: i32 = 6;

pub const OFF_HAND: i32 = 7;

pub const HELMET: i32 = 1;

pub const SEALED_SLOT: i32 = 0xF;

pub const ANY_SLOT: i32 = -1;

pub const THROWN: i16 = 0x2E;

pub const TRANSCENDED: i32 = 6;

pub const TRANSFORMED: [i32; 5] = [0x16, 0x17, 0x18, 0x19, 0x20];

pub const ATILAS_CROWN: i32 = 0x2EB;

pub fn may_equip(player: &Entered, items: &ItemList, src: Coord, dst: Coord) -> bool {
    if dst.container != container::EQUIPMENT {
        return true;
    }

    let Some(source) = item_at(player, src) else {
        return false;
    };

    let mask = mask_of(items, &source);

    if dst.slot == OFF_HAND && matches!(mask, equip::WEAPON | equip::TWO_HANDED) {
        let main = player
            .equipment
            .items
            .get(MAIN_HAND as usize)
            .map_or(0, |i| i.index as i16);

        if main < 1 {
            return false;
        }

        if src.container == container::EQUIPMENT && src.slot == MAIN_HAND {
            return false;
        }
    }

    allows(player, items, &source, dst.slot)
}

pub fn ctrl_movable(items: &ItemList, source: &Item) -> bool {
    mask_of(items, source) >= 2
}

pub fn mask_slot(mask: i32) -> i32 {
    let mut mask = mask;

    let mut slot = 0;

    loop {
        mask /= 2;

        if mask == 0 {
            return slot;
        }

        slot += 1;
    }
}

pub fn ctrl_target(player: &Entered, items: &ItemList, source: &Item) -> Option<i32> {
    let effects = Effects::new(items, &[]);

    let mut target = mask_slot(effects.total_scaled(source, effect::EQUIP_MASK));

    let mask = mask_of(items, source) as i32;

    if !allows(player, items, source, ANY_SLOT) {
        tracing::debug!(
            item = source.index,
            body = player.equipment.items[0].index,
            account_body = player.account_body,
            "CTRL equip rejected by general equipment requirements"
        );

        return None;
    }

    if (equip::WEAPON as i32..=equip::TWO_HANDED as i32).contains(&mask) {
        let main = hand_mask(player, items, MAIN_HAND);

        let off = hand_mask(player, items, OFF_HAND);

        let mut fixup = true;

        if main == equip::WEAPON as i32 {
            if off == equip::SHIELD as i32 && mask != equip::SHIELD as i32 {
                target = MAIN_HAND;
            }

            if mask == equip::TWO_HANDED as i32 {
                target = MAIN_HAND;
            }
        } else if main == equip::TWO_HANDED as i32 && off == equip::TWO_HANDED as i32 {
            if mask != equip::SHIELD as i32 && mask != equip::TWO_HANDED as i32 {
                tracing::debug!(
                    item = source.index,
                    main,
                    off,
                    mask,
                    "CTRL equip rejected by occupied-hand combination"
                );

                return None;
            }

            fixup = false;
        }

        if fixup
            && off == equip::SHIELD as i32
            && target == OFF_HAND
            && mask != equip::SHIELD as i32
        {
            target = MAIN_HAND;
        }

        if mask == equip::SHIELD as i32 {
            target = OFF_HAND;
        } else if (player.equipment.items[MAIN_HAND as usize].index as i16) < 1 {
            target = MAIN_HAND;
        }

        if !allows(player, items, source, target) {
            tracing::debug!(
                item = source.index,
                target,
                main,
                off,
                "CTRL equip rejected by destination equipment requirements"
            );

            return None;
        }
    }

    Some(target)
}

fn hand_mask(player: &Entered, items: &ItemList, slot: i32) -> i32 {
    let index = player
        .equipment
        .items
        .get(slot as usize)
        .map_or(0, |item| item.index);

    let bare = Item {
        index,
        ..Item::default()
    };

    Effects::new(items, &[]).total_scaled(&bare, effect::EQUIP_MASK)
}

pub fn allows(player: &Entered, items: &ItemList, source: &Item, slot: i32) -> bool {
    let fx = Effects::new(items, &[]);

    let id = i32::from(source.index as i16);

    let reject = |reason: &str| {
        tracing::debug!(
            target : "nwyd::body_state", event = "equipment_rejected", entity = player
            .client_id, reason, item = ? source, target_slot = slot, live_body = player
            .equipment.body().index, retained_body = player.account_body, class = player
            .class, main_hand = ? player.equipment.items[6], off_hand = ? player
            .equipment.items[7], level = player.status.level, stats = ? player.status
            .stat, "Local equipment eligibility refusal"
        );

        false
    };

    if !(1..=item_effect::MAX_ID).contains(&id) {
        return reject("invalid_item");
    }

    let grade = i32::from(grade_of(items, source));

    if slot == SEALED_SLOT {
        return reject("sealed_slot");
    }

    if slot != ANY_SLOT {
        let mask = fx.total_scaled(source, effect::EQUIP_MASK);

        if mask >> (slot & 0x1F) & 1 == 0 {
            return reject("slot_mask");
        }

        let other = match slot {
            MAIN_HAND => OFF_HAND,
            OFF_HAND => MAIN_HAND,
            _ => ANY_SLOT,
        };

        if other != ANY_SLOT {
            let worn = player
                .equipment
                .items
                .get(other as usize)
                .copied()
                .unwrap_or_default();

            let worn_id = i32::from(worn.index as i16);

            if (1..=item_effect::MAX_ID).contains(&worn_id) {
                let worn_grade = i32::from(grade_of(items, &worn));

                let worn_mask = fx.total_scaled(&worn, effect::EQUIP_MASK);

                if mask as u32 == equip::WEAPON || worn_mask as u32 == equip::WEAPON {
                    match grade == i32::from(THROWN) {
                        true => {
                            if worn_mask as u32 != equip::SHIELD {
                                return reject("thrown_weapon_requires_shield");
                            }
                        }
                        false => {
                            if worn_grade != i32::from(THROWN) {
                                return reject("other_hand_requires_thrown_weapon");
                            }

                            if mask as u32 != equip::SHIELD {
                                return reject("other_hand_requires_shield");
                            }
                        }
                    }
                }
            }
        }
    }

    let mut code = i32::from(player.equipment.body().index as i16);

    if TRANSFORMED.contains(&code) {
        code = i32::from(player.account_body as i16);
    }

    let rank = code % 10;

    match fx.total_scaled(source, effect::RESTRICTION) {
        1 => {
            if rank < TRANSCENDED {
                return reject("requires_transcended_body");
            }
        }
        2 => {
            if rank >= TRANSCENDED {
                return reject("requires_non_transcended_body");
            }
        }
        3 => {
            if rank < TRANSCENDED {
                return reject("requires_transcended_body");
            }

            if FLAG != 1 {
                return reject("restriction_flag");
            }
        }
        _ => {
            if slot == HELMET && rank >= TRANSCENDED && id != ATILAS_CROWN {
                return reject("transcended_helmet");
            }
        }
    }

    let mask = fx.total_scaled(source, effect::EQUIP_MASK);

    if fx.total_scaled(source, effect::CLASS) >> ((code / 10) & 0x1F) & 1 == 0
        && (rank < TRANSCENDED
            || !matches!(
                mask as u32,
                equip::WEAPON | equip::SHIELD | equip::TWO_HANDED
            ))
    {
        return reject("class_mask");
    }

    if rank >= TRANSCENDED {
        return true;
    }

    let mut need = [
        fx.total_scaled(source, effect::REQ_LEVEL),
        fx.total_scaled(source, effect::REQ_STR),
        fx.total_scaled(source, effect::REQ_INT),
        fx.total_scaled(source, effect::REQ_DEX),
        fx.total_scaled(source, effect::REQ_CON),
    ];

    let digit = fx.total_scaled(source, effect::OFF_HAND_LADDER) % 10;

    if slot == OFF_HAND && digit != 0 {
        let pct = if digit > 1 { 130 } else { 100 };

        for value in &mut need {
            *value = pct * *value / 100;
        }
    }

    let have = [
        i32::from(player.status.level as i16),
        i32::from(player.status.stat[0]),
        i32::from(player.status.stat[1]),
        i32::from(player.status.stat[2]),
        i32::from(player.status.stat[3]),
    ];

    if have.iter().zip(need).any(|(&have, need)| have < need) {
        tracing::debug!(
            target : "nwyd::body_state", ? have, ? need, "Equipment stat comparison"
        );

        return reject("insufficient_stats");
    }

    if grade % 10 > 8 && grade < 0x28 {
        return reject("grade_requires_transcendence");
    }

    true
}

const FLAG: i32 = 1;

fn mask_of(items: &ItemList, item: &Item) -> u32 {
    entry_of(items, item).map_or(0, |entry| entry.equip_mask)
}

fn grade_of(items: &ItemList, item: &Item) -> i16 {
    entry_of(items, item).map_or(0, |entry| entry.grade)
}

fn entry_of(items: &ItemList, item: &Item) -> Option<formats::data::item_list::Entry> {
    let id = i32::from(item.index as i16);

    usize::try_from(id).ok().and_then(|id| items.get(id))
}
