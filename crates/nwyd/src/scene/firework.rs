use formats::net::firework::{CELLS, Glyph, SIDE};
use glam::Vec3;
use render::billboard::{BillboardPass, BillboardUniforms, BillboardVertex};
use render::camera::Matrix4;

pub fn console_grid(text: &str, at: Vec3) -> Vec<(Vec3, i32)> {
    let mut chars = text.chars();

    if matches!(chars.next(), None | Some('^'))
        || !chars.take(3).eq(['!', '!', '!'])
        || !(3968.0..4096.0).contains(&at.x)
        || !(3968.0..4096.0).contains(&at.z)
    {
        return Vec::new();
    }

    (0..5)
        .flat_map(|row| {
            (0..5).map(move |column| {
                (
                    Vec3::new(
                        at.x - 10.0 + row as f32 * 5.0,
                        7.0,
                        at.z - 10.0 + column as f32 * 5.0,
                    ),
                    row,
                )
            })
        })
        .collect()
}

use super::effect_tree::{Base, Class, Effect, Pass, Sink, Submit, Tick};
use super::rng::Lcg;

pub const CLASS: Class = Class {
    name: "EffectFireWork",
    method_count: 14,
    base: Base::SceneNode,
    pass: Pass::Effect,
};

pub const OBJECT_SIZE: usize = 0x1EAC;

pub const PARTICLES: usize = CELLS;

pub const TEXTURE_SLOT: usize = 7;

pub const LIFE_MS: u32 = 10_000;

pub const RISE_MS: f32 = 3000.0;

pub const RAMP_MS: u32 = 7000;

pub const ANCHOR_LIFT: f32 = 5.0;

pub const RISE_VELOCITY: Vec3 = Vec3::new(0.0, 5.0, 0.0);

pub const BURST_SPEED: f32 = 15.0;

pub const LIFETIME_BASE: f32 = 10.0;

pub const LIFETIME_JITTER: f32 = 0.25;

pub const SEED_SCALE: f32 = 0.1;

pub const DRAG: f32 = 1.8;

pub const DRAG_SQUARED: f32 = 3.239_999_8;

pub const GRAVITY: Vec3 = Vec3::new(0.0, -9.8, 0.0);

pub const HALF: f32 = 0.5;

pub const SCALE_GAIN: f32 = 0.3;

pub const LAG_DIVISOR: f32 = 20.0;

pub const RISE_DIVISOR: f32 = 1.5;

pub const DROOP_PHASE: f32 = 0.25;

pub const DROOP_GAIN: f32 = 3.0;

pub const RAND_SPAN: f32 = 32767.0;

pub const GRID_ANGLE: f32 = -std::f32::consts::FRAC_PI_4;

pub const GRID_Y: f32 = -1.5;

pub const JITTER_DIVISOR: f32 = 0.08;

pub const LAUNCH_SOUND: i64 = 0x13B;

pub const BURST_SOUND: i64 = 0x13C;

pub const GLYPH_PATTERN: i32 = 6;

pub const RAMPS: i32 = 6;

pub const AGE_CEILING: f32 = 0.311_111_1;

pub const CORE_SCALE: f32 = 0.7;

pub const CORE_ARGB: u32 = 0xAAAA_AAAA;

pub const CORNERS: [([f32; 3], [f32; 2]); 4] = [
    ([-0.5, -0.5, 0.0], [0.02, 0.98]),
    ([0.5, -0.5, 0.0], [0.98, 0.98]),
    ([0.5, 0.5, 0.0], [0.98, 0.02]),
    ([-0.5, 0.5, 0.0], [0.02, 0.02]),
];

pub fn signed(rng: &mut Lcg) -> f32 {
    let a = rng.next() as i32;

    let b = rng.next() as i32;

    (a - b) as f32 / RAND_SPAN
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Particle {
    pub position: Vec3,
    pub rise: Vec3,
    pub origin: Vec3,
    pub burst: Vec3,
    pub lifetime: f32,
    pub age: f32,
    pub kind: u16,
    pub scale: f32,
}

impl Particle {
    pub fn seed(kind: u16, at: Vec3, rng: &mut Lcg) -> Particle {
        let direction =
            Vec3::new(signed(rng), signed(rng), signed(rng)).normalize_or_zero() * BURST_SPEED;

        let lifetime = (signed(rng) * LIFETIME_JITTER + 1.0) * LIFETIME_BASE;

        Particle {
            position: Vec3::ZERO,
            rise: RISE_VELOCITY,
            origin: at,
            burst: direction + RISE_VELOCITY,
            lifetime,
            age: 0.0,
            kind,
            scale: SEED_SCALE,
        }
    }

    pub fn visible(&self) -> bool {
        use std::cmp::Ordering::{Equal, Greater};

        !matches!(self.age.partial_cmp(&1.0), Some(Greater | Equal))
    }
}

#[derive(Debug, Clone)]

pub struct Firework {
    pub pattern: i32,
    pub ramp: Option<i32>,
    pub particles: Box<[Particle; PARTICLES]>,
    pub dirs: Box<[Vec3; PARTICLES]>,
    pub birth_ms: u32,
    pub colour: u32,
    pub banged: bool,
    pub gone: bool,
    pub sounds: Vec<i64>,
    pub facing: (f32, f32),
    pub rng: Lcg,
}

impl Firework {
    pub fn new(at: Vec3, pattern: i32, birth_ms: u32, rng: &mut Lcg) -> Firework {
        let mut own = Lcg::new(rng.next());

        let particles = std::array::from_fn(|i| Particle::seed((i % 3) as u16, at, &mut own));

        Firework {
            pattern,
            ramp: None,
            particles: Box::new(particles),
            dirs: Box::new([Vec3::ZERO; PARTICLES]),
            birth_ms,
            colour: 0,
            banged: false,
            gone: false,
            sounds: Vec::new(),
            facing: (0.0, 0.0),
            rng: own,
        }
    }

    pub fn seed_glyph(&mut self, glyph: &Glyph) {
        if self.pattern != GLYPH_PATTERN {
            return;
        }

        let cells = glyph.walked();

        let per_cell = glyph.per_cell();

        let (sin, cos) = (GRID_ANGLE.sin(), GRID_ANGLE.cos());

        let mut placed = 0usize;

        let mut last = (0usize, 0usize);

        for row in 0..SIDE {
            for col in 0..SIDE {
                if !cells[row * SIDE + col] || per_cell == 0 {
                    continue;
                }

                last = (row, col);

                let (r, k) = Glyph::grid(row, col);

                let (r, k) = (r as f32 * SCALE_GAIN, k as f32 * SCALE_GAIN);

                for step in 0..per_cell {
                    self.dirs[placed] = match step {
                        0 => Vec3::new(r * cos + k * sin, GRID_Y, cos * k - r * sin),
                        _ => Vec3::new(
                            signed(&mut self.rng) * HALF / JITTER_DIVISOR + r,
                            signed(&mut self.rng) / JITTER_DIVISOR * SCALE_GAIN - RISE_DIVISOR,
                            signed(&mut self.rng) * HALF / JITTER_DIVISOR + k,
                        ),
                    };

                    placed += 1;
                }
            }
        }

        let (row, col) = last;

        while placed < PARTICLES {
            self.dirs[placed] = Vec3::new(
                signed(&mut self.rng) / JITTER_DIVISOR + row as f32 * SCALE_GAIN,
                GRID_Y,
                signed(&mut self.rng) / JITTER_DIVISOR + col as f32 * SCALE_GAIN,
            );

            placed += 1;
        }

        self.ramp = Some((self.rng.next() % RAMPS as u32) as i32);
    }

    pub fn elapsed(&self, now_ms: u32) -> u32 {
        now_ms.wrapping_sub(self.birth_ms)
    }

    pub fn phase(&self, now_ms: u32) -> f32 {
        self.elapsed(now_ms) as f32 / RISE_MS - 1.0
    }

    pub fn ramp(&self, now_ms: u32) -> f32 {
        let held = self.elapsed(now_ms).min(RAMP_MS) as f32 / RAMP_MS as f32;

        (held * std::f32::consts::PI).sin()
    }

    pub fn ramp_index(&self) -> Option<i32> {
        (0..RAMPS).find(|&k| self.pattern == k || self.ramp == Some(k))
    }

    pub fn colour_at(&self, now_ms: u32) -> u32 {
        let f = self.ramp(now_ms);

        let full = (f * 255.0).trunc().max(0.0) as u32;

        let half = (f * 100.0).trunc().max(0.0) as u32;

        let (r, g, b) = match self.ramp_index() {
            Some(0) => (full, half, 0),
            Some(1) => (0, full, 0),
            Some(2) => (full, 0, full),
            Some(3) => (full, full, full),
            Some(4) => (full, 0, 0),
            Some(5) => (0, half, full),
            _ => (0, 0, 0),
        };

        (r << 8 | g) << 8 | b
    }

    fn fuse(&mut self, i: usize, t: f32) {
        let lag = (signed(&mut self.rng) + 1.0) / LAG_DIVISOR;

        let d = t - lag;

        let p = &mut self.particles[i];

        p.position = p.origin + p.rise * d / RISE_DIVISOR;
    }

    fn drag(&mut self, i: usize, t: f32) {
        let e = (-DRAG * t).exp();

        let f = (1.0 - e) * HALF / DRAG_SQUARED;

        let p = &mut self.particles[i];

        p.position = p.origin + GRAVITY * t / DRAG + (p.burst * DRAG + GRAVITY) * f;
    }

    fn glyph_step(&mut self, i: usize, t: f32) {
        let s = match t < HALF {
            true => (t * std::f32::consts::PI).sin() * HALF,
            false => HALF,
        };

        let droop = (t * std::f32::consts::PI * DROOP_PHASE).sin() * t * DROOP_GAIN;

        let dir = self.dirs[i];

        let p = &mut self.particles[i];

        p.position = Vec3::new(
            p.origin.x + s * dir.x,
            p.origin.y + s * dir.y - droop,
            p.origin.z + s * dir.z,
        );
    }

    fn settle(&mut self, i: usize, t: f32) {
        let p = &mut self.particles[i];

        p.age = t / p.lifetime;

        let a = HALF + p.age;

        p.scale = (-(a * a)).exp() * SCALE_GAIN;
    }

    pub fn world_of(&self, at: Vec3, scale: f32) -> Matrix4 {
        let (yaw, pitch) = self.facing;

        let facing =
            Matrix4::rotation_yaw_pitch_roll(std::f32::consts::FRAC_PI_2 - yaw, -pitch, 0.0);

        Matrix4::multiply(
            &Matrix4::multiply(&facing, &Matrix4::scaling(scale, scale, scale)),
            &Matrix4::translation(at.x, at.y, at.z),
        )
    }

    pub fn vertices(diffuse: u32) -> [BillboardVertex; 4] {
        CORNERS.map(|(position, uv)| BillboardVertex {
            position,
            diffuse,
            uv,
        })
    }

    pub fn passes(&self, particle: &Particle) -> [(f32, u32); 2] {
        [
            (particle.scale, self.colour),
            (particle.scale * CORE_SCALE, CORE_ARGB),
        ]
    }

    fn bang(&mut self) {
        if !self.banged {
            self.banged = true;

            self.sounds.push(BURST_SOUND);
        }
    }
}

impl Effect for Firework {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        if self.elapsed(tick.now_ms) > LIFE_MS {
            self.gone = true;

            return;
        }

        let t = self.phase(tick.now_ms);

        self.colour = self.colour_at(tick.now_ms);

        self.facing = (tick.yaw, tick.pitch);

        for i in 0..PARTICLES {
            if t < 0.0 {
                self.fuse(i, t);

                continue;
            }

            match self.pattern {
                p if p < GLYPH_PATTERN => {
                    self.bang();

                    self.drag(i, t);

                    self.settle(i, t);
                }
                GLYPH_PATTERN => {
                    self.bang();

                    self.glyph_step(i, t);

                    self.settle(i, t);
                }
                _ => {}
            }
        }
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn take_sounds(&mut self) -> Vec<i64> {
        std::mem::take(&mut self.sounds)
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        if self.gone || (self.pattern != 6 && cx.scene.detail == 1) {
            return;
        }

        let Some((array, layer)) = cx.scene.placement(TEXTURE_SLOT) else {
            out.effect.unbound += 1;

            return;
        };

        let opaque = cx.scene.alpha_at(TEXTURE_SLOT);

        for particle in self.particles.iter() {
            if !particle.visible() {
                continue;
            }

            for (scale, diffuse) in self.passes(particle) {
                out.effect.push_quad(
                    BillboardPass::Additive,
                    BillboardUniforms::new(
                        self.world_of(particle.position, scale),
                        BillboardPass::Additive,
                    )
                    .with_layer(layer)
                    .with_opaque(opaque),
                    &Firework::vertices(diffuse),
                    array,
                );

                out.effect.submitted(&CLASS);
            }
        }
    }
}
