use super::quads::{GroundQuad, Slot};
use super::rng::Lcg;
use super::snow::roofed;
use super::timers::TimedQuad;
use super::weather::Anchor;
use formats::world::attribute_map::{self, AttributeMap};
use glam::Vec3;
use render::billboard::{BillboardPass, BillboardVertex};

pub const DROPS: usize = 50;

pub const SEED_SPAN: f32 = 6.0;

pub const SEED_STEP: f32 = 0.5;

pub const SEED_MOD: u32 = 24;

pub const SEED_HEIGHT_MOD: u32 = 10;

pub const SPEED_STEP: f32 = 0.08;

pub const SPEED_MOD: u32 = 3;

pub const SPEED_WHEN_ZERO: f32 = 0.24;

pub const SPAWN_ABOVE: f32 = 10.0;

pub const FALL_SCALE: f32 = 0.1;

pub const SPLASH_ODDS: u32 = 5;

pub const SPLASH_SLOT: usize = 10;

pub const SPLASH_LIFE_MS: u32 = 700;

pub const SPLASH_SCALE: f32 = 0.01;

pub const SPLASH_GROW: f32 = 0.0004;

pub const SPLASH_ARGB: u32 = 0xFFFF_FFFF;

pub const HALF_WIDTH: f32 = 0.008;

pub const HALF_HEIGHT: f32 = 0.2;

pub const DIFFUSE: u32 = 0x3333_3333;

pub const TEXTURE_SLOT: usize = 9;

pub const DRAW_GATE: super::lod::DetailGate = super::lod::DetailGate::NotReduced;

pub const UV: [[f32; 2]; 4] = [[0.02, 0.02], [0.98, 0.02], [0.98, 0.98], [0.02, 0.98]];

pub const RAIN_CLASS: &super::effect_tree::Class = &super::effect_tree::Class {
    name: "Rain",
    method_count: 15,
    base: super::effect_tree::Base::VisualEffect,
    pass: super::effect_tree::Pass::Effect,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Drop {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub speed: f32,
}

impl Drop {
    pub fn world(&self, anchor: Anchor) -> [f32; 3] {
        [anchor.x + self.x, self.y, anchor.z + self.z]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Rain {
    pub drops: Vec<Drop>,
    pub last_ms: u32,
    pub grounded: usize,
}

impl Rain {
    pub fn new(rng: &mut Lcg) -> Rain {
        let drops = (0..DROPS)
            .map(|_| {
                let x = SEED_SPAN - rng.next_mod(SEED_MOD) as f32 * SEED_STEP;

                let z = SEED_SPAN - rng.next_mod(SEED_MOD) as f32 * SEED_STEP;

                let y = rng.next_mod(SEED_HEIGHT_MOD) as f32;

                let drawn = rng.next_mod(SPEED_MOD) as f32 * SPEED_STEP;

                let speed = if drawn == 0.0 { SPEED_WHEN_ZERO } else { drawn };

                Drop { x, y, z, speed }
            })
            .collect();

        Rain {
            drops,
            last_ms: 0,
            grounded: 0,
        }
    }

    pub fn tick(
        &mut self,
        now_ms: u32,
        anchor: Anchor,
        rng: &mut Lcg,
        map: Option<&AttributeMap>,
        ground: impl Fn(f32, f32) -> f32,
    ) -> Vec<TimedQuad> {
        self.tick_on_surface(now_ms, anchor, rng, map, |x, z| {
            super::drip::splash_y(ground(x, z))
        })
    }

    pub fn tick_on_surface(
        &mut self,
        now_ms: u32,
        anchor: Anchor,
        rng: &mut Lcg,
        map: Option<&AttributeMap>,
        surface: impl Fn(f32, f32) -> f32,
    ) -> Vec<TimedQuad> {
        let mut splashes = Vec::new();

        let step = now_ms.wrapping_sub(self.last_ms) as f32 * FALL_SCALE;

        for drop in &mut self.drops {
            if drop.y <= anchor.floor {
                let won = rng.next_mod(SPLASH_ODDS) == 0;

                let (wx, wz) = (anchor.x + drop.x, anchor.z + drop.z);

                if won && !splash_roofed(map, wx, wz) {
                    splashes.push(splash(Vec3::new(wx, surface(wx, wz), wz)));
                }

                match anchor.lift_from {
                    Some(base) => drop.y = base + SPAWN_ABOVE,
                    None => self.grounded += 1,
                }

                drop.x = SEED_SPAN - rng.next_mod(SEED_MOD) as f32 * SEED_STEP;

                drop.z = SEED_SPAN - rng.next_mod(SEED_MOD) as f32 * SEED_STEP;
            } else {
                drop.y -= drop.speed * step;
            }
        }

        self.last_ms = now_ms;

        splashes
    }

    pub fn quad(&self, right: [f32; 3], up: [f32; 3]) -> [BillboardVertex; 4] {
        let signs = [(-1.0, 1.0), (1.0, 1.0), (1.0, -1.0), (-1.0, -1.0)];

        let heights = [-HALF_HEIGHT, -HALF_HEIGHT, HALF_HEIGHT, HALF_HEIGHT];

        std::array::from_fn(|i| {
            let (rs, us): (f32, f32) = signs[i];

            BillboardVertex {
                position: [
                    (rs * right[0] + us * up[0]) * HALF_WIDTH,
                    heights[i],
                    (rs * right[2] + us * up[2]) * HALF_WIDTH,
                ],
                diffuse: DIFFUSE,
                uv: UV[i],
            }
        })
    }

    pub fn draws(&self, drop: &Drop, anchor: Anchor, map: Option<&AttributeMap>) -> bool {
        let world = drop.world(anchor);

        !roofed(map, world[0], world[2])
    }
}

pub fn splash_roofed(map: Option<&AttributeMap>, x: f32, z: f32) -> bool {
    let quarter = |v: f32| {
        let truncated = v as i32;

        (truncated + ((truncated >> 31) & 3)) >> 2
    };

    let flat = i64::from(quarter(z)) * attribute_map::DIM as i64 + i64::from(quarter(x));

    let Ok(flat) = usize::try_from(flat) else {
        return false;
    };

    map.and_then(|m| m.at_flat(flat))
        .is_some_and(|b| b & attribute_map::flags::ROOFED != 0)
}

pub fn splash(at: Vec3) -> TimedQuad {
    TimedQuad::new(
        GroundQuad {
            owner: 0,
            slot: Slot::Placed,
            texture: SPLASH_SLOT,
            pass: BillboardPass::Additive,
            anchor: at,
            base_scale: Vec3::splat(SPLASH_SCALE),
            grow: SPLASH_GROW,
            spin_period: 0,
            ordinal: 0,
            argb: SPLASH_ARGB,
        },
        SPLASH_LIFE_MS,
    )
}
