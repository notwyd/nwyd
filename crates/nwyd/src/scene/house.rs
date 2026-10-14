use formats::world::attribute_map::{AttributeMap, flags};
use formats::world::objects::{self, ObjectClass};
use glam::Vec3;

pub fn swing(now_ms: u32) -> f32 {
    ((now_ms % 20_000) as f32 / 10_000.0) * std::f32::consts::PI
}

pub fn proximity_opacity(at: Vec3, target: Option<Vec3>) -> Option<f32> {
    target.map(|target| {
        if (target.x - at.x).abs() < 6.0 && (target.z - at.z).abs() < 6.0 {
            0.7
        } else {
            1.0
        }
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Yaw {
    Record,
    RecordQuarterTurn,
    Swing,
    NegSwing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pitch {
    Zero,
    Swing,
    NegSwing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Gate {
    Always,
    PlayerNotRoofed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Part {
    pub mesh_id: u32,
    pub offset: Vec3,
    pub rotate_offset: bool,
    pub yaw: Yaw,
    pub pitch: Pitch,
    pub roll: f32,
    pub uv_anim: u8,
    pub gate: Gate,
}

const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;

fn part(mesh_id: u32, dy: f32) -> Part {
    Part {
        mesh_id,
        offset: Vec3::new(0.0, dy, 0.0),
        rotate_offset: false,
        yaw: Yaw::Record,
        pitch: Pitch::Zero,
        roll: 0.0,
        uv_anim: 0,
        gate: Gate::Always,
    }
}

pub fn parts(kind: u32) -> Vec<Part> {
    if objects::classify(kind) != ObjectClass::House {
        return Vec::new();
    }

    let Some(variant) = objects::house_type(kind) else {
        return Vec::new();
    };

    match variant {
        0 => vec![Part {
            gate: Gate::PlayerNotRoofed,
            ..part(kind + 1, 0.0)
        }],
        1 => {
            vec![Part {
                yaw: Yaw::RecordQuarterTurn,
                pitch: Pitch::Swing,
                roll: HALF_PI,
                ..part(kind + 1, 5.22)
            }]
        }
        2 | 3 => vec![part(u32::from(objects::house_leaf_id(kind)), 0.0)],
        4 => {
            vec![
                Part {
                    yaw: Yaw::NegSwing,
                    ..part(kind + 1, 0.45)
                },
                Part {
                    yaw: Yaw::NegSwing,
                    ..part(kind + 1, 2.7)
                },
                Part {
                    yaw: Yaw::Swing,
                    ..part(kind + 2, 2.0)
                },
            ]
        }
        5 => {
            let (sin, cos) = 0.897_597_9_f32.sin_cos();

            vec![
                Part {
                    yaw: Yaw::RecordQuarterTurn,
                    pitch: Pitch::Swing,
                    roll: HALF_PI,
                    offset: Vec3::new(1.8 * cos, 1.2, 1.8 * sin),
                    rotate_offset: true,
                    ..part(kind + 1, 1.2)
                },
                Part {
                    yaw: Yaw::RecordQuarterTurn,
                    pitch: Pitch::NegSwing,
                    roll: HALF_PI,
                    offset: Vec3::new(-1.8 * cos, 1.2, 1.8 * sin),
                    rotate_offset: true,
                    ..part(kind + 1, 1.2)
                },
                part(kind + 2, 0.0),
            ]
        }
        6 => vec![Part {
            uv_anim: 6,
            ..part(kind + 1, 0.0)
        }],
        7 => vec![Part {
            uv_anim: 101,
            ..part(kind + 0x14, 0.0)
        }],
        8 => vec![Part {
            uv_anim: 100,
            ..part(kind + 0x20, 0.0)
        }],
        9 => vec![Part {
            uv_anim: 6,
            ..part(kind + 0x3D, 0.0)
        }],
        _ => Vec::new(),
    }
}

impl Part {
    pub fn offset_at(&self, record_yaw: f32) -> Vec3 {
        if !self.rotate_offset {
            return self.offset;
        }

        let (sin, cos) = record_yaw.sin_cos();

        Vec3::new(
            self.offset.x * cos + self.offset.z * sin,
            self.offset.y,
            -self.offset.x * sin + self.offset.z * cos,
        )
    }

    pub fn pose(&self, record_yaw: f32, swing: f32) -> (f32, f32, f32) {
        let yaw = match self.yaw {
            Yaw::Record => record_yaw,
            Yaw::RecordQuarterTurn => record_yaw - HALF_PI,
            Yaw::Swing => swing,
            Yaw::NegSwing => -swing,
        };

        let pitch = match self.pitch {
            Pitch::Zero => 0.0,
            Pitch::Swing => swing,
            Pitch::NegSwing => -swing,
        };

        (yaw, pitch, self.roll)
    }
}

pub fn uv_scroll(mode: u8, now: u32) -> [[f32; 4]; 2] {
    let t = if mode == 100 {
        (now % 8000) as f32 / 8000.0
    } else {
        (now % 4000) as f32 / 4000.0
    };

    let mut uv = [[1.0, 1.0, 0.0, 0.0]; 2];

    match mode {
        1 => {
            uv[1][2] = t;

            uv[1][3] = t;
        }
        2 => uv[1][2] = t,
        3 => uv[1][3] = t,
        4 => uv[1][2] = -t,
        5 => uv[1][3] = -t,
        6 => uv[0][3] = -t,
        7 => uv[0][2] = -3.0 * t,
        100 => uv[0][2] = -2.0 * t,
        101 => {
            uv[0][2] = t;

            uv[0][3] = -t;
        }
        _ => {}
    }

    uv
}

#[derive(Debug, Clone)]
pub struct Source {
    pub kind: u32,
    pub position: Vec3,
    pub yaw: f32,
    pub last_ms: u32,
}

#[derive(Debug, Default)]
pub struct Births {
    pub billboards: Vec<super::effects::Billboard>,
    pub particle: Option<super::burst::Recipe>,
}

impl Source {
    pub fn tick(
        &mut self,
        now: u32,
        visible: bool,
        detail: bool,
        environment: i32,
        zone: (u8, u8),
        rng: &mut super::rng::Lcg,
    ) -> Births {
        use super::effects::{Billboard, Motion};
        use render::billboard::BillboardPass;

        let variant = objects::house_type(self.kind);

        let elapsed = now.wrapping_sub(self.last_ms);

        let due = match variant {
            Some(2) => elapsed > 200,
            Some(3) if self.kind != 0x681 => {
                elapsed
                    >= if matches!(self.kind, 0x5F6 | 0x7D5) {
                        300
                    } else {
                        100
                    }
            }
            Some(4) => elapsed >= 201,
            _ => false,
        };

        if !visible || !due {
            return Births::default();
        }

        self.last_ms = now;

        let mut out = Births::default();

        if !detail {
            return out;
        }

        match variant {
            Some(2) => {
                let (start, end, radius, turn) = match self.kind {
                    0x112 => (0, 1, 1.0, 0.0),
                    0xC3 => (0, 4, 0.8, 0.0),
                    0x2B9 => (0, 1, -1.0, 0.0),
                    0x2BB => (0, 4, 1.0, 0.0),
                    0x5F0 | 0x5FF | 0x69F => (0, 0, 1.8, 0.0),
                    0x7C9 => (0, 4, 1.5, std::f32::consts::FRAC_PI_4),
                    _ => (1, 6, 1.8, 0.0),
                };

                let angles = [
                    -1.3463968,
                    0.0,
                    std::f32::consts::FRAC_PI_2,
                    std::f32::consts::PI,
                    3.0 * std::f32::consts::FRAC_PI_2,
                    5.105088,
                ];

                for angle in &angles[start..end] {
                    let r = rng.next_mod(5) as f32;

                    let a = self.yaw + turn + angle;

                    let mut b = Billboard::new(
                        151,
                        1500,
                        Vec3::new(r * 0.1 + 0.1, r * 0.15 + 0.12, r * 0.1 + 0.1),
                        0.0005,
                        1,
                        80,
                    );

                    b.position = self.position
                        + Vec3::new(
                            -a.cos() * radius - r * 0.05,
                            0.3,
                            a.sin() * radius - r * 0.05,
                        );

                    b.lift = true;

                    b.pass = BillboardPass::Blended;

                    b.set_color(0xFFFF_FFFF);

                    out.billboards.push(b);
                }
            }
            Some(3) => {
                let wide = matches!(self.kind, 0x5F6 | 0x7D5);

                let r = rng.next_mod(5) as f32;

                let (count, radius, size) = if self.kind == 0x7D5 {
                    (4, 2.5, 0.8)
                } else if wide {
                    (4, 4.0, 0.8)
                } else {
                    (2, 3.2, 0.4)
                };

                for i in 0..count {
                    let a = self.yaw + i as f32 * 0.3 - r * size + std::f32::consts::FRAC_PI_2;

                    let jitter = r * if self.kind == 0x5F6 { 0.15 } else { 0.05 };

                    let lift = if self.kind == 0x5F6 {
                        if zone == (9, 28) { 0.0 } else { -1.9 }
                    } else {
                        0.3
                    };

                    let mut b = Billboard::new(
                        151,
                        1500,
                        Vec3::splat(i as f32 * 0.3 + r * 0.15 + size),
                        0.0005,
                        1,
                        80,
                    );

                    b.position = self.position
                        + Vec3::new(-a.cos() * radius - jitter, lift, a.sin() * radius - jitter);

                    b.lift = true;

                    b.colour_mode = 3;

                    b.pass = BillboardPass::Blended;

                    b.set_color(0xFFFF_FFFF);

                    out.billboards.push(b);
                }
            }
            Some(4) => {
                for _ in 0..5 {
                    let r = rng.next_mod(10) as i32;

                    let q = (r - 5) as f32;

                    let mut b = Billboard::new(
                        if environment == 2 { 151 } else { 0 },
                        (r as u32 + 10) * 100,
                        Vec3::new(q * 0.1 + 0.3, q * 0.6 + 1.5, q * 0.1 + 0.3),
                        0.0001,
                        1,
                        80,
                    );

                    b.position = self.position + Vec3::new(q * 0.05, 1.0, q * 0.05);

                    b.origin = b.position;

                    b.motion = Motion::Rise;

                    b.rise = 1.5;

                    b.pass = if environment == 2 {
                        BillboardPass::Blended
                    } else {
                        BillboardPass::Additive
                    };

                    b.set_color(if environment == 2 {
                        0xFFFF_FFFF
                    } else {
                        0xFF00_AAFF
                    });

                    out.billboards.push(b);
                }

                out.particle = Some(super::burst::Recipe {
                    at: self.position + Vec3::Y,
                    kind: 0,
                    count: 3,
                    spread: 0.05,
                    argb: 0xFF00_5588,
                    texture: 56,
                    ..super::burst::Recipe::MEASURED_TAIL
                });
            }
            _ => {}
        }

        out
    }
}

pub fn roof_hidden(map: Option<&AttributeMap>, cell: (i32, i32)) -> bool {
    super::attack::attribute_at(map, cell).is_some_and(|byte| byte & flags::ROOFED != 0)
}
