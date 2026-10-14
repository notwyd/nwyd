use super::{
    effects::{Billboard, Motion},
    rng::Lcg,
};
use formats::world::trn::Terrain;
use glam::Vec3;
use render::{
    billboard::BillboardPass,
    cull::{Aabb, Frustum},
};

#[derive(Debug, Clone)]
struct Cell {
    at: Vec3,
    paired: bool,
}

#[derive(Debug)]
struct Zone {
    key: (u8, u8),
    cells: Vec<Cell>,
    paired_ms: u32,
    frame_ms: Option<u32>,
}

#[derive(Debug, Default)]
pub struct State {
    zones: Vec<Zone>,
}

#[derive(Debug, Default)]
pub struct Emissions {
    pub billboards: Vec<Billboard>,
    pub fires: Vec<Vec3>,
}

impl State {
    pub fn include(&mut self, terrain: &Terrain) {
        let key = (terrain.zone_x, terrain.zone_y);

        if self.zones.iter().any(|zone| zone.key == key) {
            return;
        }

        let cells = if super::world::animated_tiles_enabled(key.0, key.1) {
            terrain
                .cells
                .iter()
                .enumerate()
                .filter_map(|(i, cell)| {
                    if !matches!(cell.base_texture_id(), 170 | 171) {
                        return None;
                    }

                    let (x, z) = (i % 64, i / 64);

                    Some(Cell {
                        at: Vec3::new(
                            key.0 as f32 * 128.0 + x as f32 * 2.0 + 0.5,
                            cell.height as f32 * 0.1 + 1.5,
                            key.1 as f32 * 128.0 + z as f32 * 2.0 + 0.5,
                        ),
                        paired: x % 2 == 0 && z % 3 == 0,
                    })
                })
                .collect()
        } else {
            Vec::new()
        };

        self.zones.push(Zone {
            key,
            cells,
            paired_ms: 0,
            frame_ms: None,
        });
    }

    pub fn tick(&mut self, now: u32, detail: bool, frustum: &Frustum, rng: &mut Lcg) -> Emissions {
        let mut out = Emissions::default();

        for zone in &mut self.zones {
            if zone.frame_ms == Some(now) {
                continue;
            }

            zone.frame_ms = Some(now);

            let paired = now.wrapping_sub(zone.paired_ms) > 2000;

            if paired {
                zone.paired_ms = now;
            }

            if !detail {
                continue;
            }

            for cell in &zone.cells {
                let bounds = Aabb {
                    min: (cell.at - Vec3::new(6.0, 3.0, 6.0)).to_array(),
                    max: (cell.at + Vec3::new(6.0, 5.0, 6.0)).to_array(),
                };

                if !frustum.admits(&bounds) {
                    continue;
                }

                for _ in 0..4 {
                    emit(cell, paired, rng, &mut out);
                }
            }
        }

        out
    }
}

fn emit(cell: &Cell, paired: bool, rng: &mut Lcg, out: &mut Emissions) {
    let chance = rng.next();

    let mut at = cell.at;

    if chance % 200 < 2 {
        let n = rng.next_mod(10) as f32;

        let mut b = Billboard::new(
            0,
            1000,
            Vec3::new(0.02 + 0.19 * n, 0.02 + 0.6 * n, 0.02 + 0.19 * n),
            0.0001,
            1,
            80,
        );

        at.x += (rng.next_mod(10) as i32 - 5) as f32 * 0.02;

        at.z += (rng.next_mod(10) as i32 - 5) as f32 * 0.02;

        b.position = at;

        b.origin = at;

        b.motion = Motion::Rise;

        b.rise = 0.7;

        b.pass = BillboardPass::Additive;

        b.set_color(0xFFFFAA00);

        out.billboards.push(b);
    }

    if paired && cell.paired && chance % 100 == 0 {
        let n = rng.next_mod(7);

        for (texture, size, colour) in [(56, 0.2, 0xFFFFAA00), (60, 0.07, 0xFFFFFF00)] {
            let mut b = Billboard::new(texture, 20000, Vec3::splat(size), 0.0, 1, 80);

            b.position = at + Vec3::Y * (1.5 + 0.2 * n as f32);

            b.origin = b.position;

            b.motion = Motion::from_field(6 + n % 3);

            b.phase = 1.5 + 0.1 * n as f32;

            b.amp_h = 3.0 + 0.3 * n as f32;

            b.rise = 0.2 + 0.05 * n as f32;

            b.pass = BillboardPass::Additive;

            b.set_color(colour);

            out.billboards.push(b);
        }

        if n < 3 {
            out.fires.push(at);
        }
    }
}
