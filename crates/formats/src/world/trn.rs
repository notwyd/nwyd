use super::{IMPASSABLE, TRN_DIM, ZONE_CELLS};
use crate::error::{Error, Kind, Result};

const CELL_LEN: usize = 12;

const CELL_COUNT: usize = TRN_DIM * TRN_DIM;

const PAYLOAD_LEN: usize = CELL_COUNT * CELL_LEN;

pub const BASE_TILE_ID_BASE: i32 = 10;

pub const DETAIL_TILE_ID_BASE: i32 = 1024;

pub const HEIGHT_GRID_DIM: usize = TRN_DIM * 2;

pub const NEIGHBOUR_CLAMP: usize = 62;

pub const NORMAL_UP: [f32; 3] = [0.0, 1.0, 0.0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrnCell {
    pub height: i8,
    pub uv_mode: u8,
    pub uv_mode2: u8,
    pub pad: u8,
    pub tile0: i16,
    pub tile1: i16,
    pub color: [u8; 4],
}

impl TrnCell {
    pub fn base_texture_id(&self) -> i32 {
        self.tile0 as i32 + BASE_TILE_ID_BASE
    }

    pub fn detail_texture_id(&self) -> i32 {
        self.tile1 as i32 + DETAIL_TILE_ID_BASE
    }

    pub fn rgba(&self) -> [u8; 4] {
        [self.color[2], self.color[1], self.color[0], 0xFF]
    }

    pub fn is_sane(&self) -> bool {
        (0..=273).contains(&self.tile0)
            && (0..=139).contains(&self.tile1)
            && self.uv_mode < BASE_UV_MODES
            && self.uv_mode2 < DETAIL_UV_MODES
    }
}

pub const BASE_UV_MODES: u8 = 8;

pub const DETAIL_UV_MODES: u8 = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Terrain {
    pub name: String,
    pub zone_x: u8,
    pub zone_y: u8,
    pub cells: Vec<TrnCell>,
}

fn err(message: impl Into<String>) -> Error {
    Error::Structure {
        kind: Kind::Trn,
        message: format!(".trn: {}", message.into()),
    }
}

pub fn parse(bytes: &[u8]) -> Result<Terrain> {
    let name_len = *bytes.first().ok_or_else(|| err("file is empty"))? as usize;

    let name_end = 1 + name_len;

    let name_bytes = bytes
        .get(1..name_end)
        .ok_or_else(|| err(format!("name length {name_len} exceeds the file")))?;

    let zone_x = *bytes.get(name_end).ok_or_else(|| err("missing zone_x"))?;

    let zone_y = *bytes
        .get(name_end + 1)
        .ok_or_else(|| err("missing zone_y"))?;

    let cells_start = name_end + 2;

    let cell_data = bytes
        .get(cells_start..cells_start + PAYLOAD_LEN)
        .ok_or(Error::Truncated {
            kind: Kind::Trn,
            offset: cells_start,
            needed: PAYLOAD_LEN,
            len: bytes.len(),
        })?;

    let cells = cell_data
        .chunks_exact(CELL_LEN)
        .map(|c| TrnCell {
            height: c[0] as i8,
            uv_mode: c[1],
            uv_mode2: c[2],
            pad: c[3],
            tile0: i16::from_le_bytes([c[4], c[5]]),
            tile1: i16::from_le_bytes([c[6], c[7]]),
            color: [c[8], c[9], c[10], c[11]],
        })
        .collect();

    Ok(Terrain {
        name: String::from_utf8_lossy(name_bytes).into_owned(),
        zone_x,
        zone_y,
        cells,
    })
}

impl Terrain {
    pub fn encoded_len(&self) -> usize {
        3 + self.name.len() + PAYLOAD_LEN
    }

    pub fn cell(&self, row: usize, col: usize) -> Option<&TrnCell> {
        if row >= TRN_DIM || col >= TRN_DIM {
            return None;
        }

        self.cells.get(row * TRN_DIM + col)
    }

    pub fn set_light(&mut self, x: f32, z: f32, argb: u32) -> bool {
        let (col, row) = (x as i32 / 2, z as i32 / 2);

        let (Ok(col), Ok(row)) = (u32::try_from(col), u32::try_from(row)) else {
            return false;
        };

        let (col, row) = (col as usize, row as usize);

        if col >= TRN_DIM || row >= TRN_DIM {
            return false;
        }

        let Some(cell) = self.cells.get_mut(row * TRN_DIM + col) else {
            return false;
        };

        cell.color = argb.to_le_bytes();

        true
    }

    pub fn contains(&self, x: f32, z: f32) -> bool {
        let (ox, oz) = self.origin();

        let span = ZONE_CELLS as f32;

        (ox..ox + span).contains(&x) && (oz..oz + span).contains(&z)
    }

    pub fn origin(&self) -> (f32, f32) {
        super::zone_origin(u32::from(self.zone_x), u32::from(self.zone_y))
    }

    pub fn build_height_grid(&self) -> Vec<i8> {
        let h = self.patched_heights();

        let at = |r: usize, c: usize| h[r * TRN_DIM + c] as i32;

        let next = |i: usize| if i < NEIGHBOUR_CLAMP { i + 1 } else { i };

        let mut grid = vec![0i8; HEIGHT_GRID_DIM * HEIGHT_GRID_DIM];

        for cy in 0..TRN_DIM {
            let (r0, r1) = (cy, next(cy));

            for cx in 0..TRN_DIM {
                let (c0, c1) = (cx, next(cx));

                let h00 = at(r0, c0);

                let h10 = at(r0, c1);

                let h01 = at(r1, c0);

                let h11 = at(r1, c1);

                let sum = h00 + h10 + h01 + h11;

                let mut put = |y: usize, x: usize, own: i32| {
                    grid[y * HEIGHT_GRID_DIM + x] = ((4 * own + sum) / 8) as i8;
                };

                put(2 * cy, 2 * cx, h00);

                put(2 * cy, 2 * cx + 1, h10);

                put(2 * cy + 1, 2 * cx, h01);

                put(2 * cy + 1, 2 * cx + 1, h11);
            }
        }

        grid
    }

    pub fn color_at(&self, x: f32, z: f32) -> [f32; 4] {
        let (cx, cz) = (x as i32 / 2, z as i32 / 2);

        let (cx, cz) = match (u32::try_from(cx), u32::try_from(cz)) {
            (Ok(cx), Ok(cz)) => (cx as usize, cz as usize),
            _ => return [1.0, 1.0, 1.0, 1.0],
        };

        let last = TRN_DIM - 1;

        let corners = if cx < last && cz < last {
            [(cx, cz), (cx + 1, cz), (cx, cz + 1), (cx + 1, cz + 1)]
        } else if cx == last {
            [(last, cz), (last, cz), (last, cz + 1), (last, cz + 1)]
        } else if cz == last {
            [(cx, last), (cx + 1, last), (cx, last), (cx + 1, last)]
        } else {
            return [1.0, 1.0, 1.0, 1.0];
        };

        let (u, v) = (2.0 * cx as f32 - x, 2.0 * cz as f32 - z);

        let weights = [4.0 - u - v, u + 2.0 - v, 2.0 - u + v, u + v];

        let mut out = [0.0f32; 3];

        for ((col, row), weight) in corners.into_iter().zip(weights) {
            let Some(cell) = self.cell(row, col) else {
                continue;
            };

            let [r, g, b, _] = cell.rgba();

            for (channel, byte) in out.iter_mut().zip([r, g, b]) {
                *channel += f32::from(byte) * (1.0 / 256.0) * weight;
            }
        }

        [out[0] / 12.0, out[1] / 12.0, out[2] / 12.0, 1.0]
    }

    pub fn patched_heights(&self) -> Vec<i8> {
        let mut h: Vec<i8> = self.cells.iter().map(|c| c.height).collect();

        for r in 0..TRN_DIM {
            h[r * TRN_DIM] = h[r * TRN_DIM + 1];
        }

        for c in 0..TRN_DIM {
            h[c] = h[TRN_DIM + c];
        }

        h
    }

    pub fn vertex_normals(&self) -> Vec<[f32; 3]> {
        let heights: Vec<i8> = self.cells.iter().map(|c| c.height).collect();

        let last = TRN_DIM - 1;

        let mut normals = vec![[0.0f32; 3]; TRN_DIM * TRN_DIM];

        for row in 1..last {
            for col in 1..last {
                normals[row * TRN_DIM + col] = vertex_normal(&heights, col, row);
            }
        }

        for row in 0..TRN_DIM {
            normals[row * TRN_DIM] = normals[row * TRN_DIM + 1];

            normals[row * TRN_DIM + last] = normals[row * TRN_DIM + last - 1];
        }

        for col in 0..TRN_DIM {
            normals[col] = normals[TRN_DIM + col];

            normals[last * TRN_DIM + col] = normals[(last - 1) * TRN_DIM + col];
        }

        normals
    }

    pub fn base_texture_ids(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self
            .cells
            .iter()
            .filter(|c| c.is_sane())
            .map(|c| c.base_texture_id())
            .collect();

        v.sort_unstable();

        v.dedup();

        v
    }

    pub fn detail_texture_ids(&self) -> Vec<i32> {
        let mut v: Vec<i32> = self
            .cells
            .iter()
            .filter(|c| c.is_sane())
            .map(|c| c.detail_texture_id())
            .collect();

        v.sort_unstable();

        v.dedup();

        v
    }

    pub fn insane_cell_count(&self) -> usize {
        self.cells.iter().filter(|c| !c.is_sane()).count()
    }

    pub fn is_impassable(grid: &[i8], x: usize, y: usize) -> bool {
        grid.get(y * HEIGHT_GRID_DIM + x)
            .is_some_and(|&h| h as u8 == IMPASSABLE)
    }
}

fn face_normal(p: [f32; 3], a: [f32; 3], b: [f32; 3]) -> [f32; 3] {
    let u = [a[0] - p[0], a[1] - p[1], a[2] - p[2]];

    let v = [b[0] - p[0], b[1] - p[1], b[2] - p[2]];

    let n = [
        u[1] * v[2] - u[2] * v[1],
        u[2] * v[0] - u[0] * v[2],
        u[0] * v[1] - u[1] * v[0],
    ];

    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();

    [n[0] / length, n[1] / length, n[2] / length]
}

pub fn vertex_normal(heights: &[i8], col: usize, row: usize) -> [f32; 3] {
    let last = TRN_DIM - 1;

    if col == 0 || col >= last || row == 0 || row >= last {
        return NORMAL_UP;
    }

    let at = |c: usize, r: usize| f32::from(heights[r * TRN_DIM + c]);

    let p = [0.0, at(col, row), 0.0];

    let ring = [
        [-1.0, at(col - 1, row), 0.0],
        [0.0, at(col, row + 1), 1.0],
        [1.0, at(col + 1, row), 0.0],
        [0.0, at(col, row - 1), -1.0],
    ];

    let mut sum = [0.0f32; 3];

    for i in 0..ring.len() {
        let face = face_normal(p, ring[i], ring[(i + 1) % ring.len()]);

        for (acc, component) in sum.iter_mut().zip(face) {
            *acc += component;
        }
    }

    sum.map(|component| component * 0.25)
}
