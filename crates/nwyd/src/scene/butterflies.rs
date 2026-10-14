use super::rng::Lcg;
use formats::world::objects::{ObjRecord, ObjectClass};
use glam::Vec3;
use render::skin::{SkinFog, SkinParams};

pub const BUTTERFLY_MODEL: u32 = 0x45;

pub const BIRD_MODEL: u32 = 0x18;

pub const BUTTERFLY_SLOT: usize = 0;

pub const FLIGHT: usize = 5;

pub const CYCLE_MS: u32 = 20_000;

pub const LAP_MS: u32 = 7_000;

pub const MIRROR_WINDOW: f32 = 0.01;

pub const BOB: f32 = 6.0;

pub fn appearance_variant(kind: u32) -> u32 {
    match kind {
        0x157 => 0,
        4 => 1,
        7 => 2,
        _ => 3,
    }
}

pub fn shape(kind: u32, texture: i16) -> (f32, u32) {
    match appearance_variant(kind) {
        1 => (if texture == 3 { 1.0 } else { 0.7 }, 10),
        2 => (0.2, 4),
        3 => (0.5, 8),
        _ => (0.5, 15),
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Butterfly {
    pub record: usize,
    pub kind: u32,
    pub instance: usize,
    pub origin: Vec3,
    pub yaw: f32,
    pub arm: u32,
    pub texture: i16,
    pub model_id: u32,
    pub model: Option<usize>,
    pub anim: Option<super::actors::Anim>,
    pub rate: f32,
    pub reach: f32,
    pub rise: f32,
    pub scale: f32,
    pub sub_frame_ms: u32,
    pub phase_ms: u32,
    seed: u32,
}

impl Butterfly {
    pub fn set_clip(&mut self, slot: u32, now_ms: u32, models: &[super::actors::Model]) -> bool {
        let Some(model) = self.model.and_then(|i| models.get(i)) else {
            return false;
        };

        if slot >= model.animation.num_anims {
            return false;
        }

        let mut anim = self
            .anim
            .unwrap_or_else(|| super::actors::Anim::new(0, self.sub_frame_ms));

        if !anim.set_clip(slot as usize, now_ms, model) {
            return false;
        }

        self.anim = Some(anim);

        true
    }

    pub fn flight(record: usize, kind: u32, world: [f32; 3]) -> Vec<Butterfly> {
        let arm = appearance_variant(kind);

        (0..FLIGHT)
            .map(|instance| {
                let mut rng = Lcg::for_object(record, instance);

                let n = rng.next_mod(10);

                let motion = if kind == 7 { 3 } else { n % 3 };

                let mut rate = n as f32 * 0.1 + 1.0;

                let mut reach = n as f32 * 0.5 + 3.0;

                let mut rise = n as f32 * 0.05 + 0.2;

                let texture = match arm {
                    0 => rng.next_mod(3) as i16,
                    1 => (rng.next_mod(2) + 3) as i16,
                    2 => 2,
                    3 => 5,
                    _ => 0,
                };

                let yaw = if kind == 7 {
                    -std::f32::consts::FRAC_PI_2
                } else {
                    rng.next_mod(4) as f32 * std::f32::consts::PI / 12.0
                };

                let jitter_x = rng.next_mod(5) as f32 * 0.1;

                let jitter_y = rng.next_mod(10) as f32 * 0.2;

                let jitter_z = rng.next_mod(5) as f32 * 0.1;

                if kind == 6 {
                    reach *= 0.5;

                    rise *= 0.5;

                    rate = instance as f32 + 8.0;
                } else if kind == 7 {
                    reach = 5.0;

                    rise = 5.0;

                    rate = instance as f32 + 8.0;
                }

                let (scale, sub_frame_ms) = shape(kind, texture);

                Butterfly {
                    record,
                    kind,
                    instance,
                    origin: Vec3::new(
                        world[0] + jitter_x,
                        world[1] + jitter_y + if kind == 7 { 6.0 } else { 0.0 },
                        world[2] + jitter_z,
                    ),
                    yaw,
                    arm: motion,
                    texture,
                    model_id: if kind == 7 {
                        BIRD_MODEL
                    } else {
                        BUTTERFLY_MODEL
                    },
                    model: None,
                    anim: None,
                    rate,
                    reach,
                    rise,
                    scale,
                    sub_frame_ms,
                    phase_ms: if kind == 7 { instance as u32 * 200 } else { 0 },
                    seed: rng.next(),
                }
            })
            .collect()
    }

    pub fn sweep(&self, now_ms: u32) -> f32 {
        let t = (now_ms.wrapping_sub(self.phase_ms) % CYCLE_MS) as f32 / CYCLE_MS as f32;

        (std::f32::consts::PI * t).sin().abs()
    }

    pub fn lap(&self, now_ms: u32) -> f32 {
        let now_ms = now_ms.wrapping_sub(self.phase_ms);

        let f = (now_ms % LAP_MS) as f32 / LAP_MS as f32;

        if f >= MIRROR_WINDOW {
            return f;
        }

        let mut rng = Lcg::new(self.seed);

        for _ in 0..now_ms / LAP_MS {
            rng.next();
        }

        if rng.next_mod(2) == 0 { -f } else { f }
    }

    pub fn position(&self, now_ms: u32) -> Vec3 {
        let w = self.sweep(now_ms);

        let pi = std::f32::consts::PI;

        let bob = pi * w * BOB * self.rate;

        let (x, y, z) = match self.arm {
            0 => (
                self.reach * w * 0.5,
                self.rise * bob.sin(),
                self.reach * (pi * w * self.rate).sin() * 0.5,
            ),
            1 => (
                self.reach * (pi * w * self.rate).cos() * 0.5,
                self.rise * bob.cos(),
                self.reach * w * 0.5,
            ),
            2 => {
                let turn = 2.0 * pi * w;

                (
                    self.reach * w * turn.cos() * 0.5,
                    self.rise * bob.cos(),
                    self.reach * w * turn.sin() * 0.5,
                )
            }
            _ => {
                let turn = 2.0 * pi * self.lap(now_ms);

                (
                    self.reach * turn.cos() * 0.5,
                    self.rise * turn.cos() * 0.5,
                    self.reach * turn.sin() * 0.5,
                )
            }
        };

        Vec3::new(self.origin.x + x, self.origin.y + y, self.origin.z + z)
    }

    pub fn pose_ms(&self, now_ms: u32) -> u32 {
        let phase = self.reach * 10.0 + self.rise * 100.0 + (self.arm * 5) as f32;

        now_ms.wrapping_add(phase as i32 as u32)
    }

    pub fn behind_camera(&self, now_ms: u32, eye: Vec3, forward: Vec3) -> bool {
        forward.dot(self.position(now_ms) - eye) <= 0.0
    }

    pub fn params(
        &self,
        distance: f32,
        light0: [f32; 4],
        light1: [f32; 4],
        fog: SkinFog,
    ) -> SkinParams {
        let (diffuse, emissive) = material(distance, light0, light1);

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

pub fn material(distance: f32, light0: [f32; 4], light1: [f32; 4]) -> ([f32; 4], [f32; 4]) {
    let mut diffuse = [0.0f32; 4];

    let mut emissive = [0.0f32; 4];

    for i in 0..3 {
        diffuse[i] = light0[i] * 0.3 + 0.7;

        emissive[i] = diffuse[i] * 0.4 + light1[i] * 0.4;
    }

    diffuse[3] = distance;

    (diffuse, emissive)
}

#[derive(Debug, Clone, Default, PartialEq)]

pub struct Flight {
    pub butterflies: Vec<Butterfly>,
}

#[derive(Debug, Clone, PartialEq)]

pub struct OwnedFlight {
    pub butterflies: [Butterfly; 4],
    pub birth_ms: Option<u32>,
}

pub fn owned(
    class: i32,
    part: formats::mesh::appearance::PartRef,
    rng: &mut Lcg,
) -> Option<OwnedFlight> {
    if !((class == 21 && part.mesh == 10)
        || (class == 28 && part.mesh == 2)
        || (class == 25 && (part.mesh == 12 || part.mesh == 3 && part.tex == 8)))
    {
        return None;
    }

    Some(OwnedFlight {
        birth_ms: None,
        butterflies: std::array::from_fn(|instance| {
            let n = rng.next_mod(10);

            Butterfly {
                record: 0,
                kind: 6,
                instance,
                origin: Vec3::ZERO,
                yaw: instance as f32 * std::f32::consts::PI / 6.0,
                arm: n % 3,
                texture: 5,
                model_id: 69,
                model: None,
                anim: None,
                rate: instance as f32 + 8.0,
                reach: (n as f32 * 0.5 + 3.0) * 0.5,
                rise: (n as f32 * 0.05 + 0.2) * 0.5,
                scale: 0.5,
                sub_frame_ms: 8,
                phase_ms: 0,
                seed: 0,
            }
        }),
    })
}

impl OwnedFlight {
    pub fn position(&self, index: usize, now: u32, owner: Vec3) -> Option<Vec3> {
        let b = self.butterflies.get(index)?;

        let age = now.wrapping_sub(self.birth_ms?);

        Some(b.position(age) + owner + Vec3::Y * if b.arm == 2 { 2.0 } else { 1.5 })
    }
}

impl Flight {
    pub fn build(records: &[ObjRecord], zone: (u8, u8)) -> Flight {
        let mut butterflies = Vec::new();

        for (record, r) in records.iter().enumerate() {
            if r.class() != ObjectClass::Butterfly {
                continue;
            }

            let world = r.world_position(u32::from(zone.0), u32::from(zone.1));

            butterflies.extend(Butterfly::flight(record, r.kind, world));
        }

        Flight { butterflies }
    }

    pub fn specs(&self) -> Vec<(u32, i16, u32)> {
        let mut v: Vec<(u32, i16, u32)> = self
            .butterflies
            .iter()
            .map(|b| (b.model_id, b.texture, b.sub_frame_ms))
            .collect();

        v.sort_unstable();

        v.dedup();

        v
    }
}
