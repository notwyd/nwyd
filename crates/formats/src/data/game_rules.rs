use crate::net::account::Item;

pub fn refinement_prompt(source: u16, target: &Item, level: i32) -> Option<usize> {
    match target.index {
        5721..=5726 | 5771..=5776 if source == 413 && level >= 9 => Some(1594),
        591..=595 if level >= 9 => Some(1594),
        5912 if matches!(source, 413 | 4141)
            && !target.effects.iter().any(|e| e.id == 111 && e.value != 0) =>
        {
            Some(1844)
        }
        _ => None,
    }
}

pub fn refinement_denied(item: u16, level: i32) -> bool {
    matches!(
        item, 997..= 1000 | 1732 | 1744..= 1751 | 1760..= 1763 | 1767 | 1770 | 3000..=
        3049 | 3197..= 3199 | 3503..= 3506 | 3510..= 3512 | 5811..= 5815 | 6447..= 6449 |
        6493..= 6499
    ) || (level >= 11 && matches!(item, 3464 | 5707..= 5713 | 5757..= 5763))
}

pub fn skill_weapon_allowed(raw: u8, weapon_effect: i32) -> bool {
    let code = if raw >= 105 {
        raw.wrapping_add(95)
    } else {
        raw
    };

    match code {
        75 | 79 => weapon_effect == 101,
        92 => weapon_effect == 41,
        _ => true,
    }
}
