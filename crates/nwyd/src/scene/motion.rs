use super::effects::Motion;
use glam::Vec3;

pub const PI: f64 = std::f32::consts::PI as f64;

pub const VERTICAL_HARMONIC: f64 = 6.0;

pub const BURST_PHASE: f32 = 0.2;

pub const BURST_RATE: f64 = 10.0;

pub const BURST_DRIFT: f64 = 0.3;

pub const TRAVEL_RATE: f64 = 5.0;

pub const SPIN_RATE: f64 = 0.1;

pub const ORBIT_PERIOD_MS: u32 = 2_000;

pub const FOLLOW_LIFT: f32 = 1.0;

pub const OWNER_LIFT: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Owner {
    pub position: Vec3,
    pub seat: Option<Vec3>,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Fields {
    pub age: f32,
    pub origin: Vec3,
    pub amp_h: f32,
    pub amp_v: f32,
    pub phase: f32,
    pub dir: Vec3,
    pub owner: Option<Owner>,
    pub now_ms: u32,
    pub elapsed_ms: u32,
}

impl Fields {
    pub fn new(age: f32) -> Fields {
        Fields {
            age,
            origin: Vec3::ZERO,
            amp_h: 1.0,
            amp_v: 2.0,
            phase: 6.0,
            dir: Vec3::ZERO,
            owner: None,
            now_ms: 0,
            elapsed_ms: 0,
        }
    }
}

fn sin(theta: f32) -> f32 {
    (theta as f64).sin() as f32
}

fn cos(theta: f32) -> f32 {
    (theta as f64).cos() as f32
}

pub fn theta(f: &Fields) -> f32 {
    ((PI * f.age as f64) * f.phase as f64) as f32
}

pub fn theta_fast(f: &Fields) -> f32 {
    ((PI * (f.age as f64 * VERTICAL_HARMONIC)) * f.phase as f64) as f32
}

pub fn apply_rise(f: &Fields, pos: &mut Vec3) {
    pos.y = f.amp_v * f.age + f.origin.y;
}

pub fn apply_sway_x(f: &Fields, pos: &mut Vec3) {
    pos.y = f.amp_v * f.age + f.origin.y;

    pos.x = sin(theta(f)) * f.amp_h + f.origin.x;
}

pub fn apply_circle(f: &Fields, pos: &mut Vec3) {
    pos.y = f.amp_v * f.age + f.origin.y;

    let theta = theta(f);

    pos.x = sin(theta) * f.amp_h + f.origin.x;

    pos.z = cos(theta) * f.amp_h + f.origin.z;
}

pub fn apply_sway_x_creep_z(f: &Fields, pos: &mut Vec3) {
    pos.y = f.amp_v * f.age + f.origin.y;

    apply_x_sway_z_creep(f, sin(theta(f)), pos);
}

fn apply_x_sway_z_creep(f: &Fields, lateral: f32, pos: &mut Vec3) {
    pos.x = lateral * f.amp_h + f.origin.x;

    pos.z = f.age * f.amp_h + f.origin.z;
}

pub fn apply_creep_x_sway_z(f: &Fields, pos: &mut Vec3) {
    pos.y = f.amp_v * f.age + f.origin.y;

    pos.x = f.age * f.amp_h + f.origin.x;

    pos.z = sin(theta(f)) * f.amp_h + f.origin.z;
}

pub fn apply_mote_x(f: &Fields, pos: &mut Vec3) {
    pos.y = sin(theta_fast(f)) * f.amp_v + f.origin.y;

    pos.x = f.age * f.amp_h + f.origin.x;

    pos.z = sin(theta(f)) * f.amp_h + f.origin.z;
}

pub fn apply_mote_z(f: &Fields, pos: &mut Vec3) {
    pos.y = cos(theta_fast(f)) * f.amp_v + f.origin.y;

    apply_x_sway_z_creep(f, cos(theta(f)), pos);
}

pub fn apply_mote_spiral(f: &Fields, pos: &mut Vec3) {
    pos.y = cos(theta_fast(f)) * f.amp_v + f.origin.y;

    let theta = theta(f);

    pos.x = cos(theta) * f.age * f.amp_h + f.origin.x;

    pos.z = sin(theta) * f.age * f.amp_h + f.origin.z;
}

pub fn apply_follow(f: &Fields, pos: &mut Vec3) {
    let Some(owner) = f.owner else {
        return;
    };

    let (t, u) = (f.age, 1.0 - f.age);

    match owner.seat {
        Some(seat) => {
            pos.y = (FOLLOW_LIFT + seat.y) * t + f.origin.y * u;

            pos.x = seat.x * t + f.origin.x * u;

            pos.z = u * f.origin.z + t * seat.y;
        }
        None => {
            pos.y = (FOLLOW_LIFT + owner.position.y) * t + f.origin.y * u;

            pos.x = owner.position.x * t + f.origin.x * u;

            pos.z = u * f.origin.z + owner.position.z * t;
        }
    }
}

pub fn apply_global_halo(f: &Fields, pos: &mut Vec3) {
    accumulate_orbit_offset(
        f,
        ((f.now_ms % ORBIT_PERIOD_MS) as f64 / ORBIT_PERIOD_MS as f64) as f32,
        pos,
    );
}

fn accumulate_orbit_offset(f: &Fields, u: f32, pos: &mut Vec3) {
    let theta = ((PI * u as f64) + (PI * u as f64)) as f32;

    let (c, s) = (cos(theta), sin(theta));

    pos.x += s * f.amp_h;

    pos.y += f.amp_v * c;

    pos.z += c * f.amp_h;
}

pub fn apply_creep(f: &Fields, pos: &mut Vec3) {
    pos.x = f.amp_h * f.age + f.origin.x;

    apply_rise(f, pos);
}

pub fn apply_burst(f: &Fields, pos: &mut Vec3) {
    if f.age < BURST_PHASE {
        scale_and_offset(f, f.age, BURST_RATE, pos);

        return;
    }

    let frozen = |d: f32| ((d as f64 * BURST_PHASE as f64) as f32 as f64 * BURST_RATE) as f32;

    let drift = |d: f32| ((d as f64 * f.age as f64) as f32 as f64 * BURST_DRIFT) as f32;

    pos.x = (f.origin.x + frozen(f.dir.x)) + drift(f.dir.x);

    pos.y = (f.origin.y + frozen(f.dir.y)) + drift(f.dir.y);

    pos.z = (f.origin.z + frozen(f.dir.z)) + drift(f.dir.z);
}

fn scale_and_offset(f: &Fields, age: f32, k: f64, pos: &mut Vec3) {
    let term = |d: f32| ((d as f64 * age as f64) as f32 as f64 * k) as f32;

    pos.x = f.origin.x + term(f.dir.x);

    pos.y = f.origin.y + term(f.dir.y);

    pos.z = f.origin.z + term(f.dir.z);
}

pub fn forward_spin_angle(f: &Fields) -> f32 {
    (PI * (f.age as f64 * SPIN_RATE)) as f32
}

pub fn reverse_spin_angle(f: &Fields) -> f32 {
    (PI * (-(f.age as f64) * SPIN_RATE)) as f32
}

pub fn apply_travel(f: &Fields, pos: &mut Vec3) {
    scale_and_offset(f, f.age, TRAVEL_RATE, pos);
}

pub fn apply_diagonal_rise(f: &Fields, pos: &mut Vec3) {
    let travel = f.amp_h * f.age;

    pos.x = f.origin.x + travel;

    pos.z = travel + f.origin.z;

    apply_rise(f, pos);
}

pub fn apply_local_halo(f: &Fields, pos: &mut Vec3) {
    let remainder = (f.elapsed_ms as i32) % ORBIT_PERIOD_MS as i32;

    accumulate_orbit_offset(f, (remainder as f64 / ORBIT_PERIOD_MS as f64) as f32, pos);
}

pub fn apply(motion: Motion, f: &Fields, pos: &mut Vec3, spin: &mut f32) {
    match motion {
        Motion::Still => {}
        Motion::Rise => apply_rise(f, pos),
        Motion::SwayX => apply_sway_x(f, pos),
        Motion::Circle => apply_circle(f, pos),
        Motion::SwayXCreepZ => apply_sway_x_creep_z(f, pos),
        Motion::CreepXSwayZ => apply_creep_x_sway_z(f, pos),
        Motion::MoteX => apply_mote_x(f, pos),
        Motion::MoteZ => apply_mote_z(f, pos),
        Motion::MoteSpiral => apply_mote_spiral(f, pos),
        Motion::Follow => apply_follow(f, pos),
        Motion::Halo => apply_global_halo(f, pos),
        Motion::Creep => apply_creep(f, pos),
        Motion::DiagonalRise => apply_diagonal_rise(f, pos),
        Motion::Burst => apply_burst(f, pos),
        Motion::Spin => *spin = forward_spin_angle(f),
        Motion::SpinBack => *spin = reverse_spin_angle(f),
        Motion::LocalHalo => apply_local_halo(f, pos),
        Motion::Travel => apply_travel(f, pos),
        Motion::Unhandled(_) => {}
    }
}

pub fn owner_follow(motion: Motion, f: &Fields, scale_y: f32, pos: &mut Vec3) {
    let Some(owner) = f.owner else {
        return;
    };

    if motion == Motion::Follow {
        return;
    }

    match owner.seat {
        Some(seat) => {
            *pos = seat;

            pos.y += OWNER_LIFT * scale_y;
        }
        None => {
            pos.x = owner.position.x;

            pos.z = owner.position.z;

            pos.y = OWNER_LIFT * scale_y + owner.position.y;
        }
    }
}
