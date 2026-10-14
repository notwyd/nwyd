use super::{
    actors::Actor,
    effects::{Billboard, Motion},
    rng::Lcg,
};
use glam::Vec3;
use render::billboard::BillboardPass;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

enum Batch {
    None,
    CloudSix,
    GreenSeven,
    GreySmall,
    GreyTiny,
    RedPair,
    GreenSix,
    Candle,
    AttackPair,
    LargePair,
    OrangeFour,
    FeetPair,
    RedPlume,
    PurpleOrbit,
    FivePlumes,
    BluePlumes,
    Costume,
    MountTail,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct LightLatch {
    pending: bool,
    active: bool,
    frame: Option<u32>,
}

impl LightLatch {
    pub(crate) fn raise(&mut self, a: &Actor, environment: i32, scene: u32) {
        if scene == 30000 && batch(a, environment) == Batch::BluePlumes {
            self.pending = true;
        }
    }

    pub(crate) fn begin(&mut self, scene: u32, now: u32) {
        if scene != 30000 {
            self.pending = false;

            self.active = false;

            self.frame = Some(now);

            return;
        }

        if self.frame == Some(now) {
            return;
        }

        self.frame = Some(now);

        self.active = std::mem::take(&mut self.pending);
    }

    pub(crate) fn apply_primary_light_suppression(
        &self,
        scene: u32,
        ordinary: [f32; 4],
    ) -> [f32; 4] {
        if scene == 30000 && self.active {
            [0.0; 4]
        } else {
            ordinary
        }
    }
}

fn batch(a: &Actor, environment: i32) -> Batch {
    if a.flags.get(0xB56) == 1 && super::body_sprites::has_primary(a) {
        return Batch::None;
    }

    let p = a.parts[0];

    match a.class {
        36 | 37 => Batch::None,
        32 => {
            if p.mesh == 1 && p.tex == 0 {
                Batch::CloudSix
            } else {
                Batch::None
            }
        }
        34 | 23 => Batch::None,
        21 if p.mesh == 10 => Batch::None,
        16 if p.mesh == 6 => Batch::GreenSeven,
        16 if p.mesh == 0 && p.tex == 1 => Batch::GreySmall,
        30 if matches!(p.mesh, 0..=2) => Batch::GreyTiny,
        30 if p.mesh == 4 => Batch::RedPair,
        38 if a.parts[2].mesh == 14 && a.kingdom == 0 => Batch::RedPair,
        25 if p.mesh == 12 || p.mesh == 3 && p.tex == 8 => Batch::None,
        28 if p.mesh == 2 => Batch::GreenSix,
        22 | 27 => Batch::Candle,
        18 if a.action == 5 => Batch::AttackPair,
        25 if p.mesh == 3 && a.scale > 1.1751 => Batch::LargePair,
        30 => Batch::LargePair,
        21 if p.mesh == 4 => Batch::OrangeFour,
        29 if p.mesh == 1 => Batch::None,
        38 if (a.kingdom as i8) > 0 => Batch::None,
        33 if p.mesh == 1 && environment == 2 => Batch::None,
        16 if p.mesh == 7 => Batch::FeetPair,
        39 => Batch::None,
        56 if p.mesh == 0 => Batch::RedPlume,
        74 if p.mesh == 0 => Batch::PurpleOrbit,
        66 if a.flags.get(0xB3E) == 0 => Batch::FivePlumes,
        67 => Batch::BluePlumes,
        _ if matches!(a.costume_effect_item, 4161 | 4162) => Batch::Costume,
        _ => Batch::MountTail,
    }
}

fn sprite(at: Vec3, slot: usize, life: u32, size: Vec3, growth: f32, colour: u32) -> Billboard {
    let mut b = Billboard::new(slot, life, size, growth, 1, 80);

    b.position = at;

    b.origin = at;

    b.set_color(colour);

    b.pass = BillboardPass::Additive;

    b
}

fn jitter(at: Vec3, x: u32, x0: i32, z: u32, z0: i32, unit: f32, rng: &mut Lcg) -> Vec3 {
    at + Vec3::new(
        ((rng.next() % x) as i32 - x0) as f32 * unit,
        0.0,
        ((rng.next() % z) as i32 - z0) as f32 * unit,
    )
}

pub(crate) fn emit(a: &mut Actor, now: u32, environment: i32, rng: &mut Lcg) -> Vec<Billboard> {
    if a.class == 45 {
        return Vec::new();
    }

    let recipe = batch(a, environment);

    emit_batch(a, now, recipe, rng)
}

fn emit_batch(a: &mut Actor, now: u32, recipe: Batch, rng: &mut Lcg) -> Vec<Billboard> {
    if matches!(recipe, Batch::None | Batch::MountTail) || !a.scale.is_finite() {
        return Vec::new();
    }

    let period = match recipe {
        Batch::CloudSix => Some(500),
        Batch::GreenSeven | Batch::GreySmall | Batch::GreyTiny => Some(300),
        Batch::RedPair | Batch::LargePair | Batch::OrangeFour | Batch::FeetPair => Some(100),
        Batch::GreenSix => Some(1000),
        Batch::Costume => Some(800),
        _ => None,
    };

    if let Some(period) = period {
        if now.wrapping_sub(a.body_points.last_ms) <= period {
            return Vec::new();
        }

        a.body_points.last_ms = now;
    }

    let points = a.body_points.points;

    let scale = a.scale;

    let mut out = Vec::new();

    match recipe {
        Batch::CloudSix => {
            for at in points[..6].iter().flatten() {
                let mut b = sprite(
                    *at,
                    119,
                    3000,
                    Vec3::new(scale + 0.1, scale + 0.3, scale + 0.1),
                    0.0001,
                    0xFFFFFFFF,
                );

                b.colour_mode = 3;

                b.pass = BillboardPass::Blended;

                out.push(b);
            }
        }
        Batch::GreenSeven | Batch::GreySmall | Batch::GreyTiny => {
            let (start, count, size, colour) = match recipe {
                Batch::GreenSeven => (1, 7, scale * 1.5, 0xFF00AA66),
                Batch::GreySmall => (0, 1, scale * 0.1, 0xFFAAAAAA),
                _ => (0, 1, scale * 0.005, 0xFFAAAAAA),
            };

            for point in &points[start..start + count] {
                let at = jitter(point.unwrap_or(Vec3::ZERO), 10, 5, 10, 5, 0.05, rng);

                if point.is_some() {
                    out.push(sprite(at, 0, 2500, Vec3::splat(size), 0.001, colour));
                }
            }
        }
        Batch::RedPair
        | Batch::AttackPair
        | Batch::LargePair
        | Batch::OrangeFour
        | Batch::FeetPair => {
            let start = match recipe {
                Batch::AttackPair => 0,
                Batch::RedPair if a.class == 38 => 2,
                _ => 1,
            };

            let colour = match recipe {
                Batch::LargePair if a.class == 30 => 0xFF66FF66,
                Batch::OrangeFour => 0xFFFFAA66,
                Batch::FeetPair => 0xFFFF8800,
                _ => 0xFFFF6666,
            };

            let count = if recipe == Batch::OrangeFour { 4 } else { 2 };

            for i in 0..count {
                let point = if recipe == Batch::FeetPair {
                    Some(a.position + Vec3::Y * 0.2)
                } else {
                    points[start + i % 2]
                };

                let at = jitter(point.unwrap_or(Vec3::ZERO), 10, 5, 10, 5, 0.05, rng);

                let life = 1500 + 400 * if count == 4 { i as u32 / 2 } else { i as u32 };

                if point.is_some() {
                    out.push(sprite(at, 0, life, Vec3::splat(0.1), 0.001, colour));
                }
            }
        }
        Batch::GreenSix => {
            for i in 0..6 {
                let r = (rng.next() % 5) as f32;

                let point = if i % 2 == 0 {
                    Some(a.position + Vec3::Y)
                } else {
                    points[0]
                };

                if let Some(at) = point {
                    let mut b = sprite(
                        at,
                        0,
                        1500 + i * 400,
                        Vec3::splat(scale * 0.2 + r * 0.1),
                        0.001,
                        0xFF00FF00,
                    );

                    b.pass = BillboardPass::Blended;

                    b.rise = -1.0;

                    out.push(b);
                }
            }
        }
        Batch::Candle | Batch::RedPlume => {
            let r = (rng.next() % 5) as f32;

            let (size, colour, rise, x) = if recipe == Batch::Candle {
                (
                    Vec3::new(r * 0.01 + 0.01, r * 0.03 + 0.01, r * 0.01 + 0.01),
                    if a.class == 27 {
                        0xFFFFDD88
                    } else {
                        0xFFFFFFCC
                    },
                    -1.0,
                    10,
                )
            } else {
                (
                    Vec3::new(r * 0.4 + 0.2, r * 0.7 + 0.9, r * 0.5 + 0.5),
                    0xFFFF7777,
                    -3.0,
                    40,
                )
            };

            let at = jitter(points[0].unwrap_or(Vec3::ZERO), x, 5, 10, 5, 0.02, rng);

            if points[0].is_some() {
                let mut b = sprite(at, 0, 1500, size, 0.0001, colour);

                b.motion = Motion::Rise;

                b.rise = rise;

                out.push(b);
            }
        }
        Batch::PurpleOrbit => {
            let r = rng.next();

            if let Some(point) = points[1] {
                let mut b = sprite(
                    point + Vec3::Y * ((r % 5) as i32 - 3) as f32 * 1.5,
                    56,
                    3500,
                    Vec3::splat(0.3),
                    0.000001,
                    0xFFAA00FF,
                );

                b.motion = Motion::Circle;

                (b.amp_h, b.rise) = if r % 3 == 0 { (2.5, 1.5) } else { (1.5, 3.5) };

                out.push(b);
            }
        }
        Batch::FivePlumes => {
            for i in 0..5 {
                let r = (rng.next() % 20) as f32;

                let (size, life, rise, colour, point) = match i {
                    0 => (
                        Vec3::new(0.4, r * 0.1 + 0.004, r * 0.01),
                        500,
                        0.9,
                        0xFFAAAACC,
                        points[8].map(|p| p + Vec3::Y * 0.6),
                    ),
                    1 => (
                        Vec3::new(0.3, r * 0.1 + 0.003, r * 0.01 + 0.09),
                        500,
                        0.5,
                        0xFF9999AA,
                        points[8].map(|p| p + Vec3::Y * 0.6),
                    ),
                    2 => (
                        Vec3::new(1.5, 1.5, 0.45),
                        500,
                        0.5,
                        0xFF1111AA,
                        points[8].map(|p| p + Vec3::Y * 0.6),
                    ),
                    _ => (
                        Vec3::new(r * 0.01 + 0.1, r * 0.1 + 0.001, r * 0.01 + 0.03),
                        700,
                        -0.5,
                        0xFFFFAAAA,
                        points[i + 6],
                    ),
                };

                let at = jitter(point.unwrap_or(Vec3::ZERO), 6, 3, 40, 20, 0.02, rng);

                if point.is_some() {
                    let mut b = sprite(at, 0, life, size, 0.0001, colour);

                    b.motion = Motion::Rise;

                    b.rise = rise;

                    out.push(b);
                }
            }
        }
        Batch::BluePlumes => {
            if now.wrapping_sub(a.body_points.last_ms) > 100 {
                a.body_points.last_ms = now;

                for at in points[8..10].iter().flatten() {
                    let mut b = sprite(*at, 56, 500, Vec3::splat(0.1), 0.0, 0xFF0088FF);

                    b.colour_mode = 0;

                    out.push(b);
                }
            }

            let r = (rng.next() % 20) as f32;

            let at = jitter(points[10].unwrap_or(Vec3::ZERO), 6, 3, 40, 20, 0.02, rng);

            if points[10].is_some() {
                let mut b = sprite(
                    at,
                    0,
                    1500,
                    Vec3::new(r * 0.01 + 0.02, r * 0.1 + 0.02, r * 0.01 + 0.06),
                    0.0001,
                    0xFFAAAAFF,
                );

                b.motion = Motion::Rise;

                b.rise = -0.5;

                out.push(b);
            }
        }
        Batch::Costume => {
            if let Some(at) = points[10] {
                let mut b = sprite(at, 56, 200, Vec3::splat(0.1), 0.0, 0xFFFF5500);

                b.colour_mode = 0;

                out.push(b);
            }
        }
        Batch::None | Batch::MountTail => {}
    }

    out
}

pub(crate) fn skin_emit(
    a: &mut Actor,
    model: u32,
    mesh: i16,
    hot: bool,
    now: u32,
    rng: &mut Lcg,
) -> Vec<Billboard> {
    match (model, mesh) {
        (20, 6) => emit_batch(a, now, Batch::GreenSeven, rng),
        (28, 3) => emit_batch(a, now, Batch::Candle, rng),
        (31, _) if hot => {
            let mut out = Vec::new();

            for point in &a.body_points.points[..4] {
                let r = rng.next() % 5;

                let at = jitter(point.unwrap_or(Vec3::ZERO), 5, 2, 5, 2, 0.05, rng);

                if point.is_some() {
                    let mut b = sprite(
                        at,
                        0,
                        500 + r,
                        Vec3::splat(0.01),
                        0.001,
                        if r > 3 { 0xFFFF1111 } else { 0xFFFFFFFF },
                    );

                    b.frames = 0;

                    b.motion = Motion::Rise;

                    b.rise = 0.05;

                    b.lift = true;

                    out.push(b);
                }
            }

            out
        }
        _ => Vec::new(),
    }
}

pub(crate) fn arm_mount(a: &mut Actor, environment: i32) {
    if a.class == 45 {
        return;
    }

    if batch(a, environment) != Batch::MountTail {
        return;
    }

    if a.mount_model == Some(31)
        && let Some(mount) = &mut a.mount
        && mount.parts[0].mesh == 8
    {
        mount.hot_effect = true;
    }
}
