use crate::error::{Error, Kind, Result};
use crate::net::account::{ITEM_LEN, Item, item_at};
use crate::util::{byte_at, u16_at, u32_at};

pub const OFFER: u16 = 0x383;

pub const CLOSE: u16 = 0x384;

pub const READY_FORCED: u16 = 0x386;

pub const OFFER_FRAME_LEN: usize = 0xD8;

pub const OFFER_LEN: usize = OFFER_FRAME_LEN - 0x0C;

pub const CLOSE_FRAME_LEN: usize = 0x0C;

pub const SLOTS: usize = 15;

pub const ITEMS_OFF: usize = 0x00;

pub const SOURCE_SLOTS_OFF: usize = ITEMS_OFF + SLOTS * ITEM_LEN;

pub const GOLD_OFF: usize = 0xC4;

pub const READY_OFF: usize = 0xC8;

pub const PARTNER_OFF: usize = 0xCA;

pub const WITH_STRDEF: usize = 64;

pub const QUESTION_STRDEF: usize = 28;

pub const SOURCE_NONE: u8 = 0xFF;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Offer {
    pub items: [Item; SLOTS],
    pub source_slots: [u8; SLOTS],
    pub gold: u32,
    pub ready: u8,
    pub partner: u16,
    pub unknown: [u8; 2],
}

impl Default for Offer {
    fn default() -> Offer {
        Offer {
            items: [Item::default(); SLOTS],
            source_slots: [SOURCE_NONE; SLOTS],
            gold: 0,
            ready: 0,
            partner: 0,
            unknown: [0; 2],
        }
    }
}

impl Offer {
    pub fn cleared() -> Offer {
        Offer::default()
    }

    pub fn is_empty(&self) -> bool {
        self.gold == 0 && self.items.iter().all(Item::is_empty)
    }

    pub fn filled(&self) -> usize {
        self.items.iter().filter(|item| !item.is_empty()).count()
    }

    pub fn differs_from(&self, other: &Offer) -> bool {
        self.gold != other.gold || self.items != other.items
    }

    pub fn to_bytes(&self) -> [u8; OFFER_LEN] {
        let mut out = [0u8; OFFER_LEN];

        for (n, item) in self.items.iter().enumerate() {
            let at = ITEMS_OFF + n * ITEM_LEN;

            out[at..at + ITEM_LEN].copy_from_slice(&item.to_bytes());
        }

        out[SOURCE_SLOTS_OFF..SOURCE_SLOTS_OFF + SLOTS].copy_from_slice(&self.source_slots);

        out[GOLD_OFF - 1] = self.unknown[0];

        out[GOLD_OFF..GOLD_OFF + 4].copy_from_slice(&self.gold.to_le_bytes());

        out[READY_OFF] = self.ready;

        out[READY_OFF + 1] = self.unknown[1];

        out[PARTNER_OFF..PARTNER_OFF + 2].copy_from_slice(&self.partner.to_le_bytes());

        out
    }

    pub fn to_frame(&self) -> [u8; OFFER_FRAME_LEN] {
        let mut out = [0u8; OFFER_FRAME_LEN];

        out[0x04..0x06].copy_from_slice(&OFFER.to_le_bytes());

        out[0x0C..].copy_from_slice(&self.to_bytes());

        out
    }
}

pub fn parse(payload: &[u8]) -> Result<Offer> {
    if payload.len() < OFFER_LEN {
        return Err(Error::Truncated {
            kind: Kind::Trade,
            offset: 0,
            needed: OFFER_LEN,
            len: payload.len(),
        });
    }

    let mut items = [Item::default(); SLOTS];

    for (n, item) in items.iter_mut().enumerate() {
        *item = item_at(payload, ITEMS_OFF + n * ITEM_LEN)?;
    }

    let mut source_slots = [0u8; SLOTS];

    source_slots.copy_from_slice(&payload[SOURCE_SLOTS_OFF..SOURCE_SLOTS_OFF + SLOTS]);

    Ok(Offer {
        items,
        source_slots,
        gold: u32_at(payload, GOLD_OFF, Kind::Trade)?,
        ready: byte_at(payload, READY_OFF, Kind::Trade)?,
        partner: u16_at(payload, PARTNER_OFF, Kind::Trade)?,
        unknown: [
            byte_at(payload, GOLD_OFF - 1, Kind::Trade)?,
            byte_at(payload, READY_OFF + 1, Kind::Trade)?,
        ],
    })
}

pub fn close_frame() -> [u8; CLOSE_FRAME_LEN] {
    let mut out = [0u8; CLOSE_FRAME_LEN];

    out[0x04..0x06].copy_from_slice(&CLOSE.to_le_bytes());

    out
}
