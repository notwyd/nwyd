use super::{consume, emote, hud, item_move, item_use};
use formats::data::item_effect::Effects;
use formats::net::account::Item;

pub const KEY: u8 = 0x7A;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refusal {
    InputLocked,
    SharedFloor,
    AirMove,
    MidStep,
}

pub fn admit(
    input_lock: &hud::InputLock,
    now_ms: u32,
    floor_ms: u32,
    air_move: bool,
    move_alpha: Option<f32>,
) -> Result<(), Refusal> {
    if !input_lock.allows(now_ms) {
        return Err(Refusal::InputLocked);
    }

    if emote::too_soon(now_ms, floor_ms) {
        return Err(Refusal::SharedFloor);
    }

    if air_move {
        return Err(Refusal::AirMove);
    }

    if move_alpha.is_some_and(|alpha| alpha > 0.0 && alpha < 0.9) {
        return Err(Refusal::MidStep);
    }

    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    Refused(Refusal),
    NoScroll,
    MissingBody { slot: i32 },
    Used(Spent),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spent {
    pub slot: i32,
    pub previous_count: i32,
    pub emptied: bool,
}

pub fn first_slot(carry: &[Item], effects: &Effects<'_>) -> Option<i32> {
    carry
        .iter()
        .take(item_move::INVENTORY_SLOTS)
        .position(|item| {
            !item.is_empty()
                && effects.total_scaled(item, item_move::EFFECT_KIND) == item_use::KIND_RECALL
        })
        .map(|slot| slot as i32)
}

pub fn spend(carry: &mut [Item], slot: i32) -> Option<Spent> {
    let slot = usize::try_from(slot).ok()?;

    if slot >= item_move::INVENTORY_SLOTS {
        return None;
    }

    let cell = carry.get_mut(slot)?;

    let previous_count = consume::stored_count(cell);

    let emptied = previous_count < 2;

    if emptied {
        *cell = Item::default();
    } else {
        cell.appearance = (previous_count - 1) as u16;
    }

    Some(Spent {
        slot: slot as i32,
        previous_count,
        emptied,
    })
}

pub fn truncate_coordinate_to_u16(value: f32) -> u16 {
    let truncated = f64::from(value).trunc();

    if !truncated.is_finite() || truncated < f64::from(i32::MIN) || truncated > f64::from(i32::MAX)
    {
        0
    } else {
        truncated as i32 as u16
    }
}
