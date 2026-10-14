use crate::error::{Error, Kind, Result};
use crate::net::account::{Effect, ITEM_LEN, Item};
use crate::net::enter::CARRY_SLOTS;

pub const SLOTS: usize = CARRY_SLOTS;

pub const CARRY_OFF: usize = 0x000;

pub const GOLD_OFF: usize = 0x300;

pub const PAYLOAD_LEN: usize = GOLD_OFF + 4;

pub const FRAME_LEN: usize = PAYLOAD_LEN + 0x0C;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Bag {
    pub carry: [Item; SLOTS],
    pub gold: u32,
}

impl Default for Bag {
    fn default() -> Self {
        Bag {
            carry: [Item::default(); SLOTS],
            gold: 0,
        }
    }
}

impl Bag {
    pub fn filled(&self) -> usize {
        self.carry.iter().filter(|item| !item.is_empty()).count()
    }

    pub fn to_payload(&self) -> Vec<u8> {
        let mut out = vec![0u8; PAYLOAD_LEN];

        for (slot, item) in self.carry.iter().enumerate() {
            let at = CARRY_OFF + slot * ITEM_LEN;

            out[at..at + ITEM_LEN].copy_from_slice(&item.to_bytes());
        }

        out[GOLD_OFF..GOLD_OFF + 4].copy_from_slice(&self.gold.to_le_bytes());

        out
    }
}

pub fn parse(payload: &[u8]) -> Result<Bag> {
    if payload.len() < PAYLOAD_LEN {
        return Err(Error::Truncated {
            kind: Kind::Inventory,
            offset: 0,
            needed: PAYLOAD_LEN,
            len: payload.len(),
        });
    }

    let mut carry = [Item::default(); SLOTS];

    for (slot, item) in carry.iter_mut().enumerate() {
        let b = &payload[CARRY_OFF + slot * ITEM_LEN..][..ITEM_LEN];

        let mut effects = [Effect::default(); 3];

        for (n, effect) in effects.iter_mut().enumerate() {
            *effect = Effect {
                id: b[6 + n * 2],
                value: b[7 + n * 2],
            };
        }

        *item = Item {
            index: u16::from_le_bytes([b[0], b[1]]),
            appearance: u16::from_le_bytes([b[2], b[3]]),
            reserved: u16::from_le_bytes([b[4], b[5]]),
            effects,
        };
    }

    let gold = u32::from_le_bytes([
        payload[GOLD_OFF],
        payload[GOLD_OFF + 1],
        payload[GOLD_OFF + 2],
        payload[GOLD_OFF + 3],
    ]);

    Ok(Bag { carry, gold })
}

pub fn split_frame(client_id: u16, slot: usize, item: u16, quantity: u32) -> Option<[u8; 24]> {
    if slot >= 60 || item == 0 || item > i16::MAX as u16 || !(1..500).contains(&quantity) {
        return None;
    }

    let mut frame = [0u8; 24];

    frame[4..6].copy_from_slice(&0x2E5u16.to_le_bytes());

    frame[6..8].copy_from_slice(&client_id.to_le_bytes());

    frame[12..16].copy_from_slice(&(slot as u32).to_le_bytes());

    frame[16..20].copy_from_slice(&u32::from(item).to_le_bytes());

    frame[20..24].copy_from_slice(&quantity.to_le_bytes());

    Some(frame)
}

pub fn split_individual_frame(
    client_id: u16,
    slot: usize,
    item: u16,
    quantity: u32,
) -> Option<[u8; 32]> {
    let ordinary = split_frame(client_id, slot, item, quantity)?;

    let mut frame = [0; 32];

    frame[..24].copy_from_slice(&ordinary);

    frame[4..6].copy_from_slice(&0x4308u16.to_le_bytes());

    Some(frame)
}
