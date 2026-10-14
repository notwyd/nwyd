use super::rng::Lcg;
use super::weather::Anchor;
use formats::world::attribute_map::{AttributeMap, flags};
use render::billboard::BillboardVertex;

pub const FLAKES: usize = 200;

pub const HALF_PER_SCALE: f32 = 0.05;

pub const SOUND_GATE: f32 = 0.07;

pub const SEED_SPAN: f32 = 6.0;

pub const SEED_STEP: f32 = 0.5;

pub const SEED_MOD: u32 = 24;

pub const SEED_HEIGHT_MOD: u32 = 10;

pub const SPEED_STEP: f32 = 0.04;

pub const SPEED_MOD: u32 = 3;

pub const SPEED_WHEN_ZERO: f32 = 0.24;

pub const SPAWN_ABOVE: f32 = 7.0;

pub const SPAWN_STEP: f32 = 0.5;

pub const SPAWN_STEPS: u32 = 10;

pub const SPREAD_BASE: f32 = 4.0;

pub const SPREAD_DIVISOR: f32 = 3.0;

pub const SPREAD_MOD: u32 = 32;

pub const FALL_SCALE: f32 = 0.02;

pub const FLAKE_DRIFT: f32 = 0.01;

pub const DRIFT_TWO: f64 = 2.0;

pub const DRIFT_DIVISOR: f64 = 200.0;

pub const DRIFT_PI: f64 = 3.141_592_741_012_573_2;

pub fn drift_angle(i: usize) -> f32 {
    (DRIFT_TWO * (i as f64 * DRIFT_TWO / DRIFT_DIVISOR) * DRIFT_PI) as f32
}

pub const ALPHA_SLOPE: f32 = 24.0;

pub const ALPHA_SPAN: f32 = 7.0;

pub const ALPHA_FULL: u8 = 0xAA;

pub const DIFFUSE_SEED: u32 = 0xAAAA_AAAA;

pub const TEXTURE_SLOT: usize = 2;

pub const UV: [[f32; 2]; 4] = [[0.02, 0.02], [0.98, 0.02], [0.98, 0.98], [0.02, 0.98]];

pub const SNOW_CLASS: &super::effect_tree::Class = &super::effect_tree::Class {
    name: "Snow",
    method_count: 15,
    base: super::effect_tree::Base::VisualEffect,
    pass: super::effect_tree::Pass::Effect,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Flake {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub speed: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Snow {
    pub flakes: Vec<Flake>,
    pub half: f32,
    pub last_ms: u32,
}

impl Snow {
    pub fn new(scale: f32, rng: &mut Lcg) -> Snow {
        let flakes = (0..FLAKES)
            .map(|_| {
                let x = SEED_SPAN - rng.next_mod(SEED_MOD) as f32 * SEED_STEP;

                let z = SEED_SPAN - rng.next_mod(SEED_MOD) as f32 * SEED_STEP;

                let y = rng.next_mod(SEED_HEIGHT_MOD) as f32;

                let drawn = rng.next_mod(SPEED_MOD) as f32 * SPEED_STEP;

                let speed = if drawn == 0.0 { SPEED_WHEN_ZERO } else { drawn };

                Flake { x, y, z, speed }
            })
            .collect();

        Snow {
            flakes,
            half: scale * HALF_PER_SCALE,
            last_ms: 0,
        }
    }

    pub fn drives_the_blizzard_sound(&self) -> bool {
        self.half > SOUND_GATE
    }

    pub fn tick(&mut self, now_ms: u32, anchor: Anchor, rng: &mut Lcg) {
        let step = now_ms.wrapping_sub(self.last_ms) as f32 * FALL_SCALE;

        for (i, flake) in self.flakes.iter_mut().enumerate() {
            if flake.y <= anchor.floor {
                let Some(base) = anchor.lift_from else {
                    continue;
                };

                flake.y = base + SPAWN_ABOVE + rng.next_mod(SPAWN_STEPS) as f32 * SPAWN_STEP;

                flake.x = spread(rng) + anchor.x;

                flake.z = spread(rng) + anchor.z;
            } else {
                flake.y -= flake.speed * step;

                flake.x += drift_angle(i).sin() * FLAKE_DRIFT;
            }
        }

        self.last_ms = now_ms;
    }

    pub fn alpha(&self, flake: &Flake, floor: f32) -> u8 {
        let mut alpha = ((flake.y - floor) * ALPHA_SLOPE) as i32 as u8;

        if floor + ALPHA_SPAN < flake.y {
            alpha = ALPHA_FULL;
        }

        if flake.y < floor {
            alpha = 0;
        }

        alpha
    }

    pub fn quad(&self, alpha: u8, right: [f32; 3], up: [f32; 3]) -> [BillboardVertex; 4] {
        let s = self.half;

        let diffuse = u32::from_le_bytes([alpha; 4]);

        let corner = |rs: f32, us: f32| {
            [
                (rs * right[0] + us * up[0]) * s,
                (rs * right[1] + us * up[1]) * s,
                (rs * right[2] + us * up[2]) * s,
            ]
        };

        let signs = [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)];

        std::array::from_fn(|i| BillboardVertex {
            position: corner(signs[i].0, signs[i].1),
            diffuse,
            uv: UV[i],
        })
    }
}

fn spread(rng: &mut Lcg) -> f32 {
    SPREAD_BASE - rng.next_mod(SPREAD_MOD) as f32 / SPREAD_DIVISOR
}

pub fn roofed(map: Option<&AttributeMap>, x: f32, z: f32) -> bool {
    let cell = |v: f32| {
        let truncated = v as i32;

        let quarter = (truncated + ((truncated >> 31) & 3)) >> 2;

        if !(0..=1023).contains(&quarter) {
            0usize
        } else {
            quarter as usize
        }
    };

    match map.and_then(|m| m.at(cell(x), cell(z))) {
        Some(byte) => byte & flags::ROOFED != 0,
        None => false,
    }
}
