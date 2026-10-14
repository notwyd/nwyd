use super::effect_context::ActorKey;
use super::effect_tree::{
    Base, Class, Cull, Effect, MeshSubmission, Pass, Sink, Submit, TexturedMesh, Tick,
};
use super::effects::Billboard;
use super::rng::Lcg;
use super::shade::{Ground, Shade};
use glam::Vec3;
use render::camera::Matrix4;

pub const CLASS: Class = Class {
    name: "SkillDoubleSwing",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

pub const MESH_ID: i32 = 0x2BE;

pub const LIFETIME_PER_UNIT_MS: i32 = 300;

pub const LIFETIME_FLOOR_MS: i32 = 1;

pub const LIFETIME_CEILING_MS: i32 = 5000;

pub const EMIT_MS: u32 = 0x65;

pub const HALFWAY_T: f32 = 0.5;

pub const EXPIRE_T: f32 = 1.0;

pub const OVERSHOOT: f32 = 3.0;

pub const OVERSHOOT_DIVE: f32 = 2.0;

pub const SPARK_KIND: usize = 0;

pub const SPARK_LIFETIME_MS: u32 = 1000;

pub const SPARK_GROWTH: f32 = 0.001;

pub const SPARK_FRAMES: u32 = 1;

pub const SPARK_FRAME_MS: u32 = 0x50;

pub const SPARK_SIZE_STEP: f32 = 0.2;

pub const SPARK_SIZE_BASE: f32 = 0.3;

pub const SPARK_DROP: f32 = 0.5;

pub const SPARK_COLOURS: [(i32, u32); 2] = [(0, 0xFFAA_FFEE), (1, 0xFFFF_9999)];

pub mod mode {

    pub const SWING: i32 = 0;

    pub const SECOND: i32 = 1;

    pub const SPIN: i32 = 2;

    pub const BOUND: i32 = 3;

    pub const DIVE: i32 = 4;
}

#[derive(Debug)]

pub struct DoubleSwing {
    pub source: Vec3,
    pub dest: Vec3,
    pub mode: i32,
    pub target: Option<u16>,
    pub target_key: Option<ActorKey>,
    pub shade: Shade,
    pub children: Vec<Billboard>,
    pub position: Vec3,
    pub birth_ms: u32,
    pub lifetime_ms: i32,
    pub t: f32,
    pub yaw: f32,
    pub spin: f32,
    pub last_emit_ms: u32,
    spawned: Vec<Box<dyn Effect>>,
    gone: bool,
}

impl DoubleSwing {
    pub fn new(source: Vec3, dest: Vec3, mode: i32, target: Option<u16>, now_ms: u32) -> Self {
        let span = (dest - source).length().trunc() as i32;

        let lifetime_ms = match span.saturating_mul(LIFETIME_PER_UNIT_MS) {
            0 => LIFETIME_FLOOR_MS,
            ms => ms.min(LIFETIME_CEILING_MS),
        };

        let (grid, colour) = match mode {
            1 => (5, 0x0077_0000),
            2 => (3, 0x0000_3355),
            3 => (3, 0x0055_3355),
            4 => (0, 0x0077_0000),
            _ => (3, 0x0000_5533),
        };

        let mut children = Vec::new();

        if matches!(mode, 2 | 3) {
            for (kind, size, colour) in [
                (
                    56,
                    Vec3::ONE,
                    if mode == 2 { 0xFF55_EEFF } else { 0xFFFF_55EE },
                ),
                (2, Vec3::splat(0.5), 0xFFFF_FFFF),
                (
                    60,
                    Vec3::new(2.5, 2.0, 2.5),
                    if mode == 2 { 0x3300_3344 } else { 0x3344_0033 },
                ),
            ] {
                let mut child = Billboard::new(kind, 0, size, 0.0, 1, 80);

                child.set_color(colour);

                child.pass = render::billboard::BillboardPass::Additive;

                child.colour_mode = 0;

                child.position = dest - Vec3::Y * 0.5;

                children.push(child);
            }
        }

        DoubleSwing {
            source,
            dest,
            mode,
            target: match mode == mode::BOUND {
                true => target,
                false => None,
            },
            target_key: None,
            shade: Shade::new(grid, 7, colour, source, 0),
            children,
            position: source,
            birth_ms: now_ms,
            lifetime_ms,
            t: 0.0,
            yaw: 0.0,
            spin: 0.0,
            last_emit_ms: 0,
            spawned: Vec::new(),
            gone: false,
        }
    }

    pub fn overshoot(&self) -> f32 {
        match self.mode == mode::DIVE {
            true => OVERSHOOT_DIVE,
            false => OVERSHOOT,
        }
    }

    pub fn spark_colour(&self) -> Option<u32> {
        SPARK_COLOURS
            .iter()
            .find(|(m, _)| *m == self.mode)
            .map(|(_, argb)| *argb)
    }

    pub fn tick_with(&mut self, now_ms: u32, rng: &mut Lcg, at: impl Fn(u16) -> Option<Vec3>) {
        if self.gone {
            return;
        }

        let elapsed = now_ms.wrapping_sub(self.birth_ms) as f32;

        self.t = elapsed / self.lifetime_ms.max(LIFETIME_FLOOR_MS) as f32;

        if self.t > HALFWAY_T {
            self.target = None;

            self.target_key = None;
        }

        if let Some(live) = self.target.and_then(&at) {
            self.dest = Vec3::new(live.x, live.y + 1.0, live.z);
        }

        let delta = self.dest - self.source;

        self.yaw = delta.x.atan2(delta.z) - std::f32::consts::FRAC_PI_2;

        if self.mode == mode::SPIN {
            self.spin = (now_ms % 1000) as f32 * std::f32::consts::TAU / 1000.0;
        }

        if (self.mode == mode::DIVE && self.t > HALFWAY_T) || self.t >= EXPIRE_T {
            self.gone = true;

            if self.t >= EXPIRE_T {
                return;
            }
        }

        let far = self.dest + delta * self.overshoot();

        self.position = self.source * (1.0 - self.t) + far * self.t;

        self.shade.centre = Vec3::new(self.position.x, 0.0, self.position.z);

        for child in &mut self.children {
            child.position = self.position;
        }

        if self.mode == mode::BOUND || now_ms.wrapping_sub(self.last_emit_ms) < EMIT_MS {
            return;
        }

        self.emit_spark(rng);

        self.last_emit_ms = now_ms;
    }

    fn emit_spark(&mut self, rng: &mut Lcg) {
        let spark = self.spark(rng.next());

        self.spawned.push(Box::new(spark));
    }

    pub fn spark(&self, roll: u32) -> Billboard {
        let size = (roll % 5) as f32 * SPARK_SIZE_STEP + SPARK_SIZE_BASE;

        let mut spark = Billboard::new(
            SPARK_KIND,
            SPARK_LIFETIME_MS,
            Vec3::new(size, size, size),
            SPARK_GROWTH,
            SPARK_FRAMES,
            SPARK_FRAME_MS,
        );

        if let Some(argb) = self.spark_colour() {
            spark.set_color(argb);
        }

        spark.position = Vec3::new(
            self.position.x,
            self.position.y - SPARK_DROP,
            self.position.z,
        );

        spark.pass = render::billboard::BillboardPass::Additive;

        spark.lift = true;

        spark
    }

    pub fn mesh_id(&self) -> u32 {
        if self.mode == mode::SPIN {
            12
        } else {
            MESH_ID as u32
        }
    }

    pub fn mesh_draw(&self) -> Option<TexturedMesh> {
        if self.gone || self.t < 0.05 || self.mode == mode::BOUND {
            return None;
        }

        let (xz, y, tint) = match self.mode {
            1 => (5.0, 2.0, [1.0, 0.0, 0.0, 1.0]),
            4 => (5.8, 3.0, [221.0 / 255.0, 68.0 / 255.0, 0.0, 1.0]),
            2 => (1.0, 1.0, [170.0 / 255.0, 170.0 / 255.0, 170.0 / 255.0, 1.0]),
            _ => (1.5, 1.5, [170.0 / 255.0, 170.0 / 255.0, 170.0 / 255.0, 1.0]),
        };

        let rotation = Matrix4::rotation_yaw_pitch_roll(
            self.yaw,
            self.spin - std::f32::consts::FRAC_PI_2,
            std::f32::consts::FRAC_PI_2,
        );

        let world = Matrix4::multiply(&super::objects::DEVICE_WORLD, &rotation);

        let world = Matrix4::multiply(&world, &Matrix4::scaling(xz, y, xz));

        let world = Matrix4::multiply(
            &world,
            &Matrix4::translation(self.position.x, self.position.y, self.position.z),
        );

        Some(TexturedMesh {
            mesh_id: self.mesh_id(),
            world,
            texture: 91,
            tint,
            blend: 1,
            uv_offset: [0.0; 2],
        })
    }

    fn update_children(&mut self, tick: &Tick, ground: Option<&Ground<'_>>) {
        for child in &mut self.children {
            child.update(tick.now_ms, tick.yaw, tick.pitch);
        }

        if let Some(ground) = ground {
            self.shade.place(ground);
        }
    }
}

impl Effect for DoubleSwing {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let mut rng = Lcg::new(self.birth_ms);

        self.tick_with(tick.now_ms, &mut rng, |_| None);

        self.update_children(tick, None);
    }

    fn needs_ground(&self) -> bool {
        true
    }

    fn update_grounded(&mut self, tick: &Tick, ground: &Ground<'_>) {
        self.update(tick);

        self.shade.place(ground);
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        let followed = self.target_key.and_then(|key| (live.body)(key));

        if self.target_key.is_some() && followed.is_none() {
            self.target_key = None;

            self.target = None;
        }

        if let Some(body) = followed {
            self.target = Some(body.key.entity);
        }

        self.tick_with(tick.now_ms, live.rng, |_| {
            followed.as_ref().map(|body| body.position)
        });

        self.update_children(tick, live.ground);
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, _cx: &Submit<'_>, out: &mut Sink<'_>) {
        if let Some(draw) = self.mesh_draw() {
            out.mesh.push(MeshSubmission::Textured(draw));
        }
    }

    fn submit_effect_children(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        if self.gone {
            return;
        }

        self.shade.submit(cx, out);

        for child in &self.children {
            child.submit(cx, out);
        }
    }

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        std::mem::take(&mut self.spawned)
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}
