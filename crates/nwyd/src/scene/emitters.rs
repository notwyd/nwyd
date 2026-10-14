use super::effects::Billboard;
use formats::world::objects::{ObjRecord, ObjectClass};
use glam::Vec3;
use render::billboard::BillboardPass;

pub const FRAME_MS: u32 = 0x50;

pub const HALO_SLOT: usize = 2;

pub const HALO_SCALE: f32 = 2.8;

pub const HALO_COLOUR: u32 = 0x5555_3300;

pub const FIRE_SLOT: usize = 0x0B;

pub const FIRE_FRAMES: u32 = 8;

pub const FIRE_COLOUR: u32 = 0xEEEE_CC00;

pub const CAMPFIRE_SLOT: usize = 0x3D;

pub const CAMPFIRE_FRAMES: u32 = 6;

pub const GFIRE_SLOT: usize = 0x65;

pub const GFIRE_FRAMES: u32 = 8;

pub const GFIRE_COLOUR: u32 = 0xFF55_00FF;

pub const GFIRE_INNER_SCALE: f32 = 0.5;

pub const GFIRE_INNER_COLOUR: u32 = 0xFFFF_FFFF;

pub const GFIRE_INNER_DROP: f32 = 0.2;

pub const GFIRE_HALO_COLOUR: u32 = 0xFF33_0055;

pub const FLARE_SLOT: usize = 0x38;

pub const FLARE_COLOUR: u32 = 0xFFFF_0000;

pub const FLARE_COLOUR_MODE: u32 = 2;

pub fn cell_key(x: f32, z: f32) -> i32 {
    ((z as i32 >> 5) << 16) | (x as i32 >> 5)
}

pub fn spawn(record: &ObjRecord, world: [f32; 3], seed: u32) -> Vec<Billboard> {
    if record.kind == 2035 {
        return [(423, Vec3::new(1.7, 3.6, 1.7), 3.57), (424, Vec3::ONE, 0.5)]
            .into_iter()
            .map(|(slot, size, lift)| {
                let mut billboard = Billboard::new(slot, 0, size, 0.0, 1, 80);

                billboard.colour_mode = 0;

                billboard.pass = BillboardPass::Additive;

                billboard.position = Vec3::from(world) + Vec3::Y * lift;

                billboard.sector = cell_key(world[0], world[2]);

                billboard.glow = super::glow::Glow::Breathe;

                billboard
            })
            .collect();
    }

    if record.kind == 1846 {
        let mut built = Vec::with_capacity(32);

        let mut push = |slot, frames, scale, colour, position| {
            let mut b = Billboard::new(slot, 0, Vec3::splat(scale), 0.0, frames, 80);

            b.colour_mode = 0;

            b.set_color(colour);

            b.pass = BillboardPass::Additive;

            b.position = position;

            b.sector = cell_key(world[0], world[2]);

            built.push(b);
        };

        for index in 0..12 {
            let angle = index as f32 * std::f32::consts::TAU / 12.0;

            let (sin, cos) = angle.sin_cos();

            let at = Vec3::from(world) + Vec3::new(0.5 + cos * 3.0, 2.9, sin * 3.0);

            push(2, 1, 2.8, 0x55553300, at);

            push(11, 8, 1.0, 0xEEEECC00, at);
        }

        for index in 0..8 {
            let angle = index as f32 * std::f32::consts::TAU / 8.0;

            let (sin, cos) = angle.sin_cos();

            let at = Vec3::from(world) + Vec3::new(0.5 + cos * 2.25, 4.65, sin * 2.25);

            push(2, 1, 2.8, 0x55553300, at);
        }

        return built;
    }

    let Some([e0, e1]) = record.extra else {
        return Vec::new();
    };

    let position = Vec3::from(world);

    let size = Vec3::new(e0, e1, e0);

    let cell = cell_key(position.x, position.z);

    let mut built = Vec::new();

    let mut push = |slot: usize, scale: Vec3, frames: u32, colour: Option<u32>, at: Vec3| {
        let mut b = Billboard::new(slot, 0, scale, 0.0, frames, FRAME_MS);

        b.colour_mode = 0;

        if let Some(argb) = colour {
            b.set_color(argb);
        }

        b.position = at;

        b.pass = BillboardPass::Additive;

        b.sector = cell;

        b.phase_seed = seed;

        built.push(b);
    };

    match record.kind {
        0x1F5 => {
            push(FIRE_SLOT, size, FIRE_FRAMES, Some(FIRE_COLOUR), position);

            push(HALO_SLOT, size * HALO_SCALE, 1, Some(HALO_COLOUR), position);
        }
        0x1F6 => {
            push(CAMPFIRE_SLOT, size, CAMPFIRE_FRAMES, None, position);

            push(HALO_SLOT, size * HALO_SCALE, 1, Some(HALO_COLOUR), position);
        }
        0x1F7 => {
            push(GFIRE_SLOT, size, GFIRE_FRAMES, Some(GFIRE_COLOUR), position);

            let inner = Vec3::new(position.x, position.y - e1 * GFIRE_INNER_DROP, position.z);

            let scale = size * GFIRE_INNER_SCALE;

            push(
                GFIRE_SLOT,
                scale,
                GFIRE_FRAMES,
                Some(GFIRE_INNER_COLOUR),
                inner,
            );

            push(
                HALO_SLOT,
                size * HALO_SCALE,
                1,
                Some(GFIRE_HALO_COLOUR),
                position,
            );
        }
        0x1F8 => {
            push(FLARE_SLOT, size, 1, Some(FLARE_COLOUR), position);

            built[0].colour_mode = FLARE_COLOUR_MODE;
        }
        0x1F9 => {
            push(79, size, 1, Some(0x33330000), position);

            built[0].facing = false;

            built[0].roll = record.yaw;
        }
        _ => {}
    }

    built
}

pub fn build(records: &[ObjRecord], zone: (u8, u8)) -> Vec<Billboard> {
    let mut out = Vec::new();

    for (index, record) in records.iter().enumerate() {
        if record.class() != ObjectClass::Particle && !matches!(record.kind, 1846 | 2035) {
            continue;
        }

        let world = record.world_position(u32::from(zone.0), u32::from(zone.1));

        out.extend(spawn(record, world, index as u32));
    }

    out
}
