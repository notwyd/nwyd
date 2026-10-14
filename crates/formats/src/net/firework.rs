use crate::error::{Error, Kind, Result};
use crate::net::ground_item::HEADER_LEN;

const KIND: Kind = Kind::Firework;

pub const OPCODE: u16 = 0x3CA;

pub const FRAME_LEN: usize = 0x24;

pub const LEN: usize = FRAME_LEN - HEADER_LEN;

pub const CELLS: usize = 100;

pub const SIDE: usize = 10;

pub const MASK_BYTES: usize = CELLS.div_ceil(8);

pub mod at {

    pub const MASK: usize = 0x08;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glyph {
    pub cells: [bool; CELLS],
}

impl Default for Glyph {
    fn default() -> Self {
        Glyph {
            cells: [false; CELLS],
        }
    }
}

impl Glyph {
    pub fn lit(&self) -> usize {
        let n = self.cells.iter().filter(|c| **c).count();

        if n == 0 { 1 } else { n }
    }

    pub fn forced(&self) -> bool {
        !self.cells.iter().any(|c| *c)
    }

    pub fn walked(&self) -> [bool; CELLS] {
        let mut cells = self.cells;

        if self.forced() {
            cells[0] = true;
        }

        cells
    }

    pub fn per_cell(&self) -> usize {
        CELLS / self.lit()
    }

    pub fn placed(&self) -> usize {
        self.per_cell() * self.lit()
    }

    pub fn grid(row: usize, col: usize) -> (i32, i32) {
        (5 - row as i32, 4 - col as i32)
    }

    pub fn to_bytes(&self) -> [u8; LEN] {
        let mut out = [0u8; LEN];

        for (i, lit) in self.cells.iter().enumerate() {
            if *lit {
                out[at::MASK + i / 8] |= 1 << (i % 8);
            }
        }

        out
    }
}

pub fn parse(payload: &[u8]) -> Result<Glyph> {
    if payload.len() < LEN {
        return Err(Error::structure(
            KIND,
            format!("expected at least {LEN} bytes, found {}", payload.len()),
        ));
    }

    let mut cells = [false; CELLS];

    for (i, cell) in cells.iter_mut().enumerate() {
        *cell = payload[at::MASK + i / 8] >> (i % 8) & 1 == 1;
    }

    Ok(Glyph { cells })
}
