use super::view_distance::ViewDistance;
use formats::world::objects::{ObjRecord, ObjectClass};
use formats::world::trn::Terrain;
use glam::Vec3;
use render::skin::{SkinFog, SkinParams};

#[cfg(test)]
mod tests;

pub const LEAF_MODEL: u32 = 0x3D;

pub const LEAF_SUB_FRAME_MS: u32 = 0x50;

pub const LEAF_SLOT: usize = 0;

pub const SPECIAL_ZONE_COLOUR: u32 = 9;

pub const CULL_RADIUS_SQUARED: f32 = 900.0;

pub const FADE_START_SQUARED: f32 = 400.0;

pub const FADE_RANGE: f32 = 384.0;

pub const PHASE_STEP_MS: u32 = 500;

pub const PHASE_STEPS: u32 = 9;

pub const INNER_TERRAIN_MIX: f32 = 0.5;

pub const OUTER_LIGHT_MIX: f32 = 0.8;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blade {
    pub zone: (u8, u8),
    pub record: usize,
    pub kind: u32,
    pub colour: u32,
    pub model: Option<usize>,
    pub position: Vec3,
    pub yaw: f32,
    pub cell: i32,
    pub phase_ms: u32,
    pub terrain_colour: [f32; 4],
    pub emissive: [f32; 4],
}

impl Blade {
    pub fn distance_squared(&self, eye: Vec3) -> f32 {
        let (dx, dz) = (self.position.x - eye.x, self.position.z - eye.z);

        dx * dx + dz * dz
    }

    pub fn alpha(distance_squared: f32) -> f32 {
        if distance_squared <= FADE_START_SQUARED {
            return 1.0;
        }

        1.0 - (distance_squared - FADE_START_SQUARED) / FADE_RANGE
    }

    pub fn cell_key(x: f32, z: f32) -> i32 {
        ((z as i32 >> 5) << 16) | (x as i32 >> 5)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Field {
    pub model: Option<usize>,
    pub blades: Vec<Blade>,
    pub enabled: bool,
    pub inner_zone: bool,
    pub view_distance: ViewDistance,
}

impl Field {
    pub fn build(
        records: &[ObjRecord],
        terrain: &Terrain,
        window: &[i8],
        light0: [f32; 4],
        enabled: bool,
    ) -> Field {
        let inner_zone = crate::scene::world::is_special_zone(terrain.zone_x, terrain.zone_y);

        let (ox, oz) =
            formats::world::zone_origin(u32::from(terrain.zone_x), u32::from(terrain.zone_y));

        let mut blades = Vec::new();

        for (record, r) in records.iter().enumerate() {
            if r.class() != ObjectClass::Grass {
                continue;
            }

            let [x, own, z] =
                r.world_position(u32::from(terrain.zone_x), u32::from(terrain.zone_y));

            let y = super::actors::snap_height(window, (terrain.zone_x, terrain.zone_y), x, z, own);

            let ordinal = blades.len() as u32;

            blades.push(Blade {
                zone: (terrain.zone_x, terrain.zone_y),
                record,
                kind: r.kind,
                colour: colour(r.kind, inner_zone),
                model: None,
                position: Vec3::new(x, y, z),
                yaw: r.yaw,
                cell: Blade::cell_key(x, z),
                phase_ms: (ordinal % PHASE_STEPS) * PHASE_STEP_MS,
                terrain_colour: terrain.color_at(x - ox, z - oz),
                emissive: emissive(terrain.color_at(x - ox, z - oz), light0, inner_zone),
            });
        }

        Field {
            model: None,
            blades,
            enabled,
            inner_zone,
            view_distance: ViewDistance::Retail,
        }
    }

    pub fn refresh_light(&mut self, light0: [f32; 4]) {
        for blade in &mut self.blades {
            blade.emissive = emissive(
                blade.terrain_colour,
                light0,
                crate::scene::world::is_special_zone(blade.zone.0, blade.zone.1),
            );
        }
    }

    pub fn texture_colour(&self, kind: u32) -> u32 {
        colour(kind, self.inner_zone)
    }

    pub fn mesh_index(kind: u32) -> u32 {
        if (0x137..=0x13C).contains(&kind) {
            0
        } else {
            2
        }
    }

    pub fn visible(&self, eye: Vec3) -> impl Iterator<Item = (usize, &Blade, f32)> {
        let multiplier = self.view_distance.multiplier();
        let cull_radius_squared = CULL_RADIUS_SQUARED * multiplier * multiplier;

        self.blades
            .iter()
            .enumerate()
            .filter_map(move |(i, blade)| {
                let d2 = blade.distance_squared(eye);

                (self.enabled && d2 <= cull_radius_squared).then_some((i, blade, d2))
            })
    }

    pub fn bucket_bounds(&self, eye: Vec3) -> (usize, usize) {
        let inside: Vec<i32> = self.visible(eye).map(|(_, b, _)| b.cell).collect();

        let (cx, cz) = (eye.x as i32 >> 5, eye.z as i32 >> 5);

        let mut counts = Vec::with_capacity(4);

        for sx in [-1, 1] {
            for sz in [-1, 1] {
                let cells = [
                    (cz << 16) | cx,
                    (cz << 16) | (cx + sx),
                    ((cz + sz) << 16) | cx,
                    ((cz + sz) << 16) | (cx + sx),
                ];

                counts.push(inside.iter().filter(|c| cells.contains(c)).count());
            }
        }

        (
            counts.iter().copied().min().unwrap_or(0),
            counts.iter().copied().max().unwrap_or(0),
        )
    }

    pub fn params(&self, blade: &Blade, distance_squared: f32, fog: SkinFog) -> SkinParams {
        let multiplier = self.view_distance.multiplier();
        let distance_squared = distance_squared / (multiplier * multiplier);

        SkinParams {
            diffuse: [0.0, 0.0, 0.0, Blade::alpha(distance_squared)],
            ambient: [0.0; 4],
            emissive: blade.emissive,
            unlit: true,
            fog_start: fog.start,
            fog_end: fog.end,
            fog_enabled: fog.enabled,
            force_opaque: false,
            modulate_alpha: true,
            glow: None,
        }
    }
}

fn colour(kind: u32, inner: bool) -> u32 {
    if (0x137..=0x13C).contains(&kind) {
        if inner {
            SPECIAL_ZONE_COLOUR
        } else {
            kind.wrapping_sub(0x37) & 0xFF
        }
    } else {
        kind.wrapping_sub(0x39) & 0xFF
    }
}

pub fn emissive(terrain: [f32; 4], light0: [f32; 4], inner_zone: bool) -> [f32; 4] {
    let mut out = [0.0f32; 4];

    for i in 0..3 {
        out[i] = if inner_zone {
            terrain[i] * INNER_TERRAIN_MIX + light0[i] * INNER_TERRAIN_MIX
        } else {
            light0[i] * OUTER_LIGHT_MIX
        };
    }

    out
}
