use super::actors::Actor;
use super::field::{FieldScene, player_tile};
use super::sky::Sky;
use super::world::is_special_tile;

pub const SLOT_MS: u32 = 600_000;

pub const SLOTS: u32 = 12;

pub const FORCING_CLASS: i32 = 0x43;

pub const FORCE_AFTER_MS: u32 = 100;

pub const FORCED_SLOT: u32 = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]

pub struct Emitters {
    pub rain: bool,
    pub snow: bool,
    pub snow_fast: bool,
}

impl Emitters {
    pub const NONE: Emitters = Emitters {
        rain: false,
        snow: false,
        snow_fast: false,
    };

    pub const RAIN: Emitters = Emitters {
        rain: true,
        snow: false,
        snow_fast: false,
    };

    pub const SNOW: Emitters = Emitters {
        rain: false,
        snow: true,
        snow_fast: false,
    };

    pub const SNOW_HEAVY: Emitters = Emitters {
        rain: false,
        snow: true,
        snow_fast: true,
    };
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Anchor {
    pub x: f32,
    pub z: f32,
    pub floor: f32,
    pub lift_from: Option<f32>,
}

pub const ADRIFT_OFFSET: f32 = 2.5;

pub const LOGIN_X_OFFSET: f32 = 0.5;

pub const LOGIN_Z_OFFSET: f32 = 4.5;

pub const LOGIN_FLOOR_DROP: f32 = 2.0;

impl Anchor {
    pub fn on_target(x: f32, y: f32, z: f32) -> Anchor {
        Anchor {
            x,
            z,
            floor: y,
            lift_from: Some(y),
        }
    }

    pub fn adrift(camera_x: f32, camera_z: f32) -> Anchor {
        Anchor {
            x: camera_x + ADRIFT_OFFSET,
            z: camera_z + ADRIFT_OFFSET,
            floor: 0.0,
            lift_from: None,
        }
    }

    pub fn on_login_camera(eye: [f32; 3], target_y: Option<f32>) -> Anchor {
        let floor = eye[1] - LOGIN_FLOOR_DROP;

        Anchor {
            x: eye[0] - LOGIN_X_OFFSET,
            z: eye[2] + LOGIN_Z_OFFSET,
            floor,
            lift_from: Some(target_y.unwrap_or(floor)),
        }
    }
}

pub fn billboard_axes(view: &render::camera::Matrix4) -> ([f32; 3], [f32; 3]) {
    let m = &view.0;

    ([m[0], m[4], m[8]], [m[1], m[5], m[9]])
}

#[derive(Debug, Clone, Copy)]

pub struct Fall<'a> {
    pub emitters: Emitters,
    pub rain: &'a super::rain::Rain,
    pub snow: [&'a super::snow::Snow; 2],
    pub anchor: Anchor,
    pub map: Option<&'a formats::world::attribute_map::AttributeMap>,
    pub view: &'a render::camera::Matrix4,
}

pub fn login_snow(day_quarter: u32) -> bool {
    (day_quarter + 1) % 4 == 2
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Weather {
    pub code: u32,
    pub emitters: Emitters,
    pub forced: u32,
}

impl Default for Weather {
    fn default() -> Weather {
        Weather::new(0)
    }
}

pub fn slot(now_ms: u32) -> u32 {
    (now_ms / SLOT_MS) % SLOTS
}

pub fn force_due(class: i32, last_ms: &mut u32, now_ms: u32) -> bool {
    if class != FORCING_CLASS || now_ms.wrapping_sub(*last_ms) <= FORCE_AFTER_MS {
        return false;
    }

    *last_ms = now_ms;

    true
}

pub fn force_from_actors(
    actors: &mut [Actor],
    weather: &mut Weather,
    env_global: &mut i32,
    now_ms: u32,
) -> usize {
    let mut fired = 0;

    for actor in actors {
        if !force_due(actor.class, &mut actor.forced_weather_ms, now_ms) {
            continue;
        }

        weather.forced = FORCED_SLOT;

        weather.code = 0;

        *env_global = 1;

        fired += 1;
    }

    fired
}

pub const INDOOR_TILE_BOX: ((i32, i32), (i32, i32)) = ((8, 12), (11, 14));

pub fn is_indoor_tile(tile: (i32, i32)) -> bool {
    let ((x0, x1), (z0, z1)) = INDOOR_TILE_BOX;

    (x0..=x1).contains(&tile.0) && (z0..=z1).contains(&tile.1)
}

pub fn env_global_after(env_global: i32, tile: (i32, i32)) -> i32 {
    let inside = is_indoor_tile(tile);

    match env_global {
        0 if inside => 3,
        3 if !inside => 0,
        other => other,
    }
}

pub fn refuses_weather(tile: (i32, i32)) -> bool {
    matches!(tile.0, 29 | 30) && tile.1 == 22
}

pub fn changed(
    scene: &mut FieldScene,
    forecast: &formats::net::weather::Forecast,
    now_ms: u32,
) -> Commanded {
    let tile = player_tile(scene);

    let env_type = scene.world.env_global;

    scene
        .world
        .weather
        .commanded(forecast.code, &mut scene.world.sky, env_type, tile, now_ms)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Commanded {
    pub was: u32,
    pub code: u32,
    pub refused: bool,
    pub before: Emitters,
    pub after: Emitters,
    pub armed: bool,
}

impl Commanded {
    pub fn cycle_runs(&self) -> bool {
        self.code == 0
    }

    pub fn describe(&self) -> String {
        let sky = match (self.armed, self.cycle_runs()) {
            (true, _) => "sky to day 11",
            (false, true) => "day cycle running",
            (false, false) => "no emitter, day cycle stopped",
        };

        format!(
            "0x18B weather — {} -> {}{}, emitters {:?} -> {:?}, {sky}",
            self.was,
            self.code,
            if self.refused {
                " (zone refuses weather)"
            } else {
                ""
            },
            self.before,
            self.after,
        )
    }
}

impl Weather {
    pub const fn new(code: u32) -> Weather {
        Weather {
            code,
            emitters: Emitters::NONE,
            forced: 0,
        }
    }

    pub fn apply(&mut self, sky: &mut Sky, env_type: i32, tile: (i32, i32), now_ms: u32) -> bool {
        if refuses_weather(tile) {
            return false;
        }

        let indoors = matches!(env_type, 3 | 4);

        let emitters = match (self.code, indoors, is_special_tile(tile)) {
            (2 | 3, true, _) => Emitters::RAIN,
            (1, _, true) => Emitters::SNOW_HEAVY,
            (_, _, true) => Emitters::SNOW,
            (1, _, false) => Emitters::RAIN,
            (2, _, false) => Emitters::SNOW,
            (3, _, false) => Emitters::SNOW_HEAVY,
            _ => return false,
        };

        self.emitters = emitters;

        sky.set_day(11, now_ms);

        true
    }

    pub fn warp(&mut self, sky: &mut Sky, env_type: i32, tile: (i32, i32), now_ms: u32) -> bool {
        if !warp_resets_yaw(env_type) {
            self.code = 0;

            self.emitters = Emitters::NONE;

            return false;
        }

        if matches!(env_type, 3 | 4) {
            self.emitters.snow = false;

            self.emitters.snow_fast = false;
        }

        self.apply(sky, env_type, tile, now_ms);

        true
    }

    pub fn commanded(
        &mut self,
        code: u32,
        sky: &mut Sky,
        env_type: i32,
        tile: (i32, i32),
        now_ms: u32,
    ) -> Commanded {
        let was = self.code;

        let before = self.emitters;

        self.code = code;

        let armed = self.apply(sky, env_type, tile, now_ms);

        Commanded {
            was,
            code,
            refused: refuses_weather(tile),
            before,
            after: self.emitters,
            armed,
        }
    }

    pub fn tick(&mut self, sky: &mut Sky, env_type: i32, tile: (i32, i32), now_ms: u32) {
        self.forced_weather(env_type);

        self.cycle(sky, env_type, tile, now_ms);
    }

    fn forced_weather(&mut self, env_type: i32) {
        if !matches!(self.code, 2 | 3) {
            return;
        }

        if matches!(env_type, 3 | 4) && self.emitters.snow {
            self.emitters = Emitters::RAIN;

            return;
        }

        if env_type == 0 && !self.emitters.snow {
            self.emitters = if self.code == 3 {
                Emitters::SNOW_HEAVY
            } else {
                Emitters::SNOW
            };
        }
    }

    fn cycle(&mut self, sky: &mut Sky, env_type: i32, tile: (i32, i32), now_ms: u32) {
        if self.code != 0 || !matches!(env_type, 0 | 3 | 4) {
            return;
        }

        let mut slot = slot(now_ms);

        if self.forced != 0 {
            slot = self.forced;
        }

        if env_type == 4 {
            slot = 11;
        }

        if matches!(env_type, 3 | 4) && self.emitters.snow {
            self.emitters = Emitters::RAIN;
        }

        match slot {
            0..=7 => {
                if is_special_tile(tile) {
                    self.raise(sky, 11, Emitters::SNOW, now_ms);
                } else {
                    self.raise(sky, 10, Emitters::NONE, now_ms);
                }
            }
            8 | 9 => self.raise(sky, 12, Emitters::NONE, now_ms),
            10 | 11 => self.raise(sky, 13, Emitters::NONE, now_ms),
            12 => self.latch(sky, slot, Emitters::RAIN, now_ms),
            13 => self.latch(sky, slot, Emitters::SNOW, now_ms),
            14 => self.latch(sky, slot, Emitters::SNOW_HEAVY, now_ms),
            _ => {}
        }
    }

    fn raise(&mut self, sky: &mut Sky, day: u32, emitters: Emitters, now_ms: u32) {
        if sky.day == day % 10 || sky.day == day {
            return;
        }

        sky.set_day(day, now_ms);

        self.emitters = emitters;
    }

    fn latch(&mut self, sky: &mut Sky, slot: u32, emitters: Emitters, now_ms: u32) {
        if sky.day != 1 {
            self.code = slot;

            sky.set_day(11, now_ms);
        }

        self.emitters = emitters;
    }
}

pub fn warp_resets_yaw(env_type: i32) -> bool {
    matches!(env_type, 0 | 3 | 4)
}
