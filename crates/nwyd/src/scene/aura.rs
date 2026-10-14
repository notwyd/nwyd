use super::effects::{Billboard, Motion};
use super::rng::Lcg;
use glam::Vec3;
use render::billboard::BillboardPass;
use render::camera::Matrix4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Detail {
    pub effects: bool,
    pub glow: bool,
}

impl Default for Detail {
    fn default() -> Detail {
        Detail {
            effects: true,
            glow: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Arm {
    AuraLow,
    AuraHigh,
    Skill,
    Glow,
    Sanc,
    Spark,
    WeaponEffect,
    Panel,
}

impl Arm {
    pub const ALL: [Arm; 8] = [
        Arm::AuraLow,
        Arm::AuraHigh,
        Arm::Skill,
        Arm::Glow,
        Arm::Sanc,
        Arm::Spark,
        Arm::WeaponEffect,
        Arm::Panel,
    ];

    pub fn cooldown_ms(self) -> Option<u32> {
        match self {
            Arm::AuraLow => Some(100),
            Arm::AuraHigh => Some(0x1E),
            Arm::Glow => Some(0x32),
            Arm::Sanc => Some(0x96),
            Arm::Spark => Some(100),
            Arm::Skill | Arm::WeaponEffect | Arm::Panel => None,
        }
    }

    pub fn winds_clock(self) -> bool {
        matches!(
            self,
            Arm::AuraHigh | Arm::Skill | Arm::Glow | Arm::Sanc | Arm::Spark
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Emitters {
    pub aura_mode: i32,
    pub glow: bool,
    pub spark: bool,
    pub sanc: bool,
    pub weapon_effect: i32,
    pub skill: bool,
    pub panel: bool,
}

pub const AURA_MODE_SWING: i32 = 4;

impl Emitters {
    pub fn from_flags(flags: &[u8], base: usize) -> Emitters {
        let at = |offset: usize| flags.get(offset - base).copied().unwrap_or(0);

        let aura = at(formats::net::affect::Flags::AURA);

        let suppress = at(formats::net::affect::Flags::AURA_SUPPRESS);

        Emitters {
            aura_mode: i32::from(aura == 1 && suppress == 0),
            glow: at(formats::net::affect::Flags::WEAPON_GLOW) != 0,
            spark: at(formats::net::affect::Flags::WEAPON_SPARK) != 0,
            sanc: at(formats::net::affect::Flags::TRAIL) != 0,
            weapon_effect: i32::from(at(formats::net::affect::Flags::WEAPON_EFFECT) as i8),
            skill: false,
            panel: false,
        }
    }

    pub fn take_affects(&mut self, incoming: &Emitters) {
        let Emitters {
            aura_mode,
            glow,
            spark,
            sanc,
            weapon_effect,
            skill: _,
            panel: _,
        } = *incoming;

        self.aura_mode = aura_mode;

        self.glow = glow;

        self.spark = spark;

        self.sanc = sanc;

        self.weapon_effect = weapon_effect;
    }

    pub fn fires(&self, arm: Arm) -> bool {
        match arm {
            Arm::AuraLow => self.aura_mode == 1,
            Arm::AuraHigh => self.aura_mode == AURA_MODE_SWING,
            Arm::Skill => self.skill,
            Arm::Glow => self.glow,
            Arm::Sanc => self.sanc,
            Arm::Spark => self.spark,
            Arm::WeaponEffect => self.weapon_effect != 0,
            Arm::Panel => self.panel,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cascade {
    pub point: Vec3,
}

pub const TIP_SCALE: f32 = 1.5;

pub const TEXTURE_SLOTS: [usize; 19] = [
    0x00, 0x0B, 0x0C, 0x0D, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x38, 0x3C, 0x65, 0x66, 0x67, 0x68, 0x69,
    0x6A, 0x6B, 0x6C,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Clock {
    pub stamp_ms: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Frame {
    pub spawned: Vec<Billboard>,
    pub fired: Vec<Arm>,
}

impl Frame {
    pub fn sink(&mut self) -> Sink<'_> {
        Sink {
            spawned: &mut self.spawned,
            fired: &mut self.fired,
        }
    }
}

pub struct Sink<'a> {
    pub spawned: &'a mut Vec<Billboard>,
    pub fired: &'a mut Vec<Arm>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Blade {
    pub world: Matrix4,
    pub size: f32,
    pub radius: f32,
}

pub fn frame(
    emitters: &Emitters,
    detail: Detail,
    clock: &mut Clock,
    blade: Blade,
    now_ms: u32,
    rng: &mut Lcg,
    out: &mut Sink<'_>,
) {
    let Blade {
        world,
        size,
        radius,
    } = blade;

    if !detail.effects {
        return;
    }

    let grip = world.transform_coord(Vec3::ZERO);

    let tip = world.transform_coord(Vec3::new(0.0, 0.0, size * TIP_SCALE));

    let mut cascade = Cascade { point: grip };

    for arm in Arm::ALL {
        if !emitters.fires(arm) {
            continue;
        }

        if let Some(cooldown) = arm.cooldown_ms()
            && now_ms.wrapping_sub(clock.stamp_ms) <= cooldown
        {
            continue;
        }

        if arm == Arm::Glow && !detail.glow {
            continue;
        }

        let before = out.spawned.len();

        match arm {
            Arm::AuraLow => {
                let n = (rng.next() % 5) as f32;

                let wide = n * 0.3 + 0.1;

                let tall = n * 0.15;

                out.spawned.push(
                    Billboard::new(0xB, 1000, Vec3::new(wide, tall, wide), 0.0, 8, 0x50)
                        .additive()
                        .lifted()
                        .at(grip)
                        .argb(0xFF11_1105),
                );
            }
            Arm::AuraHigh | Arm::Skill => {
                cascade.point = tip;

                let n = (rng.next() % 5) as f32;

                let wide = n * 0.1 + 0.8;

                let tall = n * 0.2 + 0.8;

                let argb = match arm {
                    Arm::AuraHigh => 0xFF00_55FF,
                    _ => 0xFFFF_FFFF,
                };

                out.spawned.push(
                    Billboard::new(0xB, 1000, Vec3::new(wide, tall, wide), 0.0, 8, 0x50)
                        .additive()
                        .at(raised(cascade.point))
                        .argb(argb),
                );
            }
            Arm::Glow => {
                let n = (rng.next() % 5) as f32;

                out.spawned.push(
                    Billboard::new(0x38, 500, Vec3::splat(0.3), 0.002, 1, 0x50)
                        .additive()
                        .lifted()
                        .at(raised(cascade.point))
                        .argb(0xFF55_0000),
                );

                let wide = n * 0.01 + 0.6;

                let tall = n * 0.01 + 0.3;

                out.spawned.push(
                    Billboard::new(0x3C, 500, Vec3::new(wide, tall, wide), 0.000_5, 1, 0x50)
                        .additive()
                        .lifted()
                        .rolled(n * std::f32::consts::PI / 3.0)
                        .at(raised(cascade.point))
                        .argb(0xFF55_3333),
                );
            }
            Arm::Sanc => {
                for _ in 0..3 {
                    let point = blade_point(world, radius, rng);

                    cascade.point = point;

                    let n = (rng.next() % 0x3C) as f32;

                    let scale = n * 0.01 + 0.2;

                    out.spawned.push(
                        Billboard::new(0x38, 500, Vec3::splat(scale), 0.000_5, 1, 0x50)
                            .additive()
                            .lifted()
                            .rolled(n * std::f32::consts::PI / 3.0)
                            .at(point)
                            .argb(0xFF33_3355),
                    );
                }
            }
            Arm::Spark => {
                let point = blade_point(world, radius, rng);

                cascade.point = point;

                let n = (rng.next() % 5) as f32;

                out.spawned.push(
                    Billboard::new(0x65, 500, Vec3::new(0.12, 0.2, 0.2), 0.002, 8, 0x50)
                        .additive()
                        .lifted()
                        .at(raised(point))
                        .argb(0xFF55_00FF),
                );

                let scale = n * 0.01 + 0.1;

                out.spawned.push(
                    Billboard::new(0x65, 500, Vec3::splat(scale), 0.000_5, 8, 0x50)
                        .additive()
                        .lifted()
                        .rolled(n * std::f32::consts::PI / 3.0)
                        .at(raised(point))
                        .argb(0xFFFF_FFFF),
                );
            }
            Arm::WeaponEffect => {
                out.spawned
                    .push(mote(cascade.point, rng, 0.02, 0.05, 1.0, 0xFF55_AAFF));

                cascade.point = world.transform_coord(Vec3::new(0.0, 0.0, size));
            }
            Arm::Panel => {
                out.spawned
                    .push(mote(cascade.point, rng, 0.01, 0.03, -1.0, 0xFFFF_FF55));
            }
        }

        if out.spawned.len() != before {
            out.fired.push(arm);

            if arm.winds_clock() {
                clock.stamp_ms = now_ms;
            }
        }
    }
}

fn blade_point(world: Matrix4, radius: f32, rng: &mut Lcg) -> Vec3 {
    let step = radius / 10.0;

    let z = radius - (rng.next() % 10) as f32 * step;

    world.transform_coord(Vec3::new(0.0, 0.0, z))
}

fn raised(point: Vec3) -> Vec3 {
    Vec3::new(point.x, point.y + 0.2, point.z)
}

fn mote(point: Vec3, rng: &mut Lcg, base: f32, tall_step: f32, rise: f32, argb: u32) -> Billboard {
    let n = (rng.next() % 5) as f32;

    let wide = n * base + base;

    let tall = n * tall_step + base;

    let jitter = |rng: &mut Lcg| ((rng.next() % 10) as i32 - 5) as f32 * 0.02;

    let x = point.x + jitter(rng);

    let z = point.z + jitter(rng);

    let at = Vec3::new(x, point.y, z);

    let mut board = Billboard::new(0, 0x5DC, Vec3::new(wide, tall, wide), 0.000_1, 1, 0x50)
        .additive()
        .at(at)
        .argb(argb);

    board.lift = false;

    board.motion = Motion::Rise;

    board.rise = rise;

    board.origin = at;

    board
}

trait Spawned {
    fn additive(self) -> Billboard;

    fn lifted(self) -> Billboard;

    fn rolled(self, roll: f32) -> Billboard;

    fn at(self, position: Vec3) -> Billboard;

    fn argb(self, argb: u32) -> Billboard;
}

impl Spawned for Billboard {
    fn additive(mut self) -> Billboard {
        self.pass = BillboardPass::Additive;

        self
    }

    fn lifted(mut self) -> Billboard {
        self.lift = true;

        self
    }

    fn rolled(mut self, roll: f32) -> Billboard {
        self.roll = roll;

        self
    }

    fn at(mut self, position: Vec3) -> Billboard {
        self.position = position;

        self
    }

    fn argb(mut self, argb: u32) -> Billboard {
        self.base = argb.to_be_bytes();

        self
    }
}
