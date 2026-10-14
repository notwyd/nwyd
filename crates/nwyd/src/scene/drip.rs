use super::effect_tree::{Base, Class, Pass};
use super::quads::{GroundQuad, Slot};
use super::rng::Lcg;
use super::timers::TimedQuad;
use formats::world::objects::{ObjRecord, ObjectClass};
use formats::world::path::HeightField;
use glam::Vec3;
use render::billboard::{BillboardPass, BillboardVertex};

pub const DROPLETS: usize = 10;

pub const DROPLET_STRIDE: usize = 12;

pub const JITTER_STEPS: u32 = 5;

pub const JITTER_STEP: f32 = 0.25;

pub const X_BIAS: f32 = 1.0;

pub const HEIGHT_MODULUS: u32 = 10;

pub const SPEEDS: [f32; 3] = [0.24, 0.08, 0.16];

pub const FLOOR: f32 = 0.0;

pub const FALL_SCALE: f32 = 0.1;

pub const FALL_HALF: f32 = 0.5;

pub const CLOCK_WRAP: f32 = 4_294_967_296.0;

pub const SPLASH_ODDS: u32 = 5;

pub const SPLASH_SLOT: usize = 10;

pub const SPLASH_LIFE_MS: u32 = 700;

pub const SPLASH_SCALE: f32 = 0.01;

pub const SPLASH_GROW: f32 = 0.0008;

pub const SPLASH_LIFT: f32 = 0.25;

pub const HEIGHT_SCALE: f32 = 0.1;

pub const HEIGHT_SENTINEL: f32 = 127.0;

pub const SPAWN_ABOVE: f32 = 10.0;

pub const TEXTURE_SLOT: usize = 9;

pub const TEXTURE_TTL_MS: u32 = 5000;

pub const HALF_WIDTH: f32 = 0.015;

pub const HALF_HEIGHT: f32 = 0.3;

pub const DIFFUSE: u32 = 0x3333_3333;

pub const UV: [[f32; 2]; 4] = [[0.02, 0.02], [0.98, 0.02], [0.98, 0.98], [0.02, 0.98]];

pub const SOUND_DROPWATER: i64 = 8;

pub const AUDIBLE: f32 = 4.0;

pub const DROP_CLASS: &Class = &Class {
    name: "RainDroplet",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const COLUMN_SEED: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Droplet {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub rate: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Column {
    pub x: f32,
    pub z: f32,
    pub droplets: [Droplet; DROPLETS],
    pub stamp_ms: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]

pub struct Spawned {
    pub splashes: Vec<TimedQuad>,
    pub sounds: Vec<i64>,
}

pub fn splash_y(height_byte: f32) -> f32 {
    let h = match height_byte == HEIGHT_SENTINEL {
        true => 0.0,
        false => height_byte,
    };

    h * HEIGHT_SCALE + SPLASH_LIFT
}

pub fn splash_surface_y(height_byte: f32, seas: &[super::sea::Sea], x: f32, z: f32) -> f32 {
    if x.is_finite() && z.is_finite() && (0.0..4096.0).contains(&x) && (0.0..4096.0).contains(&z) {
        let zone = ((x / 128.0) as u8, (z / 128.0) as u8);

        if let Some(sea) = seas
            .iter()
            .filter(|s| s.zone == zone)
            .take(10)
            .find(|s| s.contains(x, z))
            && sea.position.y > -100.0
        {
            return (sea.position.y + 0.25) * 0.1 + 0.25;
        }
    }

    splash_y(height_byte)
}

pub fn template() -> [BillboardVertex; 4] {
    let corners = [
        [-HALF_WIDTH, -HALF_HEIGHT, 0.0],
        [HALF_WIDTH, -HALF_HEIGHT, 0.0],
        [HALF_WIDTH, HALF_HEIGHT, 0.0],
        [-HALF_WIDTH, HALF_HEIGHT, 0.0],
    ];

    std::array::from_fn(|i| BillboardVertex {
        position: corners[i],
        diffuse: DIFFUSE,
        uv: UV[i],
    })
}

impl Column {
    pub fn new(x: f32, z: f32, rng: &mut Lcg) -> Column {
        let droplets = std::array::from_fn(|_| {
            let dx = x + X_BIAS - (rng.next() % JITTER_STEPS) as f32 * JITTER_STEP;

            let dz = z - (rng.next() % JITTER_STEPS) as f32 * JITTER_STEP;

            let dy = (rng.next() % HEIGHT_MODULUS) as f32;

            let rate = SPEEDS[(rng.next() % SPEEDS.len() as u32) as usize];

            Droplet {
                x: dx,
                y: dy,
                z: dz,
                rate,
            }
        });

        Column {
            x,
            z,
            droplets,
            stamp_ms: 0,
        }
    }

    pub fn tick(
        &mut self,
        now_ms: u32,
        rng: &mut Lcg,
        target: Option<Vec3>,
        ground: impl Fn(f32, f32) -> f32,
    ) -> Spawned {
        self.tick_on_surface(now_ms, rng, target, |x, z| splash_y(ground(x, z)))
    }

    pub fn tick_on_surface(
        &mut self,
        now_ms: u32,
        rng: &mut Lcg,
        target: Option<Vec3>,
        surface: impl Fn(f32, f32) -> f32,
    ) -> Spawned {
        let mut out = Spawned::default();

        let dt = now_ms.wrapping_sub(self.stamp_ms) as f32 * FALL_SCALE;

        for i in 0..DROPLETS {
            if self.droplets[i].y > FLOOR {
                self.droplets[i].y -= self.droplets[i].rate * dt * FALL_HALF;

                continue;
            }

            if rng.next().is_multiple_of(SPLASH_ODDS) {
                let at = Vec3::new(
                    self.droplets[i].x,
                    surface(self.droplets[i].x, self.droplets[i].z),
                    self.droplets[i].z,
                );

                out.splashes.push(splash(at));

                if let Some(target) = target
                    && plane_distance(target, at) < AUDIBLE
                {
                    out.sounds.push(SOUND_DROPWATER);
                }
            }

            if let Some(target) = target {
                self.droplets[i].y = target.y + SPAWN_ABOVE;

                self.droplets[i].x =
                    self.x + X_BIAS - (rng.next() % JITTER_STEPS) as f32 * JITTER_STEP;

                self.droplets[i].z = self.z - (rng.next() % JITTER_STEPS) as f32 * JITTER_STEP;
            }
        }

        self.stamp_ms = now_ms;

        out
    }

    pub fn translations(&self) -> impl Iterator<Item = Vec3> + '_ {
        self.droplets.iter().map(|d| Vec3::new(d.x, d.y, d.z))
    }
}

pub fn window_sampler<'a>(field: &'a HeightField<'a>) -> impl Fn(f32, f32) -> f32 + 'a {
    move |x, z| field.byte(x as i32, z as i32) as f32
}

pub fn plane_distance(a: Vec3, b: Vec3) -> f32 {
    let (dx, dz) = (a.x - b.x, a.z - b.z);

    (dx * dx + dz * dz).sqrt()
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

pub const SPLASH_ARGB: u32 = 0xFFFF_FFFF;

pub fn build_columns(records: &[ObjRecord], zone: (u8, u8), rng: &mut Lcg) -> Vec<Column> {
    records
        .iter()
        .filter(|r| r.class() == ObjectClass::Drop)
        .map(|r| {
            let [x, _, z] = r.world_position(u32::from(zone.0), u32::from(zone.1));

            Column::new(x, z, rng)
        })
        .collect()
}
