use super::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use super::effects::{Billboard, Motion};
use super::lod::DetailGate;
use super::rng::Lcg;
use glam::Vec3;
use render::billboard::BillboardPass;

pub const CLASS: Class = Class {
    name: "SkillSlowSlash",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const SPAWN_ARGUMENT_COUNT: usize = 9;

pub const OBJECT_SIZE: u16 = 184;

pub const LIFETIME_MS: u32 = 2000;

pub const LAST_EMIT_INIT: u32 = 0;

pub const EMIT_GAP_MS: u32 = 500;

pub const HALFWAY_T: f32 = 0.5;

pub const FOLLOW_UNTIL_T: f32 = 0.5;

pub const EXPIRE_T: f32 = 1.0;

pub const SPEED: f32 = 2.0;

pub const EMIT_GATE: DetailGate = DetailGate::Full;

pub const JITTER_MOD: u32 = 5;

pub const JITTER_STEP: f32 = 0.1;

pub const MOTION_MOD: u32 = 3;

pub const CHILD_TEXTURE: usize = 2;

pub const CHILD_MS: u32 = 2000;

pub const CHILD_GROWTH: f32 = 1e-4;

pub const CHILD_FRAMES: u32 = 1;

pub const CHILD_FRAME_MS: u32 = 0x50;

pub const CHILD_RISE: f32 = 1.5;

pub const CHILD_PHASE: f32 = 3.0;

pub const SOUND_KIND_0: u16 = 0xA7;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Look {
    pub argb: u32,
    pub amp_h: f32,
    pub size: f32,
}

pub const CHILD_COLOUR_MODE: u32 = 1;

pub fn look(kind: i32) -> Option<Look> {
    match kind {
        0 => Some(Look {
            argb: 0xAAAA_AA00,
            amp_h: 0.2,
            size: 0.05,
        }),
        1 => Some(Look {
            argb: 0xAA55_99AA,
            amp_h: 0.6,
            size: 0.05,
        }),
        2 => Some(Look {
            argb: 0xFFFF_EEFF,
            amp_h: 1.2,
            size: 0.02,
        }),
        _ => None,
    }
}

pub fn emitted_at(base: Vec3, r: u32) -> Vec3 {
    let step = f64::from(JITTER_STEP);

    let n = f64::from(r % JITTER_MOD);

    Vec3::new(
        ((n + 1.0) * step + f64::from(base.x)) as f32,
        base.y,
        (n * step + f64::from(base.z)) as f32,
    )
}

pub fn motion_of(r: u32) -> Motion {
    Motion::from_field(r % MOTION_MOD + 1)
}

#[derive(Debug)]
pub struct SlowSlash {
    pub source: Vec3,
    pub dest: Vec3,
    pub kind: i32,
    pub target: Option<u16>,
    pub target_key: Option<super::effect_context::ActorKey>,
    pub position: Vec3,
    pub birth_ms: u32,
    pub last_emit_ms: u32,
    children: Vec<Billboard>,
    rng: Lcg,
    gone: bool,
}

impl SlowSlash {
    pub fn new(source: Vec3, dest: Vec3, kind: i32, target: Option<u16>, now_ms: u32) -> SlowSlash {
        SlowSlash {
            source,
            dest,
            kind,
            target,
            target_key: None,
            position: source,
            birth_ms: now_ms,
            last_emit_ms: LAST_EMIT_INIT,
            children: Vec::new(),
            rng: Lcg::new(now_ms),
            gone: false,
        }
    }

    pub fn progress(&self, now_ms: u32) -> f32 {
        now_ms.wrapping_sub(self.birth_ms) as f32 / LIFETIME_MS as f32
    }

    pub fn due(&self, now_ms: u32) -> bool {
        now_ms.wrapping_sub(self.last_emit_ms) > EMIT_GAP_MS
    }

    pub fn advance(&mut self, t: f32, live: Option<Vec3>) {
        if let Some(at) = live {
            self.position = at;

            return;
        }

        if t >= HALFWAY_T {
            self.position = self.dest;

            return;
        }

        let v = t * SPEED;

        let u = 1.0 - v;

        self.position = Vec3::new(
            u * self.source.x + self.dest.x * v,
            self.source.y * u + self.dest.y * v,
            u * self.source.z + self.dest.z * v,
        );
    }

    pub fn tick_with(
        &mut self,
        now_ms: u32,
        detail: i32,
        rng: &mut Lcg,
        at: impl Fn(u16) -> Option<Vec3>,
    ) {
        let t = self.progress(now_ms);

        if t > FOLLOW_UNTIL_T {
            self.target = None;
        }

        if t > EXPIRE_T {
            self.gone = true;

            return;
        }

        if !self.due(now_ms) {
            return;
        }

        self.advance(t, self.target.and_then(&at));

        if EMIT_GATE.admits(detail) {
            self.emit(rng);
        }

        self.last_emit_ms = now_ms;
    }

    fn emit(&mut self, rng: &mut Lcg) {
        let Some(look) = look(self.kind) else {
            return;
        };

        let r = rng.next();

        let mut child = Billboard::new(
            CHILD_TEXTURE,
            CHILD_MS,
            Vec3::splat(look.size),
            CHILD_GROWTH,
            CHILD_FRAMES,
            CHILD_FRAME_MS,
        );

        child.pass = BillboardPass::Additive;

        let at = emitted_at(self.position, r);

        child.origin = at;

        child.position = at;

        child.set_color(look.argb);

        child.rise = CHILD_RISE;

        child.amp_h = look.amp_h;

        child.phase = CHILD_PHASE;

        child.motion = motion_of(r);

        self.children.push(child);
    }

    pub fn children(&self) -> &[Billboard] {
        &self.children
    }
}

impl Effect for SlowSlash {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let mut rng = self.rng;

        self.tick_with(tick.now_ms, 0, &mut rng, |_| None);

        self.rng = rng;
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        let target = self.target_key.and_then(|key| (live.body)(key));

        self.target = target.map(|body| body.key.entity);

        self.tick_with(tick.now_ms, live.detail, live.rng, |_| {
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
        std::mem::take(&mut self.children)
            .into_iter()
            .map(|c| Box::new(c) as Box<dyn Effect>)
            .collect()
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}
