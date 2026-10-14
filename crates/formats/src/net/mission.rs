use crate::data::Item;
use crate::error::{Error, Kind, Result};

pub const CONFIRM: u16 = 0x2E7;

pub const SLOTS: usize = 8;

pub const FRAME_LEN: usize = 0x78;

pub const LEN: usize = FRAME_LEN - 0x0C;

const ITEMS_AT: usize = 0x04;

const POSITIONS_AT: usize = ITEMS_AT + SLOTS * Item::LEN;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Confirm {
    pub mission: i32,
    pub item: [Item; SLOTS],
    pub position: [i8; SLOTS],
}

impl Default for Confirm {
    fn default() -> Confirm {
        Confirm {
            mission: -1,
            item: [Item::default(); SLOTS],
            position: [-1; SLOTS],
        }
    }
}

impl Confirm {
    pub fn filled(&self) -> impl Iterator<Item = (usize, Item)> + '_ {
        (0..SLOTS)
            .filter(|&i| self.position[i] >= 0)
            .map(|i| (i, self.item[i]))
    }

    pub fn is_consistent(&self) -> bool {
        (0..SLOTS).all(|i| (self.position[i] >= 0) == !self.item[i].is_empty())
    }

    pub fn to_bytes(&self) -> [u8; LEN] {
        let mut out = [0u8; LEN];

        out[0x00..0x04].copy_from_slice(&self.mission.to_le_bytes());

        for (i, item) in self.item.iter().enumerate() {
            let at = ITEMS_AT + i * Item::LEN;

            out[at..at + Item::LEN].copy_from_slice(&item.to_bytes());
        }

        for (i, &p) in self.position.iter().enumerate() {
            out[POSITIONS_AT + i] = p as u8;
        }

        out
    }
}

pub fn parse(payload: &[u8]) -> Result<Confirm> {
    if payload.len() < LEN {
        return Err(Error::structure(
            Kind::MissionConfirm,
            format!("expected at least {LEN} bytes, found {}", payload.len()),
        ));
    }

    let mut item = [Item::default(); SLOTS];

    for (i, slot) in item.iter_mut().enumerate() {
        *slot = Item::parse(&payload[ITEMS_AT + i * Item::LEN..]);
    }

    let mut position = [-1i8; SLOTS];

    for (i, p) in position.iter_mut().enumerate() {
        *p = payload[POSITIONS_AT + i] as i8;
    }

    Ok(Confirm {
        mission: i32::from_le_bytes([payload[0], payload[1], payload[2], payload[3]]),
        item,
        position,
    })
}
