use super::attack::{cell_of, safe_here};
use super::field::FieldScene;
use super::sel_info::town_at;
use formats::net::account;

pub const SAVE_CITY: u16 = 0x291;

pub const SAVE_CITY_LEN: usize = 0x10;

pub const SAVED_TOWNS: usize = 4;

pub const MUSIC_RECT: (i32, i32, i32, i32) = (1036, 1700, 1088, 1774);

pub const MUSIC_RECT_SLOT: i32 = 8;

pub const MUSIC_FOLDED_TOWN: usize = 2;

pub fn castle_zone(zone: (u8, u8)) -> bool {
    matches!(zone.0, 8 | 9) && matches!(zone.1, 0xF | 0x10)
}

pub fn music_slot(cell: (i32, i32)) -> i32 {
    let (left, top, right, bottom) = MUSIC_RECT;

    if (left..right).contains(&cell.0) && (top..bottom).contains(&cell.1) {
        return MUSIC_RECT_SLOT;
    }

    let town = match town_at(cell.0, cell.1) {
        MUSIC_FOLDED_TOWN => 0,
        town => town,
    };

    town as i32 * 2 + 1
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct State {
    pub music_slot: i32,
    pub castle_town: i32,
}

impl State {
    pub fn new() -> State {
        State {
            music_slot: 0,
            castle_town: NO_TOWN,
        }
    }
}

impl Default for State {
    fn default() -> State {
        State::new()
    }
}

pub const NO_TOWN: i32 = -1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Saved {
    Nothing,
    Refused {
        town: usize,
    },
    Sent {
        town: usize,
        slot: usize,
        position: (u16, u16),
    },
}

pub fn tick(scene: &mut FieldScene) -> Saved {
    let Some(player) = scene.player else {
        return Saved::Nothing;
    };

    let cell = cell_of(&scene.world.actors.actors[player]);

    let castle = scene.siege.at_war();

    if !safe_here(scene.world.actors.attributes(), cell) {
        if castle {
            scene.city.castle_town = NO_TOWN;
        }

        return Saved::Nothing;
    }

    let moved = match castle {
        true => {
            let town = town_at(cell.0, cell.1) as i32;

            let moved = scene.city.castle_town != town;

            scene.city.castle_town = town;

            moved
        }
        false => {
            let slot = music_slot(cell);

            let moved = scene.city.music_slot != slot;

            scene.city.music_slot = slot;

            moved
        }
    };

    if !moved {
        return Saved::Nothing;
    }

    let town = town_at(cell.0, cell.1);

    if town >= SAVED_TOWNS {
        return Saved::Refused { town };
    }

    scene.outbox.push(super::interact::frame(
        SAVE_CITY,
        SAVE_CITY_LEN,
        town as u32,
    ));

    let position = (cell.0 as u16, cell.1 as u16);

    Saved::Sent {
        town,
        slot: scene.entered.slot,
        position,
    }
}

pub fn write_back(block: &mut account::Block, slot: usize, position: (u16, u16)) -> bool {
    let Some(record) = block.slots.get_mut(slot) else {
        return false;
    };

    record.x = position.0;

    record.z = position.1;

    true
}

pub fn describe(saved: &Saved) -> Option<String> {
    let name = |town: usize| match super::sel_info::TOWN_STRDEF.get(town) {
        Some(strdef) => format!("row {town}, strdef[{strdef}]"),
        None => format!("row {town}, no town"),
    };

    match saved {
        Saved::Nothing => None,
        Saved::Refused { town } => Some(format!(
            "NWYD: 0x291 not sent — {} is at or past row {SAVED_TOWNS}",
            name(*town)
        )),
        Saved::Sent {
            town,
            slot,
            position,
        } => Some(format!(
            "NWYD: 0x291 last city — {}, account slot {slot} now at ({}, {})",
            name(*town),
            position.0,
            position.1,
        )),
    }
}
