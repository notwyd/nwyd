use super::world::{LIGHT0_BY_DAY, SKY_COLOR_BY_DAY};

#[derive(Debug, Clone)]
pub struct Ornaments {
    pub billboards: [super::effects::Billboard; 22],
}

impl Ornaments {
    pub fn new(rng: &mut super::rng::Lcg) -> Self {
        use glam::Vec3;
        use render::billboard::BillboardPass;

        Self {
            billboards: std::array::from_fn(|i| {
                let mut b = super::effects::Billboard::new(56, 0, Vec3::splat(0.1), 0.0005, 1, 80);

                b.pass = BillboardPass::Additive;

                if i < 20 {
                    let x = (rng.next_mod(12000) as f32 - 6000.0) * 0.003;

                    let z = (rng.next_mod(12000) as f32 - 6000.0) * 0.003;

                    b.position = Vec3::new(x, (0.5 - (x * x + z * z).sqrt() / 50.0) * 20.0, z);

                    b.set_color(
                        [0xFFFFFFFF, 0xFF8888AA, 0xFFFF9955, 0xFFFFFFAA, 0xFFAAFFAA][i % 5],
                    );

                    b.pulse_floor = 0.3;

                    b.colour_mode = if i % 2 == 0 { 2 } else { 0 };
                } else {
                    b.kind = if i == 20 { 214 } else { 2 };

                    b.position = Vec3::new(-15.0, 7.5, 4.5);

                    b.scale = Vec3::splat(if i == 20 { 5.0 } else { 12.0 });

                    b.colour_mode = 0;

                    b.set_color(if i == 20 { 0xFFFFFFFF } else { 0x00554433 });

                    if i == 20 {
                        b.pass = BillboardPass::Blended;
                    }
                }

                b
            }),
        }
    }

    pub fn update(&mut self, sky: Sky, pose: super::world::Pose, now_ms: u32, day_tail: bool) {
        if sky.fading(now_ms) && day_tail {
            let t = now_ms.wrapping_sub(sky.stamp) as f32 / 2000.0;

            let alpha = match sky.day {
                13 => Some(t),
                10 if sky.to == 0 => Some(1.0 - t),
                _ => None,
            };

            if let Some(alpha) = alpha {
                self.billboards[20].set_color(((alpha * 255.0) as u32) << 24 | 0xFFFFFF);
            }
        }

        if !matches!(sky.day, 3 | 10 | 13) {
            return;
        }

        for b in &mut self.billboards {
            let local = b.position;

            b.position += pose.eye;

            b.update(now_ms, pose.yaw, pose.pitch);

            b.position = local;
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn submit(
        &self,
        sky: Sky,
        scene: u32,
        environment: i32,
        effects: &super::effects::BillboardScene,
        eye: glam::Vec3,
        now_ms: u32,
        frame: &mut super::effects::Frame,
    ) {
        use super::effect_tree::{Effect, Sink, Submit};

        if !super::world::draws_sky(scene, environment)
            || sky.from == 1
            || sky.to == 1
            || !(matches!(sky.day, 3 | 13) || sky.day == 10 && sky.to == 0)
        {
            return;
        }

        let mut mesh = Vec::new();

        let mut sink = Sink {
            effect: frame,
            mesh: &mut mesh,
        };

        let cx = Submit {
            scene: effects,
            eye,
            now_ms,
        };

        for b in &self.billboards {
            b.submit(&cx, &mut sink);
        }
    }
}

pub const FADE_MS: u32 = 2_000;

pub const PALETTE_BASE: u32 = 0x43;

pub fn row(day: u32) -> Option<usize> {
    match day % 10 {
        quarter @ 0..=3 => Some(quarter as usize),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blend {
    pub sky_color: u32,
    pub light0: [f32; 4],
    pub flare_scale: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sky {
    pub day: u32,
    pub from: usize,
    pub to: usize,
    pub destination_texture: usize,
    pub stamp: u32,
}

impl Default for Sky {
    fn default() -> Sky {
        Sky::new()
    }
}

impl Sky {
    pub const fn new() -> Sky {
        Sky {
            day: 0,
            from: 0,
            to: 0,
            destination_texture: 0,
            stamp: 0,
        }
    }

    pub fn at(day: u32) -> Sky {
        let mut sky = Sky::new();

        sky.set_day(day, 0);

        sky
    }

    pub fn set_day(&mut self, day: u32, now_ms: u32) {
        let Some(to) = row(day) else {
            return;
        };

        match day / 10 {
            0 => self.from = to,
            1 => {
                self.from = row(self.day).unwrap_or(to);

                self.to = to;

                self.destination_texture = 67 + to;
            }
            _ => {}
        }

        self.day = day;

        self.stamp = now_ms;
    }

    pub fn fading(&self, now_ms: u32) -> bool {
        self.day / 10 == 1 && now_ms.wrapping_sub(self.stamp) < FADE_MS
    }

    pub fn flare_suppressed(&self) -> bool {
        let settled_dark = !(self.day == 0 || self.day > 9);

        settled_dark || self.to == 1 || self.from == 1
    }

    pub fn update_blend(&mut self, now_ms: u32) -> Blend {
        if self.fading(now_ms) {
            let t = now_ms.wrapping_sub(self.stamp) as f32 / FADE_MS as f32;

            return Blend {
                sky_color: blend_color(SKY_COLOR_BY_DAY[self.from], SKY_COLOR_BY_DAY[self.to], t),
                light0: blend_light(LIGHT0_BY_DAY[self.from], LIGHT0_BY_DAY[self.to], t),
                flare_scale: self.flare_scale(t),
            };
        }

        self.set_day(self.day % 10, now_ms);

        Blend {
            sky_color: SKY_COLOR_BY_DAY[self.from],
            light0: LIGHT0_BY_DAY[self.from],
            flare_scale: None,
        }
    }

    fn flare_scale(&self, t: f32) -> Option<f32> {
        match self.day {
            13 => None,
            10 if self.to == 0 => Some(t),
            11..=12 if self.to == 1 || self.to == 2 => Some(1.0 - t),
            _ => None,
        }
    }
}

pub fn blend_color(from: u32, to: u32, t: f32) -> u32 {
    let inv = 1.0 - t;

    let mut out = 0;

    for shift in [16, 8, 0] {
        let a = ((from >> shift) & 0xFF) as f32;

        let b = ((to >> shift) & 0xFF) as f32;

        out = (out << 8) | u32::from((a * inv + b * t) as u8);
    }

    out
}

pub fn blend_light(from: [f32; 4], to: [f32; 4], t: f32) -> [f32; 4] {
    let inv = 1.0 - t;

    [
        from[0] * inv + to[0] * t,
        from[1] * inv + to[1] * t,
        from[2] * inv + to[2] * t,
        from[3],
    ]
}
