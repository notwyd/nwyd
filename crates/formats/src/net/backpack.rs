use crate::error::{Error, Kind as ErrorKind, Result};
use crate::net::account::{ITEM_LEN, Item, item_at};
use crate::net::ground_item::HEADER_LEN;
use crate::util::u32_at;

const KIND: ErrorKind = ErrorKind::Backpack;

pub const ARRIVED: u16 = 0x171;

pub const CLEARED: u16 = 0x175;

pub const FAMILY: [u16; 2] = [CLEARED, ARRIVED];

pub const ARRIVED_FRAME_LEN: usize = 0x20;

pub const ARRIVED_LEN: usize = ARRIVED_FRAME_LEN - HEADER_LEN;

pub const CLEARED_FRAME_LEN: usize = 0x1C;

pub const CLEARED_LEN: usize = CLEARED_FRAME_LEN - HEADER_LEN;

pub const CLEARED_READ_LEN: usize = 8;

pub mod at {

    pub const ARRIVED_SLOT: usize = 0x04;

    pub const ARRIVED_ITEM: usize = 0x08;

    pub const CLEARED_CONTAINER: usize = 0x00;

    pub const CLEARED_SLOT: usize = 0x04;
}

pub const GOLD_HIGH_EFFECT: u8 = 0x24;

pub const GOLD_LOW_EFFECT: u8 = 0x25;

pub const SOUND_SLOT: i32 = 0x2D;

pub const GOLD_BROADCAST_EVENT: i32 = 0x10;

pub const SLOT_BROADCAST_EVENT: i32 = 0x0D;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Container {
    Equipment,
    Carry,
    Cargo,
}

impl Container {
    pub fn from_dword(value: u32) -> Option<Container> {
        match value {
            0 => Some(Container::Equipment),
            1 => Some(Container::Carry),
            2 => Some(Container::Cargo),
            _ => None,
        }
    }

    pub fn dword(self) -> u32 {
        match self {
            Container::Equipment => 0,
            Container::Carry => 1,
            Container::Cargo => 2,
        }
    }

    pub fn base(self) -> u32 {
        match self {
            Container::Equipment => 0xB48,
            Container::Carry => 0xC20,
            Container::Cargo => 0x4BC,
        }
    }

    pub fn slots(self) -> usize {
        match self {
            Container::Equipment => crate::net::account::EQUIP_SLOTS,
            Container::Carry => crate::net::enter::CARRY_SLOTS,
            Container::Cargo => crate::net::account::CARGO_SLOTS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Arrival {
    pub slot: u32,
    pub item: Item,
}

impl Arrival {
    pub fn gold_from(high: i32, low: i32) -> u32 {
        (u32::from(high as u8) << 8) + u32::from(low as u8)
    }

    pub fn to_payload(&self) -> [u8; ARRIVED_LEN] {
        let mut out = [0u8; ARRIVED_LEN];

        out[at::ARRIVED_SLOT..][..4].copy_from_slice(&self.slot.to_le_bytes());

        out[at::ARRIVED_ITEM..][..ITEM_LEN].copy_from_slice(&self.item.to_bytes());

        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Clear {
    pub container: Option<Container>,
    pub slot: u32,
    pub raw_container: u32,
}

impl Clear {
    pub fn clears(&self) -> bool {
        self.container.is_some()
    }

    pub fn cargo_page(&self) -> bool {
        self.container == Some(Container::Cargo)
    }

    pub fn cargo_cell(&self) -> Option<(i32, i32)> {
        if !self.cargo_page() {
            return None;
        }

        let slot = self.slot as i32;

        Some((slot / CARGO_ROW, slot % CARGO_ROW))
    }

    pub fn to_payload(&self) -> [u8; CLEARED_LEN] {
        let mut out = [0u8; CLEARED_LEN];

        out[at::CLEARED_CONTAINER..][..4].copy_from_slice(&self.raw_container.to_le_bytes());

        out[at::CLEARED_SLOT..][..4].copy_from_slice(&self.slot.to_le_bytes());

        out
    }
}

pub const CARGO_ROW: i32 = 5;

pub const CARGO_CONTROL: u32 = 0x1009A;

pub const PET_MIRROR_SLOT: usize = 13;

pub fn parse_arrival(payload: &[u8]) -> Result<Arrival> {
    if payload.len() < ARRIVED_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: at::ARRIVED_ITEM,
            needed: ARRIVED_LEN,
            len: payload.len(),
        });
    }

    Ok(Arrival {
        slot: u32_at(payload, at::ARRIVED_SLOT, KIND)?,
        item: item_at(payload, at::ARRIVED_ITEM)?,
    })
}

pub fn parse_clear(payload: &[u8]) -> Result<Clear> {
    if payload.len() < CLEARED_READ_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: at::CLEARED_CONTAINER,
            needed: CLEARED_READ_LEN,
            len: payload.len(),
        });
    }

    let raw_container = u32_at(payload, at::CLEARED_CONTAINER, KIND)?;

    Ok(Clear {
        container: Container::from_dword(raw_container),
        slot: u32_at(payload, at::CLEARED_SLOT, KIND)?,
        raw_container,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Delta {
    Arrived(Arrival),
    Cleared(Clear),
}

impl Delta {
    pub fn opcode(&self) -> u16 {
        match self {
            Delta::Arrived(_) => ARRIVED,
            Delta::Cleared(_) => CLEARED,
        }
    }
}

pub fn parse(opcode: u16, payload: &[u8]) -> Result<Delta> {
    match opcode {
        ARRIVED => parse_arrival(payload).map(Delta::Arrived),
        CLEARED => parse_clear(payload).map(Delta::Cleared),
        other => Err(Error::structure(
            KIND,
            format!("opcode {other:#06X} is not a backpack delta"),
        )),
    }
}
