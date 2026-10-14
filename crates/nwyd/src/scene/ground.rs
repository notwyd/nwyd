use super::pick::Ray;
use formats::world::trn::{HEIGHT_GRID_DIM, Terrain};
use formats::world::{HEIGHT_SCALE, IMPASSABLE, TRN_DIM, ZONE_CELLS};
use glam::{Mat4, Vec3};

pub const SEARCH_RADIUS: i32 = 25;

pub const WIDE_PITCH: f32 = 1.0;

pub const WIDE_DIST_SCALE: f32 = 1.5;

pub const WIDE_DIST_BIAS: f32 = 8.0;

pub const IMPASSABLE_HEIGHT: i32 = 400;

pub const COARSE_PITCH: f32 = 0.2;

pub const MISS: Vec3 = Vec3::new(0.0, -10_000.0, 0.0);

pub const MISS_FLOOR: f32 = -5_000.0;

pub const NEAR_ENOUGH: f32 = 4.0;

pub const EXACT_ENOUGH: f32 = 2.0;

pub const NO_TARGET_Y: f32 = -9_000.0;

#[derive(Debug, Clone, Copy)]

pub struct Block<'a> {
    pub zone: (u8, u8),
    pub heights: &'a [i8],
}

pub struct Coarse<'a> {
    pub terrains: &'a [&'a Terrain],
    pub view: Mat4,
    pub projection: Mat4,
    pub viewport: (u32, u32),
    pub automatic_movement: bool,
}

impl Coarse<'_> {
    fn visible(&self, vertex: Vec3) -> bool {
        if self.viewport.0 == 0 || self.viewport.1 == 0 {
            return false;
        }

        let ndc = self
            .projection
            .project_point3(self.view.transform_point3(vertex));

        if !ndc.is_finite() || !(0.0..1.0).contains(&ndc.z) {
            return false;
        }

        let (w, h) = (self.viewport.0 as f32, self.viewport.1 as f32);

        let x = ((ndc.x + 1.0) * w * 0.5).trunc();

        let y = ((1.0 - ndc.y) * h * 0.5).trunc();

        let (mx, my) = (100.0 * w / 800.0, 100.0 * h / 600.0);

        -mx < x && x < w + mx && -my < y && y < h + my
    }
}

pub fn scan_coarse(
    block: Block<'_>,
    ray: &Ray,
    centre: (i32, i32),
    context: &Coarse<'_>,
) -> Option<Vec3> {
    let terrain = context
        .terrains
        .iter()
        .find(|t| (t.zone_x, t.zone_y) == block.zone)?;

    let range = |c: i32| {
        let end = c / 2 + 12;

        (c / 2 - 12).max(0)..if end > 64 { 63 } else { end }
    };

    let (ox, oz) = origin(block.zone);

    let height = |x: usize, z: usize| -> Option<f32> {
        let index = z.checked_mul(TRN_DIM)?.checked_add(x)?;

        let (x, z) = (index % TRN_DIM, index / TRN_DIM);

        terrain
            .cell(z.max(1), x.max(1))
            .map(|c| c.height as f32 * HEIGHT_SCALE)
    };

    for z in range(centre.1) {
        for x in range(centre.0) {
            let (x, z) = (usize::try_from(x).ok()?, usize::try_from(z).ok()?);

            let (Some(h0), Some(h1), Some(h2), Some(h3)) = (
                height(x, z),
                height(x, z + 1),
                height(x + 1, z),
                height(x + 1, z + 1),
            ) else {
                continue;
            };

            let Some(fine) = block.heights.get(z * HEIGHT_GRID_DIM + x) else {
                continue;
            };

            if (*fine as f32 * HEIGHT_SCALE - (h0 + h1 + h2 + h3) * 0.25).abs() > 1.0 {
                continue;
            }

            let (x0, z0) = (ox + x as f32 * 2.0, oz + z as f32 * 2.0);

            let v0 = Vec3::new(x0, h0, z0);

            let v1 = Vec3::new(x0, h1, z0 + 2.0);

            let v2 = Vec3::new(x0 + 2.0, h2, z0);

            let v3 = Vec3::new(x0 + 2.0, h3, z0 + 2.0);

            if let Some((u, v)) = barycentric(ray, v0, v1, v2)
                && [v0, v1, v2].into_iter().any(|p| context.visible(p))
            {
                return Some(Vec3::new(x0 + 2.0 * v, h0, z0 + 2.0 * u));
            }

            if let Some((u, v)) = barycentric(ray, v3, v2, v1)
                && [v1, v2, v3].into_iter().any(|p| context.visible(p))
            {
                return Some(Vec3::new(x0 + 2.0 - 2.0 * v, h3, z0 + 2.0 - 2.0 * u));
            }
        }
    }

    None
}

pub fn origin(zone: (u8, u8)) -> (f32, f32) {
    (
        f32::from(zone.0) * ZONE_CELLS as f32,
        f32::from(zone.1) * ZONE_CELLS as f32,
    )
}

pub fn search_box(centre: (i32, i32), radius: i32) -> (std::ops::Range<i32>, std::ops::Range<i32>) {
    let half = radius / 2;

    (
        centre.0 - half..centre.0 + radius,
        centre.1 - half..centre.1 + radius,
    )
}

pub fn radius(pitch: f32, dist: f32) -> i32 {
    if -pitch > WIDE_PITCH {
        (dist * WIDE_DIST_SCALE + WIDE_DIST_BIAS) as i32
    } else {
        SEARCH_RADIUS
    }
}

pub fn tile_height(heights: &[i8], x: i32, z: i32) -> f32 {
    let index = z as usize * HEIGHT_GRID_DIM + x as usize;

    let h = i32::from(heights.get(index).copied().unwrap_or(0));

    let h = if h == i32::from(IMPASSABLE) {
        IMPASSABLE_HEIGHT
    } else {
        h
    };

    h as f32 * HEIGHT_SCALE
}

pub fn scan(
    zone: (u8, u8),
    heights: &[i8],
    ray: &Ray,
    centre: (i32, i32),
    radius: i32,
) -> Option<Vec3> {
    let (origin_x, origin_z) = origin(zone);

    let dim = HEIGHT_GRID_DIM as i32;

    let (xs, zs) = search_box(centre, radius);

    for z in zs {
        if !(0..dim).contains(&z) {
            continue;
        }

        for x in xs.clone() {
            if !(0..dim).contains(&x) {
                continue;
            }

            let y = tile_height(heights, x, z);

            let (x0, z0) = (x as f32 + origin_x, z as f32 + origin_z);

            let v0 = Vec3::new(x0, y, z0);

            let v1 = Vec3::new(x0, y, z0 + 1.0);

            let v2 = Vec3::new(x0 + 1.0, y, z0);

            let v3 = Vec3::new(x0 + 1.0, y, z0 + 1.0);

            if let Some((u, v)) = barycentric(ray, v0, v1, v2) {
                return Some(Vec3::new(x0 + v, y, z0 + u));
            }

            if let Some((u, v)) = barycentric(ray, v3, v2, v1) {
                return Some(Vec3::new(x0 + 1.0 - v, y, z0 + 1.0 - u));
            }
        }
    }

    None
}

fn barycentric(ray: &Ray, v0: Vec3, v1: Vec3, v2: Vec3) -> Option<(f32, f32)> {
    let e1 = v1 - v0;

    let e2 = v2 - v0;

    let p = ray.direction.cross(e2);

    let det = e1.dot(p);

    if det == 0.0 {
        return None;
    }

    let t = ray.origin - v0;

    let u = t.dot(p) / det;

    if !(0.0..=1.0).contains(&u) {
        return None;
    }

    let q = t.cross(e1);

    let v = ray.direction.dot(q) / det;

    (v >= 0.0 && u + v <= 1.0).then_some((u, v))
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Picker {
    pub hit: Vec3,
    pub fresh: bool,
}

impl Default for Picker {
    fn default() -> Picker {
        Picker {
            hit: MISS,
            fresh: false,
        }
    }
}

impl Picker {
    pub fn pick(
        &mut self,
        current: Block<'_>,
        neighbours: &[Option<Block<'_>>; 4],
        ray: &Ray,
        target: Vec3,
        radius: i32,
        pitch: f32,
    ) -> Vec3 {
        self.pick_with_coarse(current, neighbours, ray, target, radius, pitch, None)
    }

    #[allow(
        clippy::too_many_arguments,
        reason = "retains the fine-picker inputs plus optional coarse-pass resources"
    )]

    pub fn pick_with_coarse(
        &mut self,
        current: Block<'_>,
        neighbours: &[Option<Block<'_>>; 4],
        ray: &Ray,
        target: Vec3,
        radius: i32,
        pitch: f32,
        coarse: Option<&Coarse<'_>>,
    ) -> Vec3 {
        let mut out = MISS;

        let player_y = target.y;

        let apart = |v: Vec3| (player_y - v.y).abs();

        let hit = self.probe(current, ray, target, radius, pitch, coarse);

        if apart(hit) < NEAR_ENOUGH {
            out = hit;
        }

        if out.y >= MISS_FLOOR && apart(out) < EXACT_ENOUGH {
            return out;
        }

        if let Some(block) = neighbours[0] {
            let hit = self.probe(block, ray, target, radius, pitch, coarse);

            let d = apart(hit);

            if d < apart(out) || d < NEAR_ENOUGH {
                out = hit;

                if out.y > MISS_FLOOR && apart(out) < EXACT_ENOUGH {
                    return out;
                }
            }
        }

        for side in [1usize, 2] {
            if let Some(block) = neighbours[side] {
                let hit = self.probe(block, ray, target, radius, pitch, coarse);

                let d = apart(hit);

                if d < apart(out) {
                    out = hit;

                    if (out.y > MISS_FLOOR && apart(out) < EXACT_ENOUGH) || d < NEAR_ENOUGH {
                        return out;
                    }
                }
            }
        }

        if let Some(block) = neighbours[3] {
            let hit = self.probe(block, ray, target, radius, pitch, coarse);

            let d = apart(hit);

            if d < apart(out) || d < NEAR_ENOUGH {
                out = hit;
            }
        }

        out
    }

    pub fn probe(
        &mut self,
        block: Block<'_>,
        ray: &Ray,
        target: Vec3,
        radius: i32,
        pitch: f32,
        coarse: Option<&Coarse<'_>>,
    ) -> Vec3 {
        let (origin_x, origin_z) = origin(block.zone);

        let centre = ((target.x - origin_x) as i32, (target.z - origin_z) as i32);

        let sticky = pitch > COARSE_PITCH || coarse.is_some_and(|c| c.automatic_movement);

        let hit = scan(block.zone, block.heights, ray, centre, radius).or_else(|| {
            if sticky {
                return None;
            }

            scan_coarse(block, ray, centre, coarse?)
        });

        let Some(hit) = hit else {
            self.fresh = false;

            return match sticky {
                true => self.hit,
                false => MISS,
            };
        };

        self.hit = hit;

        self.fresh = true;

        hit
    }
}
