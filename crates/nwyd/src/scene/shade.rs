use super::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use formats::world::trn::Terrain;
use formats::world::{HEIGHT_SCALE, QUAD_SIZE, TRN_DIM};
use glam::Vec3;
use render::billboard::{BillboardDraw, BillboardPass, BillboardUniforms, BillboardVertex};
use render::camera::Matrix4;

pub const MIN_GRID: i32 = 2;

pub const LIFT: f32 = 0.05;

pub const HEIGHT_PREFERENCE: f32 = 0.299999;

pub const HEIGHT_CEILING: i32 = 11;

pub const NEIGHBOUR_CEILING: i8 = 10;

pub const HEIGHT_DIM: usize = TRN_DIM * 2;

pub struct Ground<'a> {
    pub zone: &'a Terrain,
    pub heights: &'a [i8],
    patched: Vec<Vec<i8>>,
    pub neighbours: Vec<&'a Terrain>,
    neighbour_heights: Vec<std::cell::OnceCell<Vec<i8>>>,
    seas: &'a [super::sea::Sea],
}

impl<'a> Ground<'a> {
    pub fn new(zone: &'a Terrain, heights: &'a [i8], neighbours: Vec<&'a Terrain>) -> Ground<'a> {
        let patched = std::iter::once(zone)
            .chain(neighbours.iter().copied())
            .map(Terrain::patched_heights)
            .collect();

        Ground {
            zone,
            heights,
            patched,
            neighbour_heights: neighbours
                .iter()
                .map(|_| std::cell::OnceCell::new())
                .collect(),
            neighbours,
            seas: &[],
        }
    }

    pub fn surface_height(&self, x: f32, z: f32) -> Option<f32> {
        if !x.is_finite() || !z.is_finite() {
            return None;
        }

        let contains = |zone: &Terrain| {
            let (ox, oz) = zone.origin();

            x >= ox && x < ox + 128.0 && z >= oz && z < oz + 128.0
        };

        if contains(self.zone) {
            return Some(super::actors::ground_height(self.heights, self.zone, x, z));
        }

        self.neighbours
            .iter()
            .zip(&self.neighbour_heights)
            .find_map(|(zone, heights)| {
                contains(zone).then(|| {
                    super::actors::ground_height(
                        heights.get_or_init(|| zone.build_height_grid()),
                        zone,
                        x,
                        z,
                    )
                })
            })
    }

    pub fn with_water(mut self, seas: &'a [super::sea::Sea]) -> Self {
        self.seas = seas;

        self
    }

    pub fn water_height(&self, x: f32, z: f32) -> Option<f32> {
        let ground = self.surface_height(x, z)?;

        self.water_above(x, z, ground)
    }

    pub fn water_above(&self, x: f32, z: f32, height: f32) -> Option<f32> {
        if !x.is_finite() || !z.is_finite() || !height.is_finite() {
            return None;
        }

        let zone = std::iter::once(self.zone)
            .chain(self.neighbours.iter().copied())
            .find(|zone| {
                let (ox, oz) = zone.origin();

                x >= ox && x < ox + 128.0 && z >= oz && z < oz + 128.0
            })?;

        let sea = self
            .seas
            .iter()
            .filter(|sea| sea.zone == (zone.zone_x, zone.zone_y))
            .take(10)
            .find(|sea| sea.contains(x, z))?;

        (sea.position.y > height).then_some(sea.position.y)
    }

    fn cell_height(&self, which: usize, row: usize, col: usize) -> i32 {
        self.patched
            .get(which)
            .and_then(|z| z.get(row * TRN_DIM + col))
            .map_or(0, |&h| i32::from(h))
    }

    fn height_at(&self, cell: usize) -> i8 {
        self.heights.get(cell).copied().unwrap_or(0)
    }

    fn smoothed(&self, cell: usize, h: i32) -> i32 {
        if h < HEIGHT_CEILING {
            return h;
        }

        for delta in [HEIGHT_DIM as isize, -(HEIGHT_DIM as isize), 1, -1] {
            let Ok(index) = usize::try_from(cell as isize + delta) else {
                continue;
            };

            let Some(&n) = self.heights.get(index) else {
                continue;
            };

            if n < NEIGHBOUR_CEILING {
                return i32::from(n);
            }
        }

        h
    }
}

#[derive(Debug, Clone, PartialEq)]

pub struct Shade {
    pub grid: i32,
    pub texture: usize,
    pub radius: f32,
    pub colour: u32,
    pub rotation: f32,
    pub cosine_fade: bool,
    pub alpha_only: bool,
    pub(crate) render_with_effects_disabled: bool,
    pub pass: BillboardPass,
    pub visible: bool,
    pub centre: Vec3,
    pub sector: i32,
    pub lifetime_ms: u32,
    pub birth_ms: Option<u32>,
    pub gone: bool,
    pub vertices: Vec<BillboardVertex>,
    pub indices: Vec<u16>,
}

impl Shade {
    pub fn new(grid: i32, texture: usize, colour: u32, centre: Vec3, sector: i32) -> Shade {
        let grid = grid.max(MIN_GRID);

        let stride = grid + 1;

        let mut indices = Vec::with_capacity((grid * grid * 6) as usize);

        for row in 0..grid {
            for col in 0..grid {
                let (a, b) = (stride * row + col, stride * (row + 1) + col);

                indices.extend_from_slice(&[
                    a as u16,
                    b as u16,
                    (a + 1) as u16,
                    b as u16,
                    (b + 1) as u16,
                    (a + 1) as u16,
                ]);
            }
        }

        let vertices = vec![
            BillboardVertex {
                position: [0.0, 0.0, 0.0],
                diffuse: colour,
                uv: [0.0, 0.0]
            };
            (stride * stride) as usize
        ];

        Shade {
            grid,
            texture,
            radius: 1.0,
            colour,
            rotation: 0.0,
            cosine_fade: false,
            alpha_only: false,
            render_with_effects_disabled: false,
            pass: BillboardPass::Additive,
            visible: true,
            centre: Vec3::new(centre.x, 0.0, centre.z),
            sector,
            lifetime_ms: 0,
            birth_ms: None,
            gone: false,
            vertices,
            indices,
        }
    }

    pub fn with_lifetime(mut self, lifetime_ms: u32) -> Shade {
        self.lifetime_ms = lifetime_ms;

        self
    }

    pub fn set_color(&mut self, argb: u32) {
        self.colour = argb;

        for v in &mut self.vertices {
            v.diffuse = argb;
        }
    }

    pub fn origin(centre: f32, grid: i32) -> i32 {
        let half = grid / 2;

        let mut o = -half - (centre * -0.5) as i32;

        let line = |i: i32| (o + i) as f32 * QUAD_SIZE;

        if centre < 0.0 {
            if line(1) - centre > centre - line(0) {
                return o - 1;
            }
        } else if centre > 0.0 && centre - line(1) > line(2) - centre {
            o += 1;
        }

        o
    }

    pub fn place(&mut self, ground: &Ground<'_>) {
        let (cx, cz) = (self.centre.x, self.centre.z);

        let (col0, row0) = (Shade::origin(cx, self.grid), Shade::origin(cz, self.grid));

        let stride = self.grid + 1;

        let span = self.grid as f32 * QUAD_SIZE;

        let (ox, oz) = ground.zone.origin();

        let (rx, rz) = ((cx - ox) as i32, (cz - oz) as i32);

        let (zone_col, zone_row) = (
            i32::from(ground.zone.zone_x) * TRN_DIM as i32,
            i32::from(ground.zone.zone_y) * TRN_DIM as i32,
        );

        for row in 0..stride {
            for col in 0..stride {
                let (vx, vz) = (
                    (col + col0) as f32 * QUAD_SIZE,
                    (row + row0) as f32 * QUAD_SIZE,
                );

                let (a, b) = ((cx - vx) / span, -((cz - vz) / span));

                let (sin, cos) = self.rotation.sin_cos();

                let vertex = &mut self.vertices[(stride * row + col) as usize];

                vertex.position = [vx, 0.0, vz];

                vertex.uv = [-cos * a - sin * b - 0.5, sin * a - cos * b - 0.5];

                vertex.diffuse = self.colour;

                let local = |v: i32| {
                    if v == TRN_DIM as i32 {
                        TRN_DIM as i32 - 1
                    } else {
                        v
                    }
                };

                let (lc, lr) = (local(col + col0 - zone_col), local(row + row0 - zone_row));

                vertex.position[1] = match (u32::try_from(lc), u32::try_from(lr)) {
                    (Ok(lc), Ok(lr)) if (lc as usize) < TRN_DIM && (lr as usize) < TRN_DIM => {
                        let index = (rz as usize)
                            .wrapping_mul(HEIGHT_DIM)
                            .wrapping_add(rx as usize);

                        let h = ground.smoothed(index, i32::from(ground.height_at(index)));

                        let cell = ground.cell_height(0, lr as usize, lc as usize);

                        let (h, cell) = (h as f32 * HEIGHT_SCALE, cell as f32 * HEIGHT_SCALE);

                        if h - cell >= HEIGHT_PREFERENCE {
                            h + LIFT
                        } else {
                            cell + LIFT
                        }
                    }
                    _ => ground
                        .neighbours
                        .iter()
                        .enumerate()
                        .find_map(|(i, n)| {
                            let nc = col + col0 - i32::from(n.zone_x) * TRN_DIM as i32;

                            let nr = row + row0 - i32::from(n.zone_y) * TRN_DIM as i32;

                            let (nc, nr) = (u32::try_from(nc).ok()?, u32::try_from(nr).ok()?);

                            if nc as usize >= TRN_DIM || nr as usize >= TRN_DIM {
                                return None;
                            }

                            let cell = ground.cell_height(i + 1, nr as usize, nc as usize);

                            Some(cell as f32 * HEIGHT_SCALE + LIFT)
                        })
                        .unwrap_or(0.0),
                };
            }
        }
    }

    pub fn triangles(&self) -> usize {
        (self.grid * self.grid * 2) as usize
    }

    fn scene_height(&self, height: f32, scene: u32) -> f32 {
        if scene == 0x7531 && self.pass == BillboardPass::Additive {
            if height > 5.0 { 0.5 } else { height + 0.1 }
        } else {
            height
        }
    }
}

pub const CLASS: Class = Class {
    name: "Shade",
    method_count: 18,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

impl Effect for Shade {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        if self.vertices.is_empty() {
            return;
        }

        if self.lifetime_ms == 0 {
            return;
        }

        let birth = *self.birth_ms.get_or_insert(tick.now_ms);

        let elapsed = tick.now_ms.wrapping_sub(birth);

        if self.lifetime_ms < elapsed {
            self.gone = true;
        }

        let age = elapsed as f32 / self.lifetime_ms as f32;

        let phase = std::f32::consts::PI * age;

        let ramp = if self.cosine_fade {
            (phase * 0.5).cos().abs()
        } else {
            phase.sin().abs()
        };

        let scaled = self
            .colour
            .to_be_bytes()
            .map(|c| (f32::from(c) * ramp).round() as u32 & 0xFF);

        let argb = if self.alpha_only {
            (scaled[0] << 24) | (self.colour & 0xFFFFFF)
        } else {
            scaled.iter().fold(0u32, |acc, c| (acc << 8) | c)
        };

        for v in &mut self.vertices {
            v.diffuse = argb;
        }

        if self.grid > MIN_GRID {
            let count = self.vertices.len();

            for back in [1, 2, self.grid as usize + 2] {
                if let Some(v) = count
                    .checked_sub(back)
                    .and_then(|i| self.vertices.get_mut(i))
                {
                    v.diffuse = 0;
                }
            }
        }
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        if cx.scene.detail == 1 && !self.render_with_effects_disabled {
            return;
        }

        if self.cull(cx.eye, false) == Cull::Rejected {
            out.effect.culled += 1;

            return;
        }

        let Some((array, layer)) = cx.scene.placement(self.texture) else {
            out.effect.unbound += 1;

            return;
        };

        let object = out.effect.objects.len() as u32;

        out.effect.objects.push(
            BillboardUniforms::new(Matrix4::IDENTITY, self.pass)
                .with_layer(layer)
                .with_opaque(cx.scene.alpha_at(self.texture)),
        );

        let base_vertex = out.effect.vertices.len() as u32;

        let first = out.effect.indices.len() as u32;

        out.effect.vertices.extend_from_slice(&self.vertices);

        for vertex in &mut out.effect.vertices[base_vertex as usize..] {
            vertex.position[1] = self.scene_height(vertex.position[1], cx.scene.scene_id);
        }

        if self.lifetime_ms == 0 && self.pass == BillboardPass::Blended {
            let colour = if cx.scene.environment == 3 {
                0xA0A0A0A0
            } else {
                0xFFFFFFFF
            };

            for vertex in &mut out.effect.vertices[base_vertex as usize..] {
                vertex.diffuse = colour;
            }
        }

        out.effect.indices.extend_from_slice(&self.indices);

        out.effect.draws.push(BillboardDraw {
            pass: self.pass,
            object,
            texture: array,
            base_vertex,
            indices: first..out.effect.indices.len() as u32,
        });

        out.effect.shades += 1;

        out.effect.shade_triangles += self.triangles();

        out.effect.submitted(&CLASS);
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        match self.visible {
            true => Cull::Kept,
            false => Cull::Rejected,
        }
    }
}
