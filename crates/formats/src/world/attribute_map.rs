use crate::error::{Error, Kind, Result};

pub const DIM: usize = 1024;

pub const LEN: usize = DIM * DIM;

pub const CELLS_PER_ATTR: usize = 4;

pub mod flags {

    pub const BLOCKED: u8 = 0x01;

    pub const TERRAIN_BLOCKED: u8 = 0x02;

    pub const ROOFED: u8 = 0x08;

    pub const TELEPORT_GATE: u8 = 0x10;

    pub const SAFE_ZONE: u8 = 0x40;

    pub const VALID: u8 = 0x80;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttributeMap {
    data: Vec<u8>,
}

pub fn parse(bytes: &[u8]) -> Result<AttributeMap> {
    if bytes.len() < LEN {
        return Err(Error::Truncated {
            kind: Kind::AttributeMap,
            offset: 0,
            needed: LEN,
            len: bytes.len(),
        });
    }

    Ok(AttributeMap {
        data: bytes[..LEN].to_vec(),
    })
}

impl AttributeMap {
    pub fn at_cell(&self, cell_x: usize, cell_y: usize) -> Option<u8> {
        let (ax, ay) = (cell_x / CELLS_PER_ATTR, cell_y / CELLS_PER_ATTR);

        if ax >= DIM || ay >= DIM {
            return None;
        }

        self.data.get(ay * DIM + ax).copied()
    }

    pub fn at(&self, ax: usize, ay: usize) -> Option<u8> {
        if ax >= DIM || ay >= DIM {
            return None;
        }

        self.at_flat(ay * DIM + ax)
    }

    pub fn at_flat(&self, index: usize) -> Option<u8> {
        self.data.get(index).copied()
    }

    pub fn blocks_movement(&self, cell_x: usize, cell_y: usize) -> bool {
        self.at_cell(cell_x, cell_y)
            .is_some_and(|a| a & flags::BLOCKED != 0 && a & flags::SAFE_ZONE == 0)
    }

    pub fn is_roofed(&self, cell_x: usize, cell_y: usize) -> bool {
        self.at_cell(cell_x, cell_y)
            .is_some_and(|a| a & flags::ROOFED != 0)
    }

    pub fn terrain_blocked(&self, cell_x: usize, cell_y: usize) -> bool {
        self.at_cell(cell_x, cell_y)
            .is_some_and(|a| a & flags::TERRAIN_BLOCKED != 0)
    }

    pub fn stamp_impassable(&self, grid: &mut [i8], origin_x: usize, origin_y: usize, dim: usize) {
        for y in 0..dim {
            for x in 0..dim {
                let (wx, wy) = (origin_x + x, origin_y + y);

                let ax = (wx / CELLS_PER_ATTR) & (DIM - 1);

                let ay = (wy / CELLS_PER_ATTR) & (DIM - 1);

                if self.data[ay * DIM + ax] & flags::TERRAIN_BLOCKED != 0
                    && let Some(cell) = grid.get_mut(y * dim + x)
                {
                    *cell = super::IMPASSABLE as i8;
                }
            }
        }
    }
}
