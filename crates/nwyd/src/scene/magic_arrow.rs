use super::effect_mesh::EffectMesh;
use super::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use super::effects::Billboard;
use super::fire::Fire;
use super::rng::Lcg;
use super::shade::{Ground, Shade};
use glam::Vec3;

pub const CLASS: Class = Class {
    name: "SkillMagicArrow",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

pub mod mode {

    pub const SHOT: i32 = 0;

    pub const EAGLE: i32 = 1;

    pub const FROST: i32 = 2;

    pub const SEED: i32 = 3;

    pub const UNSHIPPED: i32 = 4;

    pub const VINE: i32 = 5;
}

pub const MESH_SHOT: i32 = 0x2BD;

pub const MESH_EAGLE: i32 = 0x008;

pub const MESH_SEED: i32 = 0x1F9;

pub const MESH_UNSHIPPED: i32 = 0x1F8;

pub const SCALE_SHOT: f32 = 0.7;

pub const SCALE_FULL: f32 = 1.0;

pub const SCALE_VINE: f32 = 0.6;

pub const LIFETIME_PER_UNIT_MS: i32 = 0x32;

pub const LIFETIME_FLOOR_MS: i32 = 1;

pub const LIFETIME_CEILING_MS: i32 = 5000;

pub const LIFETIME_EAGLE_SCALE: f32 = 120.0;

pub const LIFETIME_FROST_SCALE: f32 = 200.0;

pub const LIFETIME_SEED_MS: i32 = 4000;

pub const FOLLOW_T: f32 = 0.3;

pub const EXPIRE_T: f32 = 1.0;

pub const DRAW_T: f32 = 0.05;

pub const EMIT_MS: u32 = 0x1E;

pub const BURST: usize = 3;

pub const SPARK_KIND: usize = 0;

pub const SPARK_LIFETIME_MS: u32 = 1000;

pub const SPARK_GROWTH: f32 = 0.001;

pub const SPARK_FRAMES: u32 = 1;

pub const SPARK_FRAME_MS: u32 = 0x50;

pub const SPARK_SIZE_STEP: f32 = 0.2;

pub const SPARK_SIZE_BASE: f32 = 0.3;

pub const SPARK_JITTER_STEP: f32 = 0.1;

pub const SPARK_DROP: f32 = 1.0;

pub const FIRE_LIFT: f32 = 0.5;

pub const GROUND_SCALE: f32 = 0.1;

pub const WOBBLE_CYCLES: f32 = 4.0;

pub const SOUND_SHOT: u16 = 0xA1;

pub const SOUND_EAGLE: u16 = 0x9B;

pub const SHADE_GRID: i32 = 4;

pub const SHADE_TEXTURE: usize = 7;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Dress {
    pub mesh_id: i32,
    pub scale: f32,
    pub mesh_argb: u32,
    pub child_argb: u32,
    pub sound: Option<u16>,
    pub shade: bool,
}

pub fn dress(mode: i32) -> Dress {
    let base = Dress {
        mesh_id: MESH_SHOT,
        scale: SCALE_SHOT,
        mesh_argb: 0xFFAA_88FF,
        child_argb: 0xFF44_22AA,
        sound: Some(SOUND_SHOT),
        shade: true,
    };

    match mode {
        mode::EAGLE => Dress {
            mesh_id: MESH_EAGLE,
            scale: SCALE_FULL,
            mesh_argb: 0x4444_4444,
            child_argb: 0xFFAA_8800,
            sound: Some(SOUND_EAGLE),
            ..base
        },
        mode::FROST => Dress {
            mesh_id: MESH_EAGLE,
            scale: SCALE_FULL,
            mesh_argb: 0xFF00_4488,
            child_argb: 0xFF00_1122,
            sound: Some(SOUND_EAGLE),
            ..base
        },
        mode::VINE => Dress {
            mesh_id: MESH_EAGLE,
            scale: SCALE_VINE,
            mesh_argb: 0xFF22_6644,
            child_argb: 0xFF11_2211,
            sound: None,
            shade: false,
        },
        mode::SEED | mode::UNSHIPPED => Dress {
            mesh_id: match mode == mode::SEED {
                true => MESH_SEED,
                false => MESH_UNSHIPPED,
            },
            mesh_argb: 0xFFFF_FFFF,
            sound: None,
            shade: false,
            ..base
        },
        _ => base,
    }
}

pub fn lifetime_ms(mode: i32, distance: f32) -> i32 {
    let raw = match mode {
        mode::SHOT => distance.trunc() as i32 * LIFETIME_PER_UNIT_MS,
        mode::EAGLE => (distance * LIFETIME_EAGLE_SCALE).trunc() as i32,
        mode::FROST | mode::VINE => (distance * LIFETIME_FROST_SCALE).trunc() as i32,
        mode::SEED | mode::UNSHIPPED => LIFETIME_SEED_MS,
        _ => distance.trunc() as i32 * LIFETIME_PER_UNIT_MS,
    };

    match raw {
        0 => LIFETIME_FLOOR_MS,
        ms => ms.min(LIFETIME_CEILING_MS),
    }
}

pub fn texture_bias(mode: i32, now_ms: u32) -> i32 {
    match mode {
        mode::SHOT => ((now_ms % 0xF0) / 0x28) as i32 + 0x14,
        mode::EAGLE => ((now_ms % 300) / 0x32) as i32 + 0x3D,
        mode::FROST | mode::VINE => ((now_ms % 300) / 0x32) as i32 + 0x65,
        mode::SEED => 0x61,
        mode::UNSHIPPED => 200,
        _ => ((now_ms % 0xF0) / 0x28) as i32 + 0x14,
    }
}

pub fn fire_order(mode: i32) -> Option<(i32, i32, u32)> {
    match mode {
        mode::FROST => Some((6, 0x4488, 0x4488)),
        mode::VINE => Some((6, 0x0011_2211, 0x0011_2211)),
        mode::EAGLE => Some((3, 0x4444_4444, 0x2233_1100)),
        _ => None,
    }
}

#[derive(Debug)]

pub struct MagicArrow {
    pub source: Vec3,
    pub dest: Vec3,
    pub mode: i32,
    pub target: Option<u16>,
    pub position: Vec3,
    pub birth_ms: u32,
    pub lifetime_ms: i32,
    pub t: f32,
    pub yaw: f32,
    pub dress: Dress,
    pub mesh: EffectMesh,
    pub shade: Option<Shade>,
    pub solo_stamp_ms: u32,
    pub target_key: Option<super::effect_context::ActorKey>,
    rng: Lcg,
    clock: std::rc::Rc<std::cell::Cell<u32>>,
    spawned: Vec<Box<dyn Effect>>,
    removed: bool,
    gone: bool,
}

impl MagicArrow {
    pub fn impact_explosion(
        &self,
        cx: &super::effect_tree::RemovalContext<'_>,
    ) -> Option<super::fire_ring::FireRing> {
        if cx.detail == 1 {
            return None;
        }

        let (mode, colour) = match self.mode {
            1 => (0, 0x4444_4444),
            2 => (2, 0x0000_4488),
            _ => return None,
        };

        Some(super::fire_ring::FireRing::new(
            self.dest, mode, 1.0, 180, colour, cx.now_ms, cx.ground,
        ))
    }

    pub fn new(source: Vec3, dest: Vec3, mode: i32, target: Option<u16>, now_ms: u32) -> Self {
        let dress = dress(mode);

        let lifetime_ms = lifetime_ms(mode, (dest - source).length());

        let mut mesh = EffectMesh::new(dress.mesh_id, dress.mesh_argb, 0.0, 1, now_ms);

        let scale = match mode == mode::SEED || mode == mode::UNSHIPPED {
            true => SCALE_FULL,
            false => dress.scale,
        };

        mesh.scale_xz = scale;

        mesh.scale_y = scale;

        if mode == mode::SEED || mode == mode::UNSHIPPED {
            mesh.blend = 0;
        }

        MagicArrow {
            source,
            dest,
            mode,
            target,
            position: source,
            birth_ms: now_ms,
            lifetime_ms,
            t: 0.0,
            yaw: 0.0,
            dress,
            mesh,
            shade: dress.shade.then(|| {
                Shade::new(
                    SHADE_GRID,
                    SHADE_TEXTURE,
                    dress.child_argb,
                    source,
                    super::effects::SECTOR_UNASSIGNED,
                )
            }),
            solo_stamp_ms: 0,
            target_key: None,
            rng: Lcg::new(now_ms),
            clock: Default::default(),
            spawned: Vec::new(),
            removed: false,
            gone: false,
        }
    }

    pub fn tick_with(
        &mut self,
        now_ms: u32,
        rng: &mut Lcg,
        stamp: &mut u32,
        ground: Option<&Ground<'_>>,
        at: impl Fn(u16) -> Option<Vec3>,
    ) {
        if self.gone {
            return;
        }

        self.mesh.texture_bias = texture_bias(self.mode, now_ms);

        let elapsed = now_ms.wrapping_sub(self.birth_ms) as f32;

        self.t = elapsed / self.lifetime_ms.max(LIFETIME_FLOOR_MS) as f32;

        if self.t > FOLLOW_T {
            self.target = None;
        }

        if let Some(live) = self.target.and_then(&at) {
            self.dest = Vec3::new(live.x, live.y + 1.0, live.z);
        }

        let delta = self.dest - self.source;

        self.yaw = delta.x.atan2(delta.z) - std::f32::consts::FRAC_PI_2;

        self.mesh.yaw = match self.mode == mode::SEED || self.mode == mode::UNSHIPPED {
            true => self.yaw + std::f32::consts::PI,
            false => self.yaw,
        };

        if self.t >= EXPIRE_T {
            self.gone = true;

            return;
        }

        self.position = self.source * (1.0 - self.t) + self.dest * self.t;

        if self.mode == mode::SEED {
            self.position.x += (self.t * WOBBLE_CYCLES * std::f32::consts::PI).sin();
        }

        self.mesh.position = self.position;

        if let Some(shade) = self.shade.as_mut() {
            shade.centre = Vec3::new(self.position.x, shade.centre.y, self.position.z);

            if let Some(ground) = ground {
                shade.place(ground);
            }
        }

        if now_ms.wrapping_sub(*stamp) <= EMIT_MS {
            return;
        }

        match self.mode {
            mode::SHOT => self.burst(rng),
            _ => self.emit_fire(ground, now_ms),
        }

        *stamp = now_ms;
    }

    fn burst(&mut self, rng: &mut Lcg) {
        for _ in 0..BURST {
            let spark = self.spark(rng.next());

            self.spawned.push(Box::new(spark));
        }
    }

    pub fn spark(&self, draw: u32) -> Billboard {
        let roll = (draw % 5) as f32;

        let size = roll * SPARK_SIZE_STEP + SPARK_SIZE_BASE;

        let jitter = roll * SPARK_JITTER_STEP;

        let mut spark = Billboard::new(
            SPARK_KIND,
            SPARK_LIFETIME_MS,
            Vec3::splat(size),
            SPARK_GROWTH,
            SPARK_FRAMES,
            SPARK_FRAME_MS,
        );

        spark.position = Vec3::new(
            self.position.x + jitter,
            self.position.y - SPARK_DROP,
            self.position.z + jitter,
        );

        spark.pass = render::billboard::BillboardPass::Additive;

        spark.lift = true;

        spark.set_color(self.dress.child_argb);

        spark
    }

    fn emit_fire(&mut self, ground: Option<&Ground<'_>>, now_ms: u32) {
        let Some(mut fire) = self.fire_child(ground, now_ms) else {
            return;
        };

        self.spawned.extend(fire.take_spawned());

        self.spawned.push(Box::new(fire));
    }

    fn fire_child(&self, ground: Option<&Ground<'_>>, now_ms: u32) -> Option<Fire> {
        let (mode, spark_colour, colour) = fire_order(self.mode)?;

        let (x, z) = (self.position.x, self.position.z);

        let height = ground.and_then(|g| g.surface_height(x, z)).unwrap_or(0.0);

        let at = Vec3::new(self.position.x, height + FIRE_LIFT, self.position.z);

        Some(Fire::new(
            at,
            mode,
            None,
            spark_colour,
            colour,
            now_ms,
            ground,
        ))
    }
}

impl Effect for MagicArrow {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let mut rng = self.rng;

        let mut stamp = self.clock.get();

        self.tick_with(tick.now_ms, &mut rng, &mut stamp, None, |_| None);

        self.rng = rng;

        self.clock.set(stamp);
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn needs_ground(&self) -> bool {
        true
    }

    fn update_grounded(&mut self, tick: &Tick, ground: &Ground<'_>) {
        let mut stamp = self.clock.get();

        let mut rng = self.rng;

        self.tick_with(tick.now_ms, &mut rng, &mut stamp, Some(ground), |_| None);

        self.rng = rng;

        self.clock.set(stamp);
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        let mut stamp = self.clock.get();

        let target = self.target_key.and_then(|key| (live.body)(key));

        self.target = target.map(|body| body.key.entity);

        self.tick_with(tick.now_ms, live.rng, &mut stamp, live.ground, |_| {
            target.map(|body| body.position)
        });

        if self.target.is_none() {
            self.target_key = None;
        }

        self.clock.set(stamp);
    }

    fn bind_arrow_clock(&mut self, clock: std::rc::Rc<std::cell::Cell<u32>>) {
        self.clock = clock;
    }

    fn removed(
        &mut self,
        reason: super::effect_tree::Removal,
        cx: &super::effect_tree::RemovalContext<'_>,
    ) {
        if self.removed {
            return;
        }

        self.removed = true;

        self.gone = true;

        self.shade = None;

        if reason == super::effect_tree::Removal::SceneTeardown || cx.detail == 1 {
            return;
        }

        if let Some(mut explosion) = self.impact_explosion(cx) {
            self.spawned.extend(explosion.take_spawned());

            self.spawned.push(Box::new(explosion));
        } else if self.mode == 0 {
            let mut flash = Billboard::new(71, 888, Vec3::splat(1.5), 0.0, 8, 111);

            flash.position = self.dest - Vec3::Y * 0.5;

            flash.pass = render::billboard::BillboardPass::Additive;

            flash.colour_mode = 0;

            self.spawned.push(Box::new(flash));

            let mut shade = Shade::new(
                4,
                7,
                self.dress.child_argb,
                self.dest,
                super::effects::SECTOR_UNASSIGNED,
            )
            .with_lifetime(1500);

            if let Some(ground) = cx.ground {
                shade.place(ground);
            }

            self.spawned.push(Box::new(shade));
        }
    }

    fn submit(&self, _cx: &Submit<'_>, out: &mut Sink<'_>) {
        if self.gone || self.t < DRAW_T {
            return;
        }

        if let Some(draw) = self.mesh.draw()
            && let (Ok(mesh_id), Some(texture)) = (
                u32::try_from(draw.mesh_id),
                draw.texture_bias
                    .and_then(|slot| usize::try_from(slot).ok()),
            )
        {
            out.mesh.push(super::effect_tree::MeshSubmission::Textured(
                super::effect_tree::TexturedMesh {
                    mesh_id,
                    world: draw.world,
                    texture,
                    tint: draw.tint,
                    blend: draw.blend,
                    uv_offset: draw.uv_offset,
                },
            ));
        }
    }

    fn submit_effect_children(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        if self.gone || self.t < DRAW_T {
            return;
        }

        if let Some(shade) = self.shade.as_ref() {
            shade.submit(cx, out);
        }
    }

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        std::mem::take(&mut self.spawned)
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}
