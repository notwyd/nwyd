use super::effect_tree::{
    AuthoredMesh, Base, Class, Cull, Effect, MeshMaterial, MeshSubmission, Pass, Sink, Submit,
    TexturedMesh, Tick,
};
use super::effects::Billboard;
use super::rng::Lcg;
use super::shade::Ground;
use glam::Vec3;
use render::camera::Matrix4;

pub const CLASS: Class = Class {
    name: "SkillFreezeBlade",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

pub const MESH_SPIKE: i32 = 0x2C2;

pub const MESH_SHARD: i32 = 0x2C3;

pub const MESH_TRAIL: i32 = 0xB02;

pub const MESH_CHAIN_BASE: i32 = 0x2C6;

pub const DURATION_MS: i32 = 2000;

pub const DURATION_CHAIN_MS: i32 = 0x5DC;

pub const STAGGER_MS: u32 = 500;

pub const DRAW_T: f32 = 0.05;

pub const EXPIRE_T: f32 = 1.0;

pub const FADE_T: f32 = 0.6;

pub const CHAIN_T: f32 = 0.2;

pub const EMIT_MS: u32 = 100;

pub const BASE_ARGB: u32 = 0x5511_3366;

pub const FADE_CYCLES: f32 = 1.25;

pub const TINTED_TEXTURE: i32 = 0x13;

pub const SPARK_KIND: usize = 0;

pub const SPARK_LIFETIME_MS: u32 = 0x5DC;

pub const SPARK_GROWTH: f32 = 0.001;

pub const SPARK_FRAMES: u32 = 1;

pub const SPARK_FRAME_MS: u32 = 0x50;

pub const SPARK_SIZE_STEP: f32 = 0.3;

pub const SPARK_SIZE_BASE: f32 = 0.5;

pub const SPARK_JITTER_STEP: f32 = 0.2;

pub const SPARK_ARGB_ICE: u32 = 0xFFAA_EEFF;

pub const SPARK_ARGB_STONE: u32 = 0xFFAA_AAAA;

pub const SOUND: u16 = 0xA0;

pub fn mesh_id(kind: i32, trail: bool) -> i32 {
    if trail {
        return MESH_TRAIL;
    }

    match kind {
        1 => MESH_SHARD,
        2..=8 => kind + MESH_CHAIN_BASE,
        _ => MESH_SPIKE,
    }
}

pub fn duration_ms(kind: i32, trail: bool) -> i32 {
    match !trail && (2..=8).contains(&kind) {
        true => DURATION_CHAIN_MS,
        false => DURATION_MS,
    }
}

pub fn is_ice(kind: i32) -> bool {
    !(2..=8).contains(&kind)
}

pub fn tint(t: f32) -> u32 {
    if t < FADE_T {
        return BASE_ARGB;
    }

    let f = ((t - FADE_T) * FADE_CYCLES * std::f32::consts::PI).cos();

    let chan = |m: f32| ((f * m).trunc() as i64).clamp(0, 255) as u32;

    (chan(85.0) << 24) | (chan(17.0) << 16) | (chan(51.0) << 8) | chan(102.0)
}

pub fn scale(kind: i32, t: f32) -> Option<f32> {
    match is_ice(kind) {
        true => (t < 0.1).then_some(t * 15.0 + 0.2),
        false => match t {
            t if t < 0.1 => Some(t * 11.0 + 0.1),
            t if t > 0.9 => Some(1.2 - (t - 0.9) * 11.0),
            _ => None,
        },
    }
}

#[derive(Debug)]

pub struct FreezeBlade {
    pub position: Vec3,
    pub kind: i32,
    pub trail: bool,
    pub spin: bool,
    pub birth_ms: u32,
    pub duration_ms: i32,
    pub mesh_id: i32,
    pub t: f32,
    pub step: (f32, f32),
    pub chained: bool,
    pub mesh_scale: Option<f32>,
    pub mesh_spin: f32,
    spawned: Vec<Box<dyn Effect>>,
    rng: Lcg,
    clock: std::rc::Rc<std::cell::Cell<u32>>,
    gone: bool,
}

impl FreezeBlade {
    pub fn new(at: Vec3, kind: i32, trail: bool, spin: bool, rng: &mut Lcg, now_ms: u32) -> Self {
        let mut blade = FreezeBlade {
            position: at,
            kind,
            trail,
            spin,
            birth_ms: match trail {
                true => now_ms.wrapping_add(rng.next() % STAGGER_MS),
                false => now_ms,
            },
            duration_ms: duration_ms(kind, trail),
            mesh_id: mesh_id(kind, trail),
            t: 0.0,
            step: (0.0, 0.0),
            chained: trail,
            mesh_scale: None,
            mesh_spin: 0.0,
            spawned: Vec::new(),
            rng: *rng,
            clock: Default::default(),
            gone: false,
        };

        if !trail && is_ice(kind) {
            blade.spawned.push(Box::new(super::timers::TimedQuad::new(
                super::quads::GroundQuad {
                    owner: 0,
                    slot: super::quads::Slot::Placed,
                    texture: 2,
                    pass: render::billboard::BillboardPass::Additive,
                    anchor: at + Vec3::Y * 0.3,
                    base_scale: Vec3::splat(2.0),
                    grow: 0.002,
                    spin_period: 0,
                    ordinal: 0,
                    argb: 0xFF2255AA,
                },
                2000,
            )));
        }

        blade
    }

    pub fn born(&self, now_ms: u32) -> bool {
        now_ms.wrapping_sub(self.birth_ms) < 0x8000_0000
    }

    pub fn tick_with(
        &mut self,
        now_ms: u32,
        rng: &mut Lcg,
        stamp: &mut u32,
        ground: Option<&Ground<'_>>,
    ) {
        if !self.born(now_ms) {
            return;
        }

        let elapsed = now_ms.wrapping_sub(self.birth_ms) as f32;

        self.t = elapsed / self.duration_ms.max(1) as f32;

        if self.t >= EXPIRE_T {
            self.gone = true;

            return;
        }

        if let Some(g) = ground {
            let (x, z) = (self.position.x, self.position.z);

            if let Some(y) = g.surface_height(x, z) {
                self.position.y = y;
            }
        }

        if let Some(s) = scale(self.kind, self.t) {
            self.mesh_scale = Some(s);
        }

        if self.spin {
            self.mesh_scale = Some(self.t * 3.0);

            self.mesh_spin = self.t + self.t;
        }

        if now_ms.wrapping_sub(*stamp) > EMIT_MS {
            self.emit(rng);

            *stamp = now_ms;
        }

        self.chain(rng, now_ms);
    }

    pub fn mesh_draw(&self) -> Option<MeshSubmission> {
        if self.gone || self.t < 0.05 {
            return None;
        }

        let r = std::f32::consts::FRAC_PI_2 + if self.spin { self.t * 5.0 } else { 0.0 };

        let xz = if self.spin { self.mesh_spin } else { 1.0 };

        let rotation = Matrix4::rotation_yaw_pitch_roll(r, -std::f32::consts::FRAC_PI_2, r);

        let scaled = Matrix4::multiply(
            &rotation,
            &Matrix4::scaling(xz, self.mesh_scale.unwrap_or(1.0), xz),
        );

        let world = Matrix4::multiply(
            &scaled,
            &Matrix4::translation(self.position.x, self.position.y, self.position.z),
        );

        if self.kind < 2 && !self.trail {
            let argb = tint(self.t);

            let channel = |shift: u32| ((argb >> shift) & 255u32) as f32 / 255.0;

            Some(MeshSubmission::Textured(TexturedMesh {
                mesh_id: self.mesh_id as u32,
                world,
                blend: 1,
                uv_offset: [0.0; 2],
                texture: 19,
                tint: [channel(16), channel(8), channel(0), channel(24)],
            }))
        } else {
            Some(MeshSubmission::Authored(AuthoredMesh {
                mesh_id: self.mesh_id as u32,
                world,
                material: if self.kind < 2 {
                    MeshMaterial::Additive
                } else {
                    MeshMaterial::FreezeStone
                },
                texture: (!self.trail).then_some("mesh\\stone01.wys"),
            }))
        }
    }

    fn emit(&mut self, rng: &mut Lcg) {
        let size = (rng.next() % 5) as f32 * SPARK_SIZE_STEP + SPARK_SIZE_BASE;

        let mut spark = Billboard::new(
            SPARK_KIND,
            SPARK_LIFETIME_MS,
            Vec3::splat(size),
            SPARK_GROWTH,
            SPARK_FRAMES,
            SPARK_FRAME_MS,
        );

        let jitter = |r: &mut Lcg| ((r.next() % 10) as i32 - 5) as f32 * SPARK_JITTER_STEP;

        let dx = jitter(rng);

        let dz = jitter(rng);

        spark.position = Vec3::new(self.position.x + dx, self.position.y, self.position.z + dz);

        spark.pass = if is_ice(self.kind) {
            render::billboard::BillboardPass::Additive
        } else {
            render::billboard::BillboardPass::Blended
        };

        spark.set_color(match is_ice(self.kind) {
            true => SPARK_ARGB_ICE,
            false => SPARK_ARGB_STONE,
        });

        spark.lift = true;

        self.spawned.push(Box::new(spark));
    }

    fn chain(&mut self, rng: &mut Lcg, now_ms: u32) {
        if self.chained || !(1 < self.kind && self.kind < 8) || self.t <= CHAIN_T {
            return;
        }

        let at = Vec3::new(
            self.position.x + self.step.0,
            self.position.y,
            self.position.z + self.step.1,
        );

        let mut next = FreezeBlade::new(at, self.kind + 1, self.trail, false, rng, now_ms);

        next.step = self.step;

        self.spawned.push(Box::new(next));

        self.chained = true;
    }
}

impl Effect for FreezeBlade {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let mut rng = self.rng;

        let mut stamp = self.clock.get();

        self.tick_with(tick.now_ms, &mut rng, &mut stamp, None);

        self.rng = rng;

        self.clock.set(stamp);
    }

    fn needs_ground(&self) -> bool {
        true
    }

    fn update_grounded(&mut self, tick: &Tick, ground: &Ground<'_>) {
        let mut rng = self.rng;

        let mut stamp = self.clock.get();

        self.tick_with(tick.now_ms, &mut rng, &mut stamp, Some(ground));

        self.rng = rng;

        self.clock.set(stamp);
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        let mut stamp = self.clock.get();

        self.tick_with(tick.now_ms, live.rng, &mut stamp, live.ground);

        self.clock.set(stamp);
    }

    fn bind_freeze_clock(&mut self, clock: std::rc::Rc<std::cell::Cell<u32>>) {
        self.clock = clock;
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, _cx: &Submit<'_>, out: &mut Sink<'_>) {
        out.mesh.extend(self.mesh_draw());
    }

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        std::mem::take(&mut self.spawned)
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}
