use crate::error::{Kind as ErrorKind, Result};
use crate::util::{u16_at, u32_at};

const CARPET_KIND_ERR: ErrorKind = ErrorKind::FieldFx;

pub const CARPET: u16 = 0x3A2;

pub const SOUND: u16 = 0x3A3;

pub mod at {

    pub const X0: usize = 0x00;

    pub const Z0: usize = 0x02;

    pub const X1: usize = 0x04;

    pub const Z1: usize = 0x06;

    pub const KIND: usize = 0x08;

    pub const SLOT: usize = 0x00;
}

pub const CARPET_KIND: i16 = 0x20;

pub const STEP_X: i16 = 4;

pub const STEP_Z: i16 = 3;

pub const X_LEAD_IN: i16 = 1;

pub const Z_LIFT: f32 = 1.0;

pub const HEIGHT_SCALE: f32 = 0.1;

pub const RADIUS: f32 = 1.5;

pub const INTERVAL_MS: u32 = 0xD2;

pub const TINT: u32 = 0x4444_4444;

pub const OBJECT_SIZE: usize = 0xA8;

pub const CARPET_LEN: usize = 0x0A;

pub const SOUND_LEN: usize = 0x04;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Carpet {
    pub x0: i16,
    pub z0: i16,
    pub x1: i16,
    pub z1: i16,
    pub kind: i16,
}

impl Carpet {
    pub fn builds(self) -> bool {
        self.kind == CARPET_KIND
    }

    pub fn cells(self) -> impl Iterator<Item = (i16, i16)> {
        let (x0, z0, x1, z1) = (self.x0, self.z0, self.x1, self.z1);

        let rows: Vec<i16> = match x0 > x1 || z0 >= z1 {
            true => Vec::new(),
            false => std::iter::successors(Some(z0), |z| z.checked_add(STEP_Z))
                .take_while(|&z| z < z1)
                .collect(),
        };

        rows.into_iter().flat_map(move |z| {
            std::iter::successors(x0.checked_add(X_LEAD_IN), |x| x.checked_add(STEP_X))
                .take_while(move |&x| x < x1)
                .map(move |x| (x, z))
        })
    }

    pub fn cell_position(cell: (i16, i16)) -> (f32, f32) {
        (f32::from(cell.0), f32::from(cell.1) + Z_LIFT)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sound {
    pub slot: u32,
}

pub fn parse_carpet(payload: &[u8]) -> Result<Carpet> {
    Ok(Carpet {
        x0: u16_at(payload, at::X0, CARPET_KIND_ERR)? as i16,
        z0: u16_at(payload, at::Z0, CARPET_KIND_ERR)? as i16,
        x1: u16_at(payload, at::X1, CARPET_KIND_ERR)? as i16,
        z1: u16_at(payload, at::Z1, CARPET_KIND_ERR)? as i16,
        kind: u16_at(payload, at::KIND, CARPET_KIND_ERR)? as i16,
    })
}

pub fn parse_sound(payload: &[u8]) -> Result<Sound> {
    Ok(Sound {
        slot: u32_at(payload, at::SLOT, CARPET_KIND_ERR)?,
    })
}

pub fn carpet_bytes(c: Carpet) -> [u8; CARPET_LEN] {
    let mut out = [0u8; CARPET_LEN];

    out[at::X0..at::X0 + 2].copy_from_slice(&c.x0.to_le_bytes());

    out[at::Z0..at::Z0 + 2].copy_from_slice(&c.z0.to_le_bytes());

    out[at::X1..at::X1 + 2].copy_from_slice(&c.x1.to_le_bytes());

    out[at::Z1..at::Z1 + 2].copy_from_slice(&c.z1.to_le_bytes());

    out[at::KIND..at::KIND + 2].copy_from_slice(&c.kind.to_le_bytes());

    out
}
