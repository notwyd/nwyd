use crate::error::{Error, Kind, Result};
use crate::net::account::{self, Item};
use crate::util::{byte_at, u16_at};

const KIND: Kind = Kind::GroundItem;

pub const HEADER_LEN: usize = 0x0C;

pub const DROP: u16 = 0x26E;

pub const PICK_UP: u16 = 0x270;

pub const DROP_FRAME_LEN: usize = 0x24;

pub const DROP_LEN: usize = DROP_FRAME_LEN - HEADER_LEN;

pub const PICK_UP_FRAME_LEN: usize = 0x1C;

pub const PICK_UP_LEN: usize = PICK_UP_FRAME_LEN - HEADER_LEN;

mod at {

    pub const X: usize = 0x00;

    pub const Z: usize = 0x02;

    pub const ENTITY: usize = 0x04;

    pub const ITEM: usize = 0x06;

    pub const DIRECTION: usize = 0x12;

    pub const STATE: usize = 0x13;

    pub const GATE_TAIL: usize = 0x14;

    pub const SOUND: usize = 0x15;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Drop {
    pub x: u16,
    pub z: u16,
    pub entity: u16,
    pub item: Item,
    pub direction: u8,
    pub state: u8,
    pub gate_tail: u8,
    pub sound: u8,
}

impl Drop {
    pub fn plays_sound(&self) -> bool {
        self.sound == 1
    }

    pub fn yaw(&self) -> f32 {
        f32::from(self.direction) * std::f32::consts::FRAC_PI_2
    }

    pub fn to_bytes(self) -> [u8; DROP_LEN] {
        let mut out = [0u8; DROP_LEN];

        out[at::X..][..2].copy_from_slice(&self.x.to_le_bytes());

        out[at::Z..][..2].copy_from_slice(&self.z.to_le_bytes());

        out[at::ENTITY..][..2].copy_from_slice(&self.entity.to_le_bytes());

        out[at::ITEM..][..account::ITEM_LEN].copy_from_slice(&self.item.to_bytes());

        out[at::DIRECTION] = self.direction;

        out[at::STATE] = self.state;

        out[at::GATE_TAIL] = self.gate_tail;

        out[at::SOUND] = self.sound;

        out
    }
}

pub fn parse(payload: &[u8]) -> Result<Drop> {
    if payload.len() < DROP_LEN {
        return Err(Error::structure(
            KIND,
            format!(
                "expected at least {DROP_LEN} bytes, found {}",
                payload.len()
            ),
        ));
    }

    Ok(Drop {
        x: u16_at(payload, at::X, KIND)?,
        z: u16_at(payload, at::Z, KIND)?,
        entity: u16_at(payload, at::ENTITY, KIND)?,
        item: account::item_at(payload, at::ITEM)?,
        direction: byte_at(payload, at::DIRECTION, KIND)?,
        state: byte_at(payload, at::STATE, KIND)?,
        gate_tail: byte_at(payload, at::GATE_TAIL, KIND)?,
        sound: byte_at(payload, at::SOUND, KIND)?,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct PickUp {
    pub kind: u32,
    pub slot: i32,
    pub entity: u16,
    pub x: u16,
    pub z: u32,
}

impl PickUp {
    pub fn to_bytes(self) -> [u8; PICK_UP_LEN] {
        let mut out = [0u8; PICK_UP_LEN];

        out[0x00..0x04].copy_from_slice(&self.kind.to_le_bytes());

        out[0x04..0x08].copy_from_slice(&self.slot.to_le_bytes());

        out[0x08..0x0A].copy_from_slice(&self.entity.to_le_bytes());

        out[0x0A..0x0C].copy_from_slice(&self.x.to_le_bytes());

        out[0x0C..0x10].copy_from_slice(&self.z.to_le_bytes());

        out
    }

    pub fn parse(payload: &[u8]) -> Result<PickUp> {
        if payload.len() < PICK_UP_LEN {
            return Err(Error::structure(
                KIND,
                format!(
                    "expected at least {PICK_UP_LEN} bytes, found {}",
                    payload.len()
                ),
            ));
        }

        Ok(PickUp {
            kind: crate::util::u32_at(payload, 0x00, KIND)?,
            slot: crate::util::u32_at(payload, 0x04, KIND)? as i32,
            entity: u16_at(payload, 0x08, KIND)?,
            x: u16_at(payload, 0x0A, KIND)?,
            z: crate::util::u32_at(payload, 0x0C, KIND)?,
        })
    }
}
