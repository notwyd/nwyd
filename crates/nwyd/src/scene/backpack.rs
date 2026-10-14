use super::field::FieldScene;
use super::panels::Panels;
use super::store::MONEY_USE_KIND;
use formats::data::item_effect::{Effects, effect};
use formats::net::account::Item;
use formats::net::backpack::{Arrival, Clear, Container};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arrived {
    Gold {
        amount: u32,
        before: u32,
        after: u32,
    },
    Slot {
        slot: usize,
        item: Item,
        replaced: Item,
    },
    Refused {
        slot: u32,
    },
    NoTable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cleared {
    pub container: Option<Container>,
    pub slot: u32,
    pub removed: Option<Item>,
    pub refused: bool,
    pub cargo_cell: Option<(i32, i32)>,
    pub pet_mirror: u16,
    pub sound: bool,
}

pub fn is_money(panels: &Panels, item: &Item) -> bool {
    Effects::new(&panels.items, &[]).total_scaled(item, effect::USE_KIND) == MONEY_USE_KIND
}

fn gold_of(panels: &Panels, item: &Item) -> u32 {
    let effects = Effects::new(&panels.items, &[]);

    Arrival::gold_from(
        effects.total_scaled(item, formats::net::backpack::GOLD_HIGH_EFFECT),
        effects.total_scaled(item, formats::net::backpack::GOLD_LOW_EFFECT),
    )
}

pub fn arrived(scene: &mut FieldScene, arrival: &Arrival) -> Arrived {
    let Some(panels) = scene.panels.as_ref() else {
        return Arrived::NoTable;
    };

    if is_money(panels, &arrival.item) {
        let amount = gold_of(panels, &arrival.item);

        let before = scene.entered.gold;

        let after = before.wrapping_add(amount);

        scene.entered.gold = after;

        return Arrived::Gold {
            amount,
            before,
            after,
        };
    }

    let Ok(slot) = usize::try_from(arrival.slot) else {
        return Arrived::Refused { slot: arrival.slot };
    };

    let Some(cell) = scene.entered.carry.get_mut(slot) else {
        return Arrived::Refused { slot: arrival.slot };
    };

    let replaced = *cell;

    *cell = arrival.item;

    Arrived::Slot {
        slot,
        item: arrival.item,
        replaced,
    }
}

pub fn notify_cleared(scene: &mut FieldScene) {
    if let Some(panels) = scene.panels.as_mut() {
        super::panels::refine_item_changed(panels, &scene.entered);
    }
}

pub fn cleared(scene: &mut FieldScene, clear: &Clear) -> Cleared {
    let mut removed = None;

    let mut refused = false;

    if let Some(container) = clear.container {
        match usize::try_from(clear.slot)
            .ok()
            .filter(|slot| *slot < container.slots())
        {
            Some(slot) => {
                let cell = match container {
                    Container::Equipment => &mut scene.entered.equipment.items[slot],
                    Container::Carry => &mut scene.entered.carry[slot],
                    Container::Cargo => &mut scene.entered.warehouse.cargo[slot],
                };

                removed = Some(*cell);

                *cell = Item::default();
            }
            None => refused = true,
        }
    }

    scene.entered.refresh_class();

    if let Some(actor) = scene
        .player
        .and_then(|index| scene.world.actors.actors.get_mut(index))
    {
        actor.pet_item = scene.entered.equipment.items[13].index;
    }

    Cleared {
        container: clear.container,
        slot: clear.slot,
        removed,
        refused,
        cargo_cell: clear.cargo_cell(),
        pet_mirror: scene.entered.equipment.items[formats::net::backpack::PET_MIRROR_SLOT].index,
        sound: true,
    }
}

pub fn describe_arrival(applied: &Arrived) -> String {
    match applied {
        Arrived::Gold {
            amount,
            before,
            after,
        } => {
            format!("NWYD: 0x171 — {amount} coins arrived, pocket {before} -> {after}")
        }
        Arrived::Slot {
            slot,
            item,
            replaced,
        } if replaced.is_empty() => {
            format!("NWYD: 0x171 — item {} into carry slot {slot}", item.index)
        }
        Arrived::Slot {
            slot,
            item,
            replaced,
        } => {
            format!(
                "NWYD: 0x171 — item {} into carry slot {slot}, over item {}",
                item.index, replaced.index,
            )
        }
        Arrived::Refused { slot } => {
            format!(
                "NWYD: 0x171 — carry slot {slot} is outside the {} the array has, refused",
                formats::net::enter::CARRY_SLOTS,
            )
        }
        Arrived::NoTable => {
            "NWYD: 0x171 arrived with no item table up, so neither arm can be taken".to_string()
        }
    }
}

pub fn describe_clear(applied: &Cleared) -> String {
    let where_ = match applied.container {
        Some(Container::Equipment) => "equipment".to_string(),
        Some(Container::Carry) => "carry".to_string(),
        Some(Container::Cargo) => "warehouse".to_string(),
        None => format!("container {} (no arm)", applied.slot),
    };

    match (applied.refused, applied.removed) {
        (true, _) => {
            format!(
                "NWYD: 0x175 — {where_} slot {} is out of range, refused",
                applied.slot
            )
        }
        (false, None) => "NWYD: 0x175 — nothing cleared, and the tail ran anyway".to_string(),
        (false, Some(item)) if item.is_empty() => {
            format!(
                "NWYD: 0x175 — {where_} slot {} was already empty",
                applied.slot
            )
        }
        (false, Some(item)) => {
            format!(
                "NWYD: 0x175 — item {} left {where_} slot {}",
                item.index, applied.slot
            )
        }
    }
}
