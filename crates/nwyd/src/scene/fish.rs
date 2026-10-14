use super::rng::Lcg;
use formats::world::objects::{ObjRecord, ObjectClass};
use glam::Vec3;
use render::skin::{SkinFog, SkinParams};

pub const FISH_MODEL: u32 = 0x46;

pub const FISH_SUB_FRAME_MS: u32 = 0x1E;

pub const FISH_SLOT: usize = 0;

pub const SCHOOL: usize = 5;

pub const CYCLE_MS: u32 = 40_000;

pub const ARMS: u32 = 3;

pub const PHASE_RATE: f32 = 0.05;

pub const WIDE_X: f32 = 0.4;

pub const WIDE_Z: f32 = 0.3;

pub const TIGHT: f32 = 0.1;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Fish {
    pub record: usize,
    pub kind: u32,
    pub instance: usize,
    pub ordinal: usize,
    pub origin: Vec3,
    pub arm: u32,
    pub mesh: i16,
    pub model: Option<usize>,
    pub refine: u8,
    pub glow: Option<super::actors::PartGlow>,
    pub speed: f32,
    pub swim: f32,
    pub amplitude: f32,
}

impl Fish {
    pub fn school(record: usize, kind: u32, world: [f32; 3], ordinal: usize) -> Vec<Fish> {
        (0..SCHOOL)
            .map(|instance| {
                let mut rng = Lcg::for_object(record, instance);

                let arm = rng.next_mod(ARMS);

                let speed = rng.next_mod(10) as f32 * 0.1 + 1.0;

                let (mesh, refine) = if kind == 0x158 {
                    let mesh = rng.next_mod(3) as i16;

                    (mesh, (rng.next_mod(3) + 7) as u8)
                } else {
                    (0, 4)
                };

                let swim = rng.next_mod(10) as f32 * 0.1 + 0.7;

                let amplitude = rng.next_mod(7) as f32 * 0.5 + 3.0;

                let jitter_x = rng.next_mod(5) as f32 * 0.05;

                let jitter_y = rng.next_mod(10) as f32 * 0.02;

                let jitter_z = rng.next_mod(5) as f32 * 0.05;

                Fish {
                    record,
                    kind,
                    instance,
                    ordinal: ordinal + instance,
                    origin: Vec3::new(
                        world[0] + jitter_x,
                        world[1] + jitter_y,
                        world[2] + jitter_z,
                    ),
                    arm,
                    mesh,
                    model: None,
                    refine,
                    glow: None,
                    speed,
                    swim,
                    amplitude,
                }
            })
            .collect()
    }

    pub fn phase_ms(&self) -> i32 {
        let terms = self.speed * 10.0
            + self.amplitude * 12.0
            + self.swim * 12.0
            + ((self.ordinal % 100) * 20) as f32
            + f32::from(self.mesh) * 10.0
            + (self.arm * 5) as f32;

        terms as i32
    }

    pub fn cycle(&self, now_ms: u32) -> f32 {
        let t = now_ms.wrapping_add(self.phase_ms() as u32) % CYCLE_MS;

        (std::f32::consts::TAU * t as f32 / CYCLE_MS as f32).sin()
    }

    pub fn position(&self, now_ms: u32) -> Vec3 {
        let s = self.cycle(now_ms);

        let phase = self.phase_ms() as f32 * PHASE_RATE;

        let (x, z) = match self.arm {
            0 => (
                self.amplitude * s * WIDE_X,
                self.amplitude * (std::f32::consts::PI * s * self.swim + phase).sin() * WIDE_Z,
            ),
            1 => (
                self.amplitude * (std::f32::consts::PI * s * self.swim + phase).cos() * WIDE_X,
                self.amplitude * s * WIDE_Z,
            ),
            _ => {
                let t = 2.0 * s * std::f32::consts::PI + phase;

                (
                    self.amplitude * t.cos() * TIGHT,
                    self.amplitude * t.sin() * TIGHT,
                )
            }
        };

        Vec3::new(self.origin.x + x, self.origin.y, self.origin.z + z)
    }

    pub fn yaw(&self, now_ms: u32) -> f32 {
        let at = self.position(now_ms);

        at.x.atan2(at.z)
    }

    pub fn pose_ms(&self, now_ms: u32) -> u32 {
        now_ms.wrapping_add(self.phase_ms() as u32)
    }

    pub fn behind_camera(&self, now_ms: u32, eye: Vec3, forward: Vec3) -> bool {
        forward.dot(self.position(now_ms) - eye) <= 0.0
    }

    pub fn params(
        &self,
        terrain: [f32; 4],
        light0: [f32; 4],
        light1: [f32; 4],
        fog: SkinFog,
    ) -> SkinParams {
        let (diffuse, emissive) = material(terrain, light0, light1);

        SkinParams {
            diffuse,
            ambient: [0.0; 4],
            emissive,
            unlit: false,
            fog_start: fog.start,
            fog_end: fog.end,
            fog_enabled: fog.enabled,
            force_opaque: false,
            modulate_alpha: false,
            glow: None,
        }
    }
}

pub fn material(terrain: [f32; 4], light0: [f32; 4], light1: [f32; 4]) -> ([f32; 4], [f32; 4]) {
    let mut diffuse = [0.0f32; 4];

    let mut emissive = [0.0f32; 4];

    for i in 0..3 {
        diffuse[i] = terrain[i] * 0.2 + light0[i] * 0.5;

        emissive[i] = diffuse[i] * 0.4 + light1[i] * 0.4;
    }

    diffuse[3] = 0.0;

    (diffuse, emissive)
}

#[derive(Debug, Clone, Default, PartialEq)]

pub struct School {
    pub fish: Vec<Fish>,
}

impl School {
    pub fn build(records: &[ObjRecord], zone: (u8, u8), ordinal: usize) -> School {
        let mut fish = Vec::new();

        for (record, r) in records.iter().enumerate() {
            if r.class() != ObjectClass::Fish {
                continue;
            }

            let world = r.world_position(u32::from(zone.0), u32::from(zone.1));

            fish.extend(Fish::school(record, r.kind, world, ordinal + fish.len()));
        }

        School { fish }
    }

    pub fn meshes(&self) -> Vec<i16> {
        let mut v: Vec<i16> = self.fish.iter().map(|f| f.mesh).collect();

        v.sort_unstable();

        v.dedup();

        v
    }
}
