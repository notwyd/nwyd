use super::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use super::effects::{Billboard, Motion};
use super::rng::Lcg;
use glam::Vec3;

pub const CLASS: Class = Class {
    name: "SkillFire",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

pub const DURATION_MS: i32 = 0x960;

pub const START_T: f32 = 0.01;

pub const HALFWAY_T: f32 = 0.5;

pub const EXPIRE_T: f32 = 1.0;

pub const SPARK_KIND: usize = 0x21;

pub const SPARK_LIFETIME_MS: u32 = 500;

pub const SPARK_SIZE: f32 = 0.7;

pub const SPARK_GROWTH: f32 = 0.001;

pub const SPARK_FRAMES: u32 = 9;

pub const SPARK_FRAME_MS: u32 = 0xB;

pub const JITTER_STEP: f32 = 0.01;

pub const SPARK_MOTION_PARAM: f32 = 3.0;

pub const MODES: usize = 9;

pub mod mode {

    pub const PLUME: i32 = 0;

    pub const ARROW_TRAIL: i32 = 3;

    pub const VINE_TRAIL: i32 = 6;
}

pub fn arm(mode: i32) -> Option<usize> {
    usize::try_from(mode).ok().filter(|m| *m < MODES)
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Spark {
    pub kind: usize,
    pub lifetime_ms: u32,
    pub size: Vec3,
    pub growth: f32,
    pub frames: u32,
    pub frame_ms: u32,
    pub rise: Option<f32>,
    pub tint: Option<u32>,
}

pub const SPARKS: [Spark; MODES] = [
    Spark {
        kind: 0x21,
        lifetime_ms: 500,
        size: Vec3::splat(0.7),
        growth: 0.001,
        frames: 9,
        frame_ms: 0x0B,
        rise: Some(SPARK_MOTION_PARAM),
        tint: None,
    },
    Spark {
        kind: 0x21,
        lifetime_ms: 1000,
        size: Vec3::new(1.2, 1.0, 1.2),
        growth: 0.002,
        frames: 9,
        frame_ms: 0x6F,
        rise: Some(4.0),
        tint: None,
    },
    Spark {
        kind: 0x21,
        lifetime_ms: 1000,
        size: Vec3::splat(0.4),
        growth: 0.001,
        frames: 9,
        frame_ms: 0x6F,
        rise: Some(4.0),
        tint: None,
    },
    Spark {
        kind: 0x21,
        lifetime_ms: 1000,
        size: Vec3::splat(0.6),
        growth: 0.001,
        frames: 9,
        frame_ms: 0x6F,
        rise: Some(1.7),
        tint: None,
    },
    Spark {
        kind: 0x47,
        lifetime_ms: 1000,
        size: Vec3::splat(0.6),
        growth: 0.001,
        frames: 8,
        frame_ms: 0x6F,
        rise: Some(1.7),
        tint: Some(0x3366),
    },
    Spark {
        kind: 0x47,
        lifetime_ms: 1000,
        size: Vec3::splat(0.4),
        growth: 0.001,
        frames: 8,
        frame_ms: 0x6F,
        rise: Some(4.0),
        tint: Some(0x6688),
    },
    Spark {
        kind: 0x47,
        lifetime_ms: 1000,
        size: Vec3::splat(0.4),
        growth: 0.001,
        frames: 8,
        frame_ms: 0x6F,
        rise: Some(1.7),
        tint: Some(0x3366),
    },
    Spark {
        kind: 0x02,
        lifetime_ms: 1000,
        size: Vec3::splat(0.8),
        growth: 0.001,
        frames: 1,
        frame_ms: 0x01,
        rise: Some(1.7),
        tint: Some(0x00FF_FFFF),
    },
    Spark {
        kind: 0x21,
        lifetime_ms: 1000,
        size: Vec3::new(1.2, 1.0, 1.2),
        growth: 0.002,
        frames: 9,
        frame_ms: 0x6F,
        rise: None,
        tint: None,
    },
];

pub const INTERVAL_MS: [u32; MODES] = [100, 300, 180, 10_000, 100, 180, 10_000, 1_000, 100];

pub const INTERVAL_LADDER: super::lod::Ladder = super::lod::Ladder {
    rungs: &[(10.0, 6), (20.0, 5), (30.0, 3)],
    default: 1,
};

pub const DURATION_MS_BY_MODE: [i32; MODES] = [
    DURATION_MS,
    DURATION_MS,
    800,
    800,
    DURATION_MS,
    800,
    800,
    1_000,
    100,
];

pub const SHADE_GRID: [Option<i32>; MODES] = [
    Some(4),
    Some(4),
    Some(2),
    Some(2),
    Some(4),
    Some(2),
    Some(2),
    None,
    None,
];

pub const SHADE_TEXTURE: usize = 7;

pub const SHADE_EXTRA_MS: u32 = 1_000;

pub fn decal(mode: i32, colour: u32, position: Vec3) -> Option<super::shade::Shade> {
    let row = arm(mode);

    let grid = row.map_or(SHADE_GRID[0], |m| SHADE_GRID[m])?;

    let duration = row.map_or(DURATION_MS, |m| DURATION_MS_BY_MODE[m]);

    Some(
        super::shade::Shade::new(
            grid,
            SHADE_TEXTURE,
            colour,
            position,
            super::effects::SECTOR_UNASSIGNED,
        )
        .with_lifetime(duration.max(0) as u32 + SHADE_EXTRA_MS),
    )
}

pub const OPEN_SOUND: i64 = 0x9B;

#[derive(Debug)]

pub struct Fire {
    pub position: Vec3,
    pub mode: i32,
    pub target: Option<u16>,
    pub target_key: Option<super::effect_context::ActorKey>,
    pub spark_colour: i32,
    pub colour: u32,
    pub birth_ms: u32,
    pub duration_ms: i32,
    pub t: f32,
    pub last_emit_ms: u32,
    decal: Option<super::shade::Shade>,
    sparks: Vec<Billboard>,
    sounds: Vec<i64>,
    rng: Lcg,
    decal_anchor: std::rc::Rc<std::cell::Cell<Vec3>>,
    gone: bool,
}

impl Fire {
    pub fn new(
        position: Vec3,
        mode: i32,
        target: Option<u16>,
        spark_colour: i32,
        colour: u32,
        now_ms: u32,
        ground: Option<&super::shade::Ground<'_>>,
    ) -> Fire {
        let row = arm(mode);

        let duration_ms = row.map_or(DURATION_MS, |m| DURATION_MS_BY_MODE[m]);

        let child = decal(mode, colour, position).map(|mut shade| {
            if let Some(ground) = ground {
                shade.place(ground);
            }

            shade
        });

        let sounds = match row.is_none_or(|m| SHADE_GRID[m] == Some(4)) {
            true => vec![OPEN_SOUND],
            false => Vec::new(),
        };

        Fire {
            position,
            mode,
            target,
            target_key: None,
            spark_colour,
            colour,
            birth_ms: now_ms,
            duration_ms,
            t: 0.0,
            last_emit_ms: 0,
            decal: child,
            sparks: Vec::new(),
            sounds,
            rng: Lcg::new(now_ms),
            decal_anchor: std::rc::Rc::new(std::cell::Cell::new(position)),
            gone: false,
        }
    }

    pub fn interval_ms(&self, rate: f32) -> u32 {
        let base = arm(self.mode).map_or(INTERVAL_MS[0], |m| INTERVAL_MS[m]);

        base.saturating_mul(INTERVAL_LADDER.at(rate).max(0) as u32)
    }

    pub fn tick_with(
        &mut self,
        now_ms: u32,
        detail: i32,
        rate: f32,
        rng: &mut Lcg,
        at: impl Fn(u16) -> Option<Vec3>,
    ) {
        let elapsed = now_ms.wrapping_sub(self.birth_ms) as f32;

        self.t = elapsed / self.duration_ms.max(1) as f32;

        if self.t < START_T {
            return;
        }

        if self.t > HALFWAY_T {
            self.target = None;
        }

        if self.t > EXPIRE_T {
            self.gone = true;

            return;
        }

        if now_ms.wrapping_sub(self.last_emit_ms) <= self.interval_ms(rate) {
            return;
        }

        if let Some(body) = self.target.and_then(&at) {
            self.position = body;
        }

        self.decal_anchor.set(self.position);

        if detail != 0 {
            self.last_emit_ms = now_ms;

            return;
        }

        self.emit(rng);

        self.last_emit_ms = now_ms;
    }

    pub fn spark(&self, jitter: f32) -> Option<Billboard> {
        let Some(row) = arm(self.mode) else {
            return None;
        };

        let spark_row = SPARKS[row];

        let mut spark = Billboard::new(
            spark_row.kind,
            spark_row.lifetime_ms,
            spark_row.size,
            spark_row.growth,
            spark_row.frames,
            spark_row.frame_ms,
        );

        spark.position = Vec3::new(
            self.position.x + jitter,
            self.position.y,
            self.position.z + jitter,
        );

        spark.origin = spark.position;

        spark.pass = render::billboard::BillboardPass::Additive;

        spark.motion = Motion::from_field(1);

        if let Some(rise) = spark_row.rise {
            spark.rise = rise;
        }

        if let Some(tint) = spark_row.tint {
            spark.set_color(tint);
        }

        spark.set_color(self.spark_colour as u32);

        Some(spark)
    }

    fn emit(&mut self, rng: &mut Lcg) {
        let jitter = (rng.next() % 5) as f32 * JITTER_STEP;

        if let Some(spark) = self.spark(jitter) {
            self.sparks.push(spark);
        }
    }

    pub fn sparks(&self) -> &[Billboard] {
        &self.sparks
    }

    pub fn decal(&self) -> Option<&super::shade::Shade> {
        self.decal.as_ref()
    }
}

impl Effect for Fire {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let mut rng = self.rng;

        self.tick_with(tick.now_ms, 0, super::lod::CAPTURE_RATE, &mut rng, |_| None);

        self.rng = rng;
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        let target = self.target_key.and_then(|key| (live.body)(key));

        self.target = target.map(|body| body.key.entity);

        self.tick_with(tick.now_ms, live.detail, live.rate, live.rng, |_| {
            target.map(|body| body.position)
        });

        if self.target.is_none() {
            self.target_key = None;
        }
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, _cx: &Submit<'_>, _out: &mut Sink<'_>) {}

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        let mut out: Vec<Box<dyn Effect>> = Vec::new();

        if let Some(decal) = self.decal.take() {
            out.push(Box::new(FollowingShade {
                shade: decal,
                anchor: self.decal_anchor.clone(),
            }));
        }

        out.extend(
            self.sparks
                .drain(..)
                .map(|b| Box::new(b) as Box<dyn Effect>),
        );

        out
    }

    fn take_sounds(&mut self) -> Vec<i64> {
        std::mem::take(&mut self.sounds)
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}

#[derive(Debug)]

struct FollowingShade {
    shade: super::shade::Shade,
    anchor: std::rc::Rc<std::cell::Cell<Vec3>>,
}

impl Effect for FollowingShade {
    fn class(&self) -> &'static Class {
        self.shade.class()
    }

    fn update(&mut self, tick: &Tick) {
        self.shade.centre = self.anchor.get();

        self.shade.update(tick);
    }

    fn needs_ground(&self) -> bool {
        true
    }

    fn update_grounded(&mut self, tick: &Tick, ground: &super::shade::Ground<'_>) {
        self.shade.centre = self.anchor.get();

        self.shade.place(ground);

        self.shade.update(tick);
    }

    fn expired(&self) -> bool {
        self.shade.expired()
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        self.shade.submit(cx, out);
    }

    fn cull(&self, eye: Vec3, field: bool) -> Cull {
        self.shade.cull(eye, field)
    }
}
