use super::{HEIGHT_SCALE, IMPASSABLE, WORLD_CELLS, ZONE_CELLS, ZONE_GRID, trn};
use crate::error::{Error, Kind, Result};

pub const DIM: usize = WORLD_CELLS;

pub const LEN: usize = DIM * DIM;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeightMap<'a> {
    data: &'a [u8],
}

pub fn parse(bytes: &[u8]) -> Result<HeightMap<'_>> {
    if bytes.len() < LEN {
        return Err(Error::Truncated {
            kind: Kind::HeightMap,
            offset: 0,
            needed: LEN,
            len: bytes.len(),
        });
    }

    if bytes.len() > LEN {
        return Err(Error::structure(
            Kind::HeightMap,
            format!("{} bytes, expected exactly {LEN}", bytes.len()),
        ));
    }

    Ok(HeightMap { data: bytes })
}

impl<'a> HeightMap<'a> {
    pub fn raw(&self) -> &'a [u8] {
        self.data
    }

    pub fn at(&self, x: usize, y: usize) -> Option<i8> {
        if x >= DIM || y >= DIM {
            return None;
        }

        self.data.get(y * DIM + x).map(|&h| h as i8)
    }

    pub fn is_impassable(&self, x: usize, y: usize) -> bool {
        self.at(x, y).is_some_and(|h| h as u8 == IMPASSABLE)
    }

    pub fn world_y(&self, x: usize, y: usize) -> Option<f32> {
        match self.at(x, y) {
            Some(h) if h as u8 != IMPASSABLE => Some(h as f32 * HEIGHT_SCALE),
            _ => None,
        }
    }

    pub fn zone_rows(
        &self,
        zone_x: usize,
        zone_y: usize,
    ) -> Option<impl Iterator<Item = &'a [u8]>> {
        if zone_x >= ZONE_GRID || zone_y >= ZONE_GRID {
            return None;
        }

        let (ox, oy) = (zone_x * ZONE_CELLS, zone_y * ZONE_CELLS);

        let data = self.data;

        Some(
            (0..ZONE_CELLS)
                .map(move |r| &data[(oy + r) * DIM + ox..(oy + r) * DIM + ox + ZONE_CELLS]),
        )
    }

    pub fn zone_occupancy(&self, zone_x: usize, zone_y: usize) -> Option<f32> {
        let rows = self.zone_rows(zone_x, zone_y)?;

        let live: usize = rows.map(|r| r.iter().filter(|&&h| h != 0).count()).sum();

        Some(live as f32 / (ZONE_CELLS * ZONE_CELLS) as f32)
    }

    pub fn populated_zones(&self) -> Vec<(usize, usize)> {
        let mut out = Vec::new();

        for zy in 0..ZONE_GRID {
            for zx in 0..ZONE_GRID {
                if self.zone_occupancy(zx, zy).is_some_and(|o| o > 0.0) {
                    out.push((zx, zy));
                }
            }
        }

        out
    }

    pub fn agreement(&self, terrain: &trn::Terrain) -> (usize, usize) {
        let grid = terrain.build_height_grid();

        let Some(rows) = self.zone_rows(terrain.zone_x as usize, terrain.zone_y as usize) else {
            return (0, 0);
        };

        let (mut agree, mut compared) = (0, 0);

        for (y, row) in rows.enumerate() {
            for (x, &here) in row.iter().enumerate() {
                if here == IMPASSABLE {
                    continue;
                }

                compared += 1;

                if grid[y * trn::HEIGHT_GRID_DIM + x] == here as i8 {
                    agree += 1;
                }
            }
        }

        (agree, compared)
    }
}
