use glam::{Vec2, Vec3};
use render::camera::{self, View};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Orbit,
    Fixed,
    Steep,
    Overview,
    Other(i32),
}

pub const RESET_PITCH: f32 = f32::from_bits(0xBF45_1E9B);

pub const RESET_YAW: f32 = f32::from_bits(0x3F49_0FDB);

pub const RESET_HEIGHT_OFF: f32 = f32::from_bits(0x3EA8_F5C3);

pub const RESET_TARGET_Y: f32 = 3.0;

pub const RESET_DIST_GOAL: f32 = 5.5;

pub const CLEARANCE_FLOOR: f32 = 1.3;

pub const FIELD_MAX_DIST: f32 = 15.0;

pub const WORLD_OFFSET: f32 = 0.0;

pub const ORBIT_STRETCH: f32 = 1.2;

pub const EYE_Y_BIAS: f32 = -2.0;

pub const CLEARANCE_MARGIN: f32 = 0.27;

pub const CLEARANCE_STEP: f32 = 0.05;

pub const CLEARANCE_FIRST_STEP: f32 = 1.0;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rider {
    pub mounted: bool,
    pub mount_model: Option<u32>,
    pub mount_kind: i16,
    pub con: u16,
    pub action: usize,
}

pub const CON_CLAMP: u16 = 500;

pub const GROUND_HEIGHT_OFF: f32 = 0.4;

pub const MOUNTED_HEIGHT_OFF: f32 = 1.0;

pub const HEIGHT_OFF_PER_CON: f32 = 0.0002;

pub const SEATED_HEIGHT_OFF: f32 = -0.2;

pub const SEATED_ACTIONS: [usize; 4] = [0x0F, 0x18, 0x1A, 0x1B];

pub const MOUNTED_FLOOR_TALL: f32 = 4.7;

pub const MOUNTED_FLOOR_MID: f32 = 3.5;

pub const MOUNTED_FLOOR: f32 = 2.5;

pub const FLOOR_PER_CON: f32 = 0.001;

pub const TALL_MOUNTS: [u32; 2] = [0x27, 0x28];

pub const TALL_MOUNT_CONDITIONAL: u32 = 0x14;

pub const TALL_MOUNT_EXEMPT_KIND: i16 = 3;

pub const MID_MOUNT: u32 = 0x26;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct MouseInput {
    pub dx: i32,
    pub dy: i32,
    pub dz: i32,
    pub mmb: bool,
    pub rmb: bool,
    pub alt: bool,
}

impl MouseInput {
    pub fn rotating(&self) -> bool {
        self.mmb || (self.alt && self.rmb)
    }
}

pub const YAW_PER_COUNT: f32 = 0.005;

pub const PITCH_PER_COUNT: f32 = 0.002;

pub const PITCH_FLOOR: f32 = f32::from_bits(0xBF7C_430E);

pub const PITCH_CEILING: f32 = 0.75;

pub const TWO_PI: f32 = f32::from_bits(0x40C9_0FDB);

pub const MOUNTED_PITCH_CAP: f32 = 0.45;

pub const MOUNTED_PITCH_CAP_TALL: f32 = 0.23;

pub const MOUNTED_PITCH_CAP_MID: f32 = 0.22;

pub const ZOOM_DIVISOR: f64 = 240.0;

pub const MIN_DIST: f32 = 1.2;

pub const MIN_DIST_MOUNTED: f32 = 2.5;

pub const MIN_DIST_PER_CON: f32 = 0.0002;

pub const VIEW_DIST_NEAR: f32 = 11.0;

pub const OVER_SHOULDER_YAW: f32 = f32::from_bits(0x4049_0FDB);

pub const OVER_SHOULDER_PITCH: f32 = f32::from_bits(0xBE19_999A);

pub const OVER_SHOULDER_DIST: f32 = 3.5;

pub const VIEW_YAW: f32 = f32::from_bits(0x3F4D_2A4D);

pub const VIEW_PITCH: f32 = RESET_PITCH;

pub const SCENIC_VIEW_YAW: f32 = f32::from_bits(0x3F81_B7A6);

pub const SCENIC_VIEW_PITCH: f32 = f32::from_bits(0xBF24_21D7);

pub fn next_view(view: i32, low_ceiling: bool) -> i32 {
    let next = match view + 1 > VIEWS - 1 {
        true => 0,
        false => view + 1,
    };

    match low_ceiling && next == 1 {
        true => 2,
        false => next,
    }
}

pub const VIEWS: i32 = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shake {
    pub birth_ms: u32,
    pub strength: u32,
    yaw: f32,
    pitch: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Camera {
    pub mode: Mode,
    pub eye_mode: i32,
    pub retained_target: Vec2,
    pub target_x_offset: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub target_y: f32,
    pub dist: f32,
    pub dist_goal: f32,
    pub height_off: f32,
    pub clearance_floor: f32,
    pub max_dist: f32,
    pub missing_ground_offset: f32,
    pub eye: Vec3,
    pub shake: Option<Shake>,
}

impl Camera {
    pub fn field() -> Camera {
        Camera {
            mode: Mode::Orbit,
            eye_mode: 0,
            retained_target: Vec2::ZERO,
            target_x_offset: 0.0,
            yaw: RESET_YAW,
            pitch: RESET_PITCH,
            target_y: RESET_TARGET_Y,
            dist: FIELD_MAX_DIST,
            dist_goal: RESET_DIST_GOAL,
            height_off: RESET_HEIGHT_OFF,
            clearance_floor: CLEARANCE_FLOOR,
            max_dist: FIELD_MAX_DIST,
            missing_ground_offset: 0.0,
            eye: Vec3::ZERO,
            shake: None,
        }
    }

    pub fn set_mode(&mut self, native_mode: i32, fog_enabled: &mut bool) {
        let mode = match native_mode {
            0 => Mode::Orbit,
            1 => Mode::Fixed,
            2 => Mode::Steep,
            4 => Mode::Overview,
            value => Mode::Other(value),
        };

        if self.mode == mode && matches!(mode, Mode::Orbit | Mode::Fixed) {
            return;
        }

        self.mode = mode;

        *fog_enabled = true;

        let preset = match mode {
            Mode::Orbit => Some((f32::from_bits(0xBEA0_D97C), 7.0, 0.0)),
            Mode::Fixed => Some((RESET_PITCH, self.max_dist, RESET_HEIGHT_OFF)),
            Mode::Steep => Some((f32::from_bits(0xBF86_0A92), self.max_dist - 1.0, 0.5)),
            Mode::Overview => Some((f32::from_bits(0xBF49_0FDB), 15.5, 0.0)),
            Mode::Other(_) => None,
        };

        if let Some((pitch, distance, height)) = preset {
            self.pitch = pitch;

            self.yaw = RESET_YAW;

            self.dist = distance;

            self.dist_goal = distance;

            self.height_off = height;
        }
    }

    pub fn add_angles(&mut self, yaw_delta: f32, pitch_delta: f32) {
        self.yaw += yaw_delta;

        self.pitch += pitch_delta;

        if self.yaw > TWO_PI {
            self.yaw -= TWO_PI;
        } else if self.yaw < 0.0 {
            self.yaw += TWO_PI;
        }

        let half_pi = f32::from_bits(0x3FC9_0FDB);

        if self.pitch >= half_pi {
            self.pitch = f32::from_bits(0x3FC8_EF16);
        } else if self.pitch <= -half_pi {
            self.pitch = f32::from_bits(0xBFC8_EF16);
        }
    }

    pub fn start_shake(&mut self, strength: u32, now_ms: u32) {
        let (yaw, pitch) = self
            .shake
            .map_or((self.yaw, self.pitch), |s| (s.yaw, s.pitch));

        self.shake = Some(Shake {
            birth_ms: now_ms,
            strength,
            yaw,
            pitch,
        });
    }

    pub fn tick_shake(&mut self, now_ms: u32) {
        let Some(shake) = self.shake else {
            return;
        };

        let elapsed = now_ms.wrapping_sub(shake.birth_ms);

        if shake.strength == 10 {
            let phase = elapsed.min(5000) as f32 / 3000.0 * std::f32::consts::FRAC_PI_2
                + std::f32::consts::PI * 1.5;

            let blend = (phase.sin() + 1.0).clamp(0.0, 1.0);

            self.pitch = (1.0 - blend) * 0.1 - blend * std::f32::consts::FRAC_PI_4;

            self.yaw = (1.0 - blend) * std::f32::consts::PI + blend * std::f32::consts::FRAC_PI_4;

            self.dist = (1.0 - blend) * 3.5 + blend * self.max_dist;

            self.dist_goal = self.dist;

            self.shake = (elapsed <= 5000).then_some(Shake {
                yaw: self.yaw,
                pitch: self.pitch,
                ..shake
            });

            return;
        }

        if elapsed > 1000 {
            self.yaw = shake.yaw;

            self.pitch = shake.pitch;

            self.shake = None;

            return;
        }

        let phase = elapsed as f32 / 1000.0;

        let delta = (phase * std::f32::consts::PI * 12.0).sin()
            * 0.01
            * shake.strength as f32
            * (1.0 - phase);

        self.yaw = shake.yaw + delta;

        self.pitch = shake.pitch + delta;
    }

    pub fn place(&mut self, target: Vec3, rider: Rider) -> Vec3 {
        self.place_optional(Some(target), Some(rider))
    }

    pub fn place_optional(&mut self, target: Option<Vec3>, rider: Option<Rider>) -> Vec3 {
        let mut height = 0.0;

        if self.eye_mode == 0
            && let Some(target) = target
        {
            self.retained_target = Vec2::new(target.x - self.target_x_offset, target.z);

            height = target.y;
        }

        if self.eye_mode == 0 || self.eye_mode > 1 {
            let f = camera::forward(self.yaw, self.pitch);

            let radius = self.dist * ORBIT_STRETCH;

            self.eye = Vec3::new(
                (self.retained_target.x - WORLD_OFFSET) - f.x * radius,
                (self.target_y - self.pitch.sin() * self.dist)
                    + height
                    + self.height_off
                    + EYE_Y_BIAS,
                (self.retained_target.y + WORLD_OFFSET) - f.z * radius,
            );
        }

        if let Some(rider) = rider {
            self.follow(rider);
        }

        self.eye
    }

    pub fn follow(&mut self, rider: Rider) {
        let con = f32::from(rider.con.min(CON_CLAMP));

        match rider.mounted {
            true => {
                let base = match rider.mount_model {
                    Some(model) if tall_mount(model, rider.mount_kind) => MOUNTED_FLOOR_TALL,
                    Some(MID_MOUNT) => MOUNTED_FLOOR_MID,
                    _ => MOUNTED_FLOOR,
                };

                self.clearance_floor = base + con * FLOOR_PER_CON;

                self.height_off = con * HEIGHT_OFF_PER_CON + MOUNTED_HEIGHT_OFF;
            }
            false => {
                self.clearance_floor = CLEARANCE_FLOOR;

                self.height_off = match SEATED_ACTIONS.contains(&rider.action) {
                    true => SEATED_HEIGHT_OFF,
                    false => con * HEIGHT_OFF_PER_CON + GROUND_HEIGHT_OFF,
                };
            }
        }
    }

    pub fn view(&mut self, target: Vec3, rider: Rider, ground: &dyn Fn(f32, f32) -> f32) -> View {
        self.view_with_player(target, rider, Some(target), ground)
    }

    pub fn view_with_player(
        &mut self,
        target: Vec3,
        rider: Rider,
        player: Option<Vec3>,
        ground: &dyn Fn(f32, f32) -> f32,
    ) -> View {
        let rider = player.map(|_| rider);

        self.place_optional(Some(target), rider);

        let mut sampled = (self.eye.x, self.eye.z);

        let height = ground(self.eye.x, self.eye.z);

        if height >= self.eye.y - CLEARANCE_MARGIN {
            let need = (height - self.eye.y) + CLEARANCE_MARGIN;

            let sin_pitch = self.pitch.sin();

            if sin_pitch.abs() < 0.000001 {
                self.eye.y = height + CLEARANCE_MARGIN;
            } else if need / sin_pitch < 0.0 {
                let floor = (self.dist - CLEARANCE_FIRST_STEP).max(self.clearance_floor);

                let mut d = self.dist;

                while floor < d {
                    self.dist = d;

                    self.place_optional(Some(target), rider);

                    sampled = (self.eye.x, self.eye.z);

                    if ground(self.eye.x, self.eye.z) < self.eye.y - CLEARANCE_MARGIN {
                        break;
                    }

                    d -= CLEARANCE_STEP;
                }
            } else {
                self.dist -= need / sin_pitch;

                self.place_optional(Some(target), rider);
            }
        }

        let view = View::from_pose(self.eye, self.yaw, self.pitch);

        if let Some(player) = player {
            let x = player.x as i32 / 128;

            let z = player.z as i32 / 128;

            if (26..=30).contains(&x)
                && (8..=12).contains(&z)
                && ground(sampled.0, sampled.1) == -10000.0
            {
                let delta = (self.pitch + 0.2).sin() * 3.0;

                self.dist -= delta;

                self.missing_ground_offset -= delta;
            }
        }

        view
    }

    pub fn update(&mut self, mouse: MouseInput, rider: Rider) {
        if self.shake.is_some() {
            return;
        }

        let min_dist = self.min_dist(rider);

        match self.mode {
            Mode::Orbit => {
                if mouse.rotating() {
                    self.rotate(mouse, rider);
                }
            }
            Mode::Fixed => {}
            _ => return,
        }

        self.zoom(mouse.dz, min_dist);
    }

    pub fn min_dist(&self, rider: Rider) -> f32 {
        let base = match rider.mounted {
            true => MIN_DIST_MOUNTED,
            false => MIN_DIST,
        };

        f32::from(rider.con) * MIN_DIST_PER_CON + base
    }

    fn rotate(&mut self, mouse: MouseInput, rider: Rider) {
        let pitch = f64::from(self.pitch) - f64::from(mouse.dy) * f64::from(PITCH_PER_COUNT);

        self.pitch = (pitch as f32).clamp(PITCH_FLOOR, PITCH_CEILING);

        let yaw = f64::from(mouse.dx) * f64::from(YAW_PER_COUNT) + f64::from(self.yaw);

        self.yaw = yaw as f32;

        if self.yaw > TWO_PI {
            self.yaw -= TWO_PI;
        }

        if self.yaw < 0.0 {
            self.yaw += TWO_PI;
        }

        if rider.mounted {
            let cap = match rider.mount_model {
                Some(model) if tall_mount(model, rider.mount_kind) => MOUNTED_PITCH_CAP_TALL,
                Some(MID_MOUNT) => MOUNTED_PITCH_CAP_MID,
                _ => MOUNTED_PITCH_CAP,
            };

            if self.pitch > cap {
                self.pitch = cap;
            }
        }
    }

    fn zoom(&mut self, dz: i32, min_dist: f32) {
        if dz < 0 && min_dist < self.dist {
            self.dist = (f64::from(dz) / ZOOM_DIVISOR + f64::from(self.dist)) as f32;

            self.dist_goal = self.dist;
        }

        if self.dist < min_dist {
            self.dist = min_dist;

            self.dist_goal = min_dist;
        }

        if dz > 0 && self.dist < self.max_dist {
            self.dist = (f64::from(dz) / ZOOM_DIVISOR + f64::from(self.dist)) as f32;

            self.dist_goal = self.dist;
        }

        if self.max_dist < self.dist {
            self.dist = self.max_dist;

            self.dist_goal = self.max_dist;
        }
    }

    pub fn set_view(&mut self, view: i32, scenic: bool) {
        let (yaw, pitch) = match scenic {
            true => (SCENIC_VIEW_YAW, SCENIC_VIEW_PITCH),
            false => (VIEW_YAW, VIEW_PITCH),
        };

        let (yaw, pitch, dist) = match view {
            0 => (yaw, pitch, self.max_dist),
            1 => (yaw, pitch, VIEW_DIST_NEAR),
            2 => (OVER_SHOULDER_YAW, OVER_SHOULDER_PITCH, OVER_SHOULDER_DIST),
            _ => return,
        };

        self.mode = Mode::Orbit;

        self.yaw = yaw;

        self.pitch = pitch;

        self.dist = dist;

        self.dist_goal = dist;

        self.height_off = RESET_HEIGHT_OFF;
    }
}

fn tall_mount(model: u32, kind: i16) -> bool {
    TALL_MOUNTS.contains(&model)
        || (model == TALL_MOUNT_CONDITIONAL && kind != TALL_MOUNT_EXEMPT_KIND)
}
