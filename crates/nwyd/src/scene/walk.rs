use formats::net::action::Action;
use formats::world::path::{self, HeightField, WAYPOINTS};

#[cfg(test)]
mod tests;

pub const DEFAULT_MS_PER_TILE: u32 = 1000;

pub const TURN_MS: f32 = 200.0;

pub const YAW_EPSILON: f32 = 0.017_453_292;

pub const FULL_FLATTEN_SPEED: f32 = 10.0;

pub const DISOWN_DISTANCE: i32 = 0x22;

#[derive(Debug, Clone)]
pub struct Mover {
    pub turn_origin: [f32; 2],
    position: [f32; 2],
    teleport_from: Option<[f32; 2]>,
    pub waypoints: [[f32; 2]; WAYPOINTS],
    pub count: usize,
    pub cursor: usize,
    pub start_ms: u32,
    pub moving: bool,
    pub server_driven: bool,
    pub move_alpha: f32,
    pub yaw: f32,
    pub yaw_start: f32,
    pub yaw_target: f32,
    pub rot_start_ms: u32,
    pub last_dest: (i32, i32),
    pub dirs: Vec<u8>,
    pub pathless: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Step {
    pub position: [f32; 2],
    pub ground: f32,
    pub index: usize,
    pub arrived: bool,
}

impl Mover {
    pub fn standing(position: [f32; 2], yaw: f32) -> Mover {
        Mover {
            turn_origin: position,
            position,
            teleport_from: None,
            waypoints: [position; WAYPOINTS],
            count: 0,
            cursor: 0,
            start_ms: 0,
            moving: false,
            server_driven: false,
            move_alpha: 0.0,
            yaw,
            yaw_start: yaw,
            yaw_target: yaw,
            rot_start_ms: 0,
            last_dest: (position[0] as i32, position[1] as i32),
            dirs: Vec::new(),
            pathless: false,
        }
    }

    pub fn reset_position(&mut self, position: [f32; 2]) {
        self.position = position;

        self.teleport_from = None;

        self.turn_origin = position;

        self.waypoints = [position; WAYPOINTS];

        self.count = 0;

        self.cursor = 0;

        self.moving = false;

        self.dirs.clear();

        self.last_dest = (position[0] as i32, position[1] as i32);
    }

    pub fn order(
        &mut self,
        field: &HeightField<'_>,
        to: (i32, i32),
        budget: usize,
        now_ms: u32,
        occupied: &[(i32, i32)],
    ) -> bool {
        if to == self.last_dest {
            return false;
        }

        let from = (self.position[0] as i32, self.position[1] as i32);

        let planned = path::plan(field, from, to, budget);

        let mut dirs = planned.dirs.as_slice();

        loop {
            let (points, count) = path::expand(from, dirs);

            let end = (points[count - 1][0] as i32, points[count - 1][1] as i32);

            if count > 1 && occupied.contains(&end) {
                dirs = &dirs[..dirs.len() - 1];

                continue;
            }

            if dirs.is_empty() {
                return false;
            }

            self.waypoints = points;

            self.count = count;

            self.cursor = 0;

            self.last_dest = end;

            self.dirs = dirs.to_vec();

            break;
        }

        self.look_at(self.waypoints[1], now_ms);

        self.yaw_start = self.yaw;

        self.start_ms = now_ms;

        true
    }

    pub fn redirect(
        &mut self,
        field: &HeightField<'_>,
        to: (i32, i32),
        budget: usize,
        now_ms: u32,
        speed: f32,
        occupied: &[(i32, i32)],
    ) -> bool {
        if !self.moving {
            return self.order(field, to, budget, now_ms, occupied);
        }

        if to == self.last_dest {
            return false;
        }

        let here = self.waypoints[self.cursor];

        let ahead = self.waypoints[(self.cursor + 1).min(WAYPOINTS - 1)];

        let from = (ahead[0] as i32, ahead[1] as i32);

        let planned = path::plan(field, from, to, budget);

        let mut dirs = planned.dirs.as_slice();

        let (points, count) = loop {
            let (points, count) = path::expand(from, dirs);

            let end = (points[count - 1][0] as i32, points[count - 1][1] as i32);

            if count > 1 && occupied.contains(&end) {
                dirs = &dirs[..dirs.len() - 1];

                continue;
            }

            if dirs.is_empty() {
                return false;
            }

            break (points, count);
        };

        let mut ring = [[0.0f32; 2]; WAYPOINTS];

        ring[0] = here;

        ring[1..].copy_from_slice(&points[..WAYPOINTS - 1]);

        let dropped = self.cursor as u32;

        self.waypoints = ring;

        self.count = (count + 1).min(WAYPOINTS - 1);

        self.dirs = dirs.to_vec();

        self.last_dest = (points[count - 1][0] as i32, points[count - 1][1] as i32);

        self.cursor = 0;

        self.start_ms = self
            .start_ms
            .wrapping_add(dropped * Mover::ms_per_tile(speed).max(1));

        true
    }

    pub fn look_at(&mut self, target: [f32; 2], now_ms: u32) {
        if target != self.position {
            self.turn_origin = self.position;
        }

        let position = self.waypoints[self.cursor];

        if target == position {
            return;
        }

        let want =
            (target[0] - position[0]).atan2(target[1] - position[1]) + std::f32::consts::FRAC_PI_2;

        self.yaw_target = want;

        let tau = std::f32::consts::TAU;

        if self.yaw < 0.0 {
            self.yaw += tau;
        } else if self.yaw > tau {
            self.yaw -= tau;
        }

        self.yaw_start = self.yaw;

        let d = want - self.yaw;

        if d < -std::f32::consts::PI {
            self.yaw_target = want + tau;
        } else if d > std::f32::consts::PI {
            self.yaw_target = want - tau;
        }

        self.rot_start_ms = now_ms;
    }

    pub fn ms_per_tile(speed: f32) -> u32 {
        if speed > 0.0 {
            (DEFAULT_MS_PER_TILE as f32 / speed) as u32
        } else {
            DEFAULT_MS_PER_TILE
        }
    }

    pub fn advance(&mut self, now_ms: u32, speed: f32, field: &HeightField<'_>) -> Step {
        let ms_per_tile = Mover::ms_per_tile(speed).max(1);

        let elapsed = now_ms.wrapping_sub(self.start_ms);

        let mut index = (elapsed / ms_per_tile) as usize;

        let mut alpha = (elapsed % ms_per_tile) as f32 / ms_per_tile as f32;

        let head = self.count;

        if index < head {
            self.moving = true;
        } else {
            alpha = 1.0;

            self.moving = false;

            index = head;
        }

        self.move_alpha = alpha;

        if head + 1 < WAYPOINTS && self.waypoints[head] != self.waypoints[head + 1] {
            self.count = head + 1;
        }

        if index != self.cursor {
            let next = (index + 1) % WAYPOINTS;

            self.look_at(self.waypoints[next], now_ms);

            self.cursor = index % WAYPOINTS;
        }

        let turn = (now_ms.wrapping_sub(self.rot_start_ms) as f32 / TURN_MS).clamp(0.0, 1.0);

        if (self.yaw - self.yaw_target).abs() > YAW_EPSILON {
            self.yaw = self.yaw_start + (self.yaw_target - self.yaw_start) * turn;
        }

        let a = self.waypoints[index.min(WAYPOINTS - 1)];

        let b = self.waypoints[(index + 1).min(WAYPOINTS - 1)];

        let mut position = [a[0] + (b[0] - a[0]) * alpha, a[1] + (b[1] - a[1]) * alpha];

        let mut ground = {
            let ha = field.ground(a[0] as i32, a[1] as i32);

            let hb = field.ground(b[0] as i32, b[1] as i32);

            ha + (hb - ha) * alpha
        };

        if let Some(from) = self.teleport_from {
            let ratio = elapsed as f32 / 2000.0;

            let ratio = if ratio <= 1.0 {
                ratio
            } else {
                self.teleport_from = None;

                1.0
            };

            let to = [self.last_dest.0 as f32, self.last_dest.1 as f32];

            position = [
                from[0] * (1.0 - ratio) + to[0] * ratio + 0.5,
                from[1] * (1.0 - ratio) + to[1] * ratio + 0.5,
            ];

            let start_ground = field.ground(from[0] as i32, from[1] as i32);

            let end_ground = field.ground(self.last_dest.0, self.last_dest.1);

            ground = start_ground * (1.0 - ratio) + end_ground * ratio;
        }

        self.position = position;

        let arrived = a[0] as i32 == b[0] as i32 && a[1] as i32 == b[1] as i32;

        Step {
            position,
            ground,
            index,
            arrived,
        }
    }

    pub fn accept(
        &mut self,
        field: &HeightField<'_>,
        here: (i32, i32),
        action: &Action,
        budget: usize,
        now_ms: u32,
        remote: bool,
    ) -> bool {
        let (from, to, dirs) = (action.from(), action.to(), action.dirs());

        if dirs.is_empty() {
            self.pathless = true;
        }

        let laid_out = match here == from && !self.pathless {
            true => Some((here, dirs.to_vec())),
            false => self.replan(field, here, from, to, budget, remote),
        };

        let walked = laid_out.is_some();

        if let Some((start, dirs)) = laid_out {
            let (mut points, count) = path::expand(start, &dirs);

            for i in 1..WAYPOINTS {
                let previous = points[i - 1];

                let next = points[i];

                if (field.byte(previous[0] as i32, previous[1] as i32)
                    - field.byte(next[0] as i32, next[1] as i32))
                .abs()
                    > path::HEIGHT_TOLERANCE
                {
                    points[i..].fill(previous);

                    break;
                }
            }

            self.teleport_from = None;

            self.waypoints = points;

            self.count = count;

            self.cursor = 0;

            self.last_dest = to;

            self.dirs = dirs;
        }

        self.look_at(self.waypoints[1], now_ms);

        self.yaw_start = self.yaw;

        self.start_ms = now_ms;

        walked
    }

    fn replan(
        &self,
        field: &HeightField<'_>,
        here: (i32, i32),
        from: (i32, i32),
        to: (i32, i32),
        budget: usize,
        remote: bool,
    ) -> Option<((i32, i32), Vec<u8>)> {
        let far = (from.0 - here.0).abs() >= DISOWN_DISTANCE
            || (from.1 - here.1).abs() >= DISOWN_DISTANCE;

        let mut start = match far {
            true => from,
            false => here,
        };

        let mut planned = path::walk(field, start, to, budget, path::HEIGHT_TOLERANCE);

        if planned.dirs.is_empty() {
            start = from;

            planned = path::walk(
                field,
                start,
                planned.reached,
                budget,
                path::HEIGHT_TOLERANCE,
            );
        }

        if planned.reached == to {
            return Some((start, planned.dirs));
        }

        if !remote {
            return None;
        }

        let planned = path::walk(field, from, to, budget, path::HEIGHT_TOLERANCE);

        match !planned.dirs.is_empty() && planned.reached == to {
            true => Some((from, planned.dirs)),
            false => None,
        }
    }

    pub fn teleport(&mut self, to: (i32, i32), now_ms: u32) -> (i32, i32) {
        let was = (self.position[0] as i32, self.position[1] as i32);

        self.teleport_from = Some([was.0 as f32, was.1 as f32]);

        let centre = [to.0 as f32 + 0.5, to.1 as f32 + 0.5];

        self.waypoints = [centre; WAYPOINTS];

        self.count = 0;

        self.dirs.clear();

        self.cursor = 0;

        self.moving = false;

        self.last_dest = to;

        self.start_ms = now_ms;

        was
    }

    pub fn arrive(&mut self) {
        self.pathless = false;

        self.server_driven = false;
    }

    pub fn flatten(&mut self, index: usize, speed: f32) {
        let at = self.waypoints[index.min(WAYPOINTS - 1)];

        let from = if speed <= FULL_FLATTEN_SPEED {
            (index + 1).min(WAYPOINTS)
        } else {
            0
        };

        for slot in &mut self.waypoints[from..] {
            *slot = at;
        }
    }
}
