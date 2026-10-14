use super::trn::HEIGHT_GRID_DIM;
use super::{HEIGHT_SCALE, ZONE_CELLS};

pub const RUNTIME_DIM: i32 = 256;

pub const HEIGHT_TOLERANCE: i32 = 8;

pub const STEP_BUDGET: usize = 12;

pub const EXTENDED_STEP_BUDGET: usize = 16;

pub const MAX_DIRS: usize = 23;

pub const WAYPOINTS: usize = 48;

pub const DISTANCE_TABLE: [[i32; 7]; 7] = [
    [0, 1, 2, 3, 4, 5, 6],
    [1, 1, 2, 3, 4, 5, 6],
    [2, 2, 3, 4, 4, 5, 6],
    [3, 3, 4, 4, 5, 5, 6],
    [4, 4, 4, 5, 5, 5, 6],
    [5, 5, 5, 5, 5, 6, 6],
    [6, 6, 6, 6, 6, 6, 6],
];

pub fn tile_distance(from: (i32, i32), to: (i32, i32)) -> i32 {
    let dx = (from.0 - to.0).abs();

    let dz = (from.1 - to.1).abs();

    if dx < 7 && dz < 7 {
        return DISTANCE_TABLE[dz as usize][dx as usize];
    }

    dz.max(dx) + 1
}

pub const SIGHT_TOLERANCE: i32 = 8;

pub const SIGHT_LIMIT: i32 = 0x1E;

pub fn line_of_sight(
    field: &HeightField<'_>,
    from: (i32, i32),
    to: (i32, i32),
    tolerance: i32,
) -> Option<(i32, i32)> {
    if from == to || from.0 == 0 || from.1 == 0 || to.0 == 0 || to.1 == 0 {
        return None;
    }

    let (dx, dz) = ((from.0 - to.0).abs(), (from.1 - to.1).abs());

    let distance = tile_distance(from, to);

    if distance <= 1 {
        return None;
    }

    if distance > SIGHT_LIMIT {
        return Some((0, 0));
    }

    let x_major = dz < dx;

    if !x_major && to.1 - from.1 == 0 {
        return None;
    }

    let (major0, major1, minor0, minor1) = match x_major {
        true => (from.0, to.0, from.1, to.1),
        false => (from.1, to.1, from.0, to.0),
    };

    if major1 - major0 == 0 {
        return None;
    }

    let slope = ((minor1 - minor0) * 1000) / (major1 - major0);

    let step = if major0 < major1 { 1 } else { -1 };

    let cell = |major: i32, minor: i32| match x_major {
        true => (major, minor),
        false => (minor, major),
    };

    let mut major = major0 + step;

    let mut minor_x1000 = (minor0 * 1000 - slope * major0) + slope * major;

    let first = cell(major, minor_x1000 / 1000);

    let mut height = field.at(first.0, first.1);

    if height == i32::from(super::IMPASSABLE) {
        return Some((0, 0));
    }

    if major == major1 {
        return None;
    }

    let mut budget = if x_major { dx } else { dz };

    let start = major;

    loop {
        if major != start {
            let minor = minor_x1000 / 1000;

            let (x, z) = cell(major, minor);

            let (ox, oz) = field.origin();

            if x - ox < 0 || x - ox > 0xFF || z - oz < 0 || z - oz > 0xFF {
                return Some((0, 0));
            }

            let here = field.at(x, z);

            if here == i32::from(super::IMPASSABLE) {
                return Some((0, 0));
            }

            if here > height + tolerance || here < height - tolerance {
                return Some((x, z));
            }

            height = here;
        }

        if budget - 1 < 1 {
            return None;
        }

        budget -= 1;

        minor_x1000 += step * slope;

        major += step;

        if major == major1 {
            return None;
        }
    }
}

pub const CLIFF_DROP: i32 = 30;

#[derive(Debug, Clone, Copy)]

pub struct HeightField<'a> {
    heights: &'a [i8],
    origin: (i32, i32),
    stride: i32,
}

impl<'a> HeightField<'a> {
    pub fn new(heights: &'a [i8], zone: (u8, u8)) -> HeightField<'a> {
        HeightField::laid_over(heights, zone, HEIGHT_GRID_DIM as i32)
    }

    pub fn window(heights: &'a [i8], zone: (u8, u8)) -> HeightField<'a> {
        HeightField::laid_over(heights, zone, RUNTIME_DIM)
    }

    fn laid_over(heights: &'a [i8], zone: (u8, u8), stride: i32) -> HeightField<'a> {
        HeightField {
            heights,
            origin: (
                i32::from(zone.0) * ZONE_CELLS as i32,
                i32::from(zone.1) * ZONE_CELLS as i32,
            ),
            stride,
        }
    }

    pub fn origin(&self) -> (i32, i32) {
        self.origin
    }

    pub fn at(&self, x: i32, z: i32) -> i32 {
        let (i, j) = (x - self.origin.0, z - self.origin.1);

        if i < 0 || j < 0 || i >= self.stride || j >= self.stride {
            return 0;
        }

        let index = j as usize * self.stride as usize + i as usize;

        i32::from(self.heights.get(index).copied().unwrap_or(0))
    }

    pub fn byte(&self, x: i32, z: i32) -> i32 {
        self.at(
            self.origin.0 + (x - self.origin.0).clamp(0, RUNTIME_DIM - 1),
            self.origin.1 + (z - self.origin.1).clamp(0, RUNTIME_DIM - 1),
        )
    }

    pub fn ground(&self, x: i32, z: i32) -> f32 {
        self.byte(x, z) as f32 * HEIGHT_SCALE
    }

    fn inside(&self, x: i32, z: i32) -> bool {
        let (i, j) = (x - self.origin.0, z - self.origin.1);

        i >= 1 && j >= 1 && i <= RUNTIME_DIM - 2 && j <= RUNTIME_DIM - 2
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]

pub struct Path {
    pub dirs: Vec<u8>,
    pub reached: (i32, i32),
}

impl Path {
    pub fn moved(&self, from: (i32, i32)) -> bool {
        self.reached != from
    }
}

pub fn delta(dir: u8) -> (i32, i32) {
    match dir {
        b'1' => (-1, -1),
        b'2' => (0, -1),
        b'3' => (1, -1),
        b'4' => (-1, 0),
        b'6' => (1, 0),
        b'7' => (-1, 1),
        b'8' => (0, 1),
        b'9' => (1, 1),
        _ => (0, 0),
    }
}

pub fn walk(
    field: &HeightField<'_>,
    from: (i32, i32),
    to: (i32, i32),
    max_steps: usize,
    tol: i32,
) -> Path {
    let (mut cx, mut cz) = from;

    let (tx, tz) = to;

    let mut dirs: Vec<u8> = Vec::new();

    for i in 0..max_steps {
        if i > MAX_DIRS - 1 {
            break;
        }

        if !field.inside(cx, cz) {
            break;
        }

        let h = field.at(cx, cz);

        let ok = |v: i32| v < h + tol && h - tol < v;

        let n = ok(field.at(cx, cz - 1));

        let ne = ok(field.at(cx + 1, cz - 1));

        let nw = ok(field.at(cx - 1, cz - 1));

        let e = ok(field.at(cx + 1, cz));

        let w = ok(field.at(cx - 1, cz));

        let se = ok(field.at(cx + 1, cz + 1));

        let s = ok(field.at(cx, cz + 1));

        let sw = ok(field.at(cx - 1, cz + 1));

        let mut step = if tx == cx {
            if tz == cz {
                break;
            } else if cz < tz && s {
                Some(b'8')
            } else if cz > tz && n {
                Some(b'2')
            } else {
                None
            }
        } else {
            None
        };

        if step.is_none() && cx < tx {
            step = if cz > tz && ne && (n || e) {
                Some(b'3')
            } else if tz == cz && e {
                Some(b'6')
            } else if tz > cz && se && (s || e) {
                Some(b'9')
            } else {
                None
            };
        }

        if step.is_none() && tx < cx {
            step = if cz < tz && sw && (s || w) {
                Some(b'7')
            } else if tz == cz && w {
                Some(b'4')
            } else if cz > tz && nw && (n || w) {
                Some(b'1')
            } else {
                None
            };
        }

        if step.is_none() && cx < tx {
            step = if tz < cz {
                if e {
                    Some(b'6')
                } else if n {
                    Some(b'2')
                } else {
                    None
                }
            } else if tz > cz {
                if e {
                    Some(b'6')
                } else if s {
                    Some(b'8')
                } else {
                    None
                }
            } else {
                None
            };
        }

        if step.is_none() && tx < cx {
            step = if cz < tz {
                if w {
                    Some(b'4')
                } else if s {
                    Some(b'8')
                } else {
                    None
                }
            } else if cz > tz {
                if w {
                    Some(b'4')
                } else if n {
                    Some(b'2')
                } else {
                    None
                }
            } else {
                None
            };
        }

        if step.is_none() {
            if tx == cx + 1 || tz == cz + 1 || tx == cx - 1 || tz == cz - 1 {
                break;
            }

            step = if tx == cx {
                if tz > cz {
                    if se && (s || e) {
                        Some(b'9')
                    } else if sw && (s || w) {
                        Some(b'7')
                    } else {
                        None
                    }
                } else if ne && (n || e) {
                    Some(b'3')
                } else if nw && (n || w) {
                    Some(b'1')
                } else {
                    None
                }
            } else if tz == cz {
                if tx < cx {
                    if sw && (s || w) {
                        Some(b'7')
                    } else if nw && (n || w) {
                        Some(b'1')
                    } else {
                        None
                    }
                } else if se && (s || e) {
                    Some(b'9')
                } else if ne && (n || e) {
                    Some(b'3')
                } else {
                    None
                }
            } else {
                None
            };

            if step.is_none() {
                break;
            }
        }

        let Some(dir) = step else {
            break;
        };

        let (dx, dz) = delta(dir);

        dirs.push(dir);

        cx += dx;

        cz += dz;
    }

    Path {
        dirs,
        reached: (cx, cz),
    }
}

pub fn plan(field: &HeightField<'_>, from: (i32, i32), to: (i32, i32), max_steps: usize) -> Path {
    let mut path = walk(field, from, to, max_steps, HEIGHT_TOLERANCE);

    if path.reached != to {
        path = walk(field, from, to, max_steps, HEIGHT_TOLERANCE);
    }

    let start = field.at(from.0, from.1);

    if (field.at(path.reached.0, path.reached.1) - start).abs() > CLIFF_DROP {
        path = walk(field, from, to, max_steps / 2, HEIGHT_TOLERANCE);

        if (field.at(path.reached.0, path.reached.1) - start).abs() > CLIFF_DROP {
            path = walk(field, from, to, max_steps / 4, HEIGHT_TOLERANCE);
        }
    }

    path
}

pub fn expand(from: (i32, i32), dirs: &[u8]) -> ([[f32; 2]; WAYPOINTS], usize) {
    let mut out = [[0.0f32; 2]; WAYPOINTS];

    out[0] = [from.0 as f32 + 0.5, from.1 as f32 + 0.5];

    for i in 1..WAYPOINTS {
        let (dx, dz) = dirs.get(i - 1).map_or((0, 0), |&d| delta(d));

        out[i] = [out[i - 1][0] + dx as f32, out[i - 1][1] + dz as f32];
    }

    (out, dirs.len() + 1)
}
