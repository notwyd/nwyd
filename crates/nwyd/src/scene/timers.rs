use super::aura::Detail;
use super::effect_tree::{
    Base, Class, Cull, Effect, Pass, SCENE_NODE_METHOD_COUNT, Sink, Submit, Tick,
};
use super::effects::{Billboard, Motion, SECTOR_UNASSIGNED};
use super::field::FieldScene;
use super::quads::{GroundQuad, Life, Slot};
use super::rng::Lcg;
use super::shade::Shade;
use glam::Vec3;
use render::billboard::BillboardPass;

pub const LEVEL_UP_CLASS: Class = Class {
    name: "EffectLevelUp",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const KINDS: usize = 3;

pub const LIFETIME_MS: u32 = 1000;

pub const RUN_A_TEXTURE: usize = 0x38;

pub const RUN_A_FIRST_MS: u32 = 2000;

pub const RUN_A_STEP_MS: u32 = 300;

pub const RUN_A_LIMIT_MS: u32 = 4400;

pub const RUN_A_AMP_H: f32 = 0.5;

pub const RUN_A_RISE: f32 = 1.5;

pub const RUN_A_JITTER: f32 = 0.2;

pub const RUN_A_LIFT: f32 = 0.3;

pub const RUN_A_TINTS: [u32; 2] = [0xFFFF_FF00, 0xFFFF_EEFF];

pub const RUN_B_TEXTURES: [usize; KINDS] = [0x36, 0x7A, 0x7A];

pub const RUN_B_FIRST_MS: u32 = 1500;

pub const RUN_B_STEP_MS: u32 = 400;

pub const RUN_B_LIMIT_MS: u32 = 3100;

pub const RUN_B_SCALE: f32 = 0.8;

pub const RUN_B_OFFSET_X: [i32; 4] = [-2, 0, -1, -1];

pub const RUN_B_OFFSET_Z: [i32; 4] = [0, -1, 0, -1];

pub const RUN_B_OFFSET_STEP: f32 = 0.5;

pub const RUN_B_DROP: f32 = 1.0;

pub const RUN_B_GROWTH_Y_BASE: f32 = 0.002;

pub const RUN_B_GROWTH_Y_STEP: f32 = 0.001;

pub const QUAD_TEXTURES: [usize; KINDS] = [0x37, 0x38, 0x38];

pub const QUAD_MS: u32 = 2000;

pub const QUAD_ONE_SCALE: f32 = 0.01;

pub const QUAD_ONE_GROWTH: f32 = 0.002;

pub const QUAD_ONE_LIFT: f32 = 0.1;

pub const QUAD_TWO_TEXTURE: usize = 2;

pub const QUAD_TWO_SCALE: f32 = 2.0;

pub const QUAD_TWO_LIFT: f32 = 0.3;

pub const QUAD_TWO_ARGB: u32 = 0xFF55_5555;

pub const SHADE_GRID: i32 = 4;

pub const SHADE_TEXTURE: usize = 7;

pub const SHADE_MS: u32 = 3000;

pub const TINTS: [u32; KINDS] = [0xFF55_8833, 0xFF66_6600, 0xFF66_5566];

pub const SHADE_TINTS: [u32; KINDS] = [0xFF33_5511, 0xFF33_3300, 0xFF33_2833];

pub const JITTER_SPAN: u32 = 5;

pub const JITTER_BIAS: i32 = 3;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LevelUp {
    pub at: Vec3,
    pub kind: i32,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub gone: bool,
}

impl LevelUp {
    pub fn new(at: Vec3, kind: i32, now_ms: u32) -> LevelUp {
        let kind = reduce_kind(kind);

        LevelUp {
            at,
            kind,
            birth_ms: now_ms,
            lifetime_ms: LIFETIME_MS,
            gone: false,
        }
    }
}

pub fn reduce_kind(kind: i32) -> i32 {
    let reduced = kind % KINDS as i32;

    match reduced {
        0..=2 => reduced,
        _ => 0,
    }
}

impl Effect for LevelUp {
    fn class(&self) -> &'static Class {
        &LEVEL_UP_CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let elapsed = tick.now_ms.wrapping_sub(self.birth_ms);

        if elapsed as f32 / self.lifetime_ms as f32 > 1.0 {
            self.gone = true;
        }
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, _cx: &Submit<'_>, _out: &mut Sink<'_>) {}

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Built {
    pub emitter: LevelUp,
    pub billboards: Vec<Billboard>,
    pub quads: Vec<TimedQuad>,
    pub shade: Shade,
}

impl Built {
    pub fn nodes(&self) -> usize {
        self.billboards.len() + self.quads.len() + 2
    }
}

pub fn build(at: Vec3, kind: i32, now_ms: u32, rng: &mut Lcg, detail: Detail) -> Built {
    let emitter = LevelUp::new(at, kind, now_ms);

    let kind = emitter.kind;

    let mut billboards = Vec::new();

    if detail.effects {
        if kind == 1 || kind == 2 {
            let mut life = RUN_A_FIRST_MS;

            let mut i = 0u32;

            while life < RUN_A_LIMIT_MS {
                billboards.push(run_a_child(at, kind, i, life, rng));

                i += 1;

                life += RUN_A_STEP_MS;
            }
        }

        let mut life = RUN_B_FIRST_MS;

        let mut i = 0usize;

        while life < RUN_B_LIMIT_MS {
            billboards.push(run_b_child(at, kind, i, life));

            i += 1;

            life += RUN_B_STEP_MS;
        }
    }

    let quads = vec![
        ground_quad(
            at,
            QUAD_TEXTURES[kind as usize],
            QUAD_ONE_SCALE,
            QUAD_ONE_GROWTH,
            QUAD_ONE_LIFT,
            TINTS[kind as usize],
            0,
        ),
        ground_quad(
            at,
            QUAD_TWO_TEXTURE,
            QUAD_TWO_SCALE,
            0.0,
            QUAD_TWO_LIFT,
            QUAD_TWO_ARGB,
            1,
        ),
    ];

    let shade = Shade::new(
        SHADE_GRID,
        SHADE_TEXTURE,
        SHADE_TINTS[kind as usize],
        at,
        SECTOR_UNASSIGNED,
    )
    .with_lifetime(SHADE_MS);

    Built {
        emitter,
        billboards,
        quads,
        shade,
    }
}

fn run_a_child(at: Vec3, kind: i32, i: u32, life: u32, rng: &mut Lcg) -> Billboard {
    let scale = (i & 1) as f32 * SCALE_STEP + SCALE_STEP;

    let mut child = Billboard::new(
        RUN_A_TEXTURE,
        life,
        Vec3::splat(scale),
        0.0,
        CHILD_FRAMES,
        CHILD_FRAME_MS,
    );

    child.pass = BillboardPass::Additive;

    child.amp_h = RUN_A_AMP_H;

    child.motion = Motion::Circle;

    child.rise = RUN_A_RISE;

    let jitter = |rng: &mut Lcg| (rng.next() % JITTER_SPAN) as i32 - JITTER_BIAS;

    let x = jitter(rng) as f32 * RUN_A_JITTER + at.x;

    let y = at.y + RUN_A_LIFT;

    let z = jitter(rng) as f32 * RUN_A_JITTER + at.z;

    let placed = Vec3::new(x, y, z);

    child.origin = placed;

    child.position = placed;

    child.set_color(match kind {
        1 => RUN_A_TINTS[0],
        _ => RUN_A_TINTS[1],
    });

    child
}

fn run_b_child(at: Vec3, kind: i32, i: usize, life: u32) -> Billboard {
    let mut child = Billboard::new(
        RUN_B_TEXTURES[kind as usize],
        life,
        Vec3::splat(RUN_B_SCALE),
        0.0,
        CHILD_FRAMES,
        CHILD_FRAME_MS,
    );

    let (dx, dz) = (RUN_B_OFFSET_X[i % 4], RUN_B_OFFSET_Z[i % 4]);

    child.position = Vec3::new(
        at.x + dx as f32 * RUN_B_OFFSET_STEP,
        at.y - RUN_B_DROP,
        at.z + dz as f32 * RUN_B_OFFSET_STEP,
    );

    child.growth.y = i as f32 * RUN_B_GROWTH_Y_STEP + RUN_B_GROWTH_Y_BASE;

    child.pass = BillboardPass::Additive;

    child.lift = true;

    child.set_color(TINTS[kind as usize]);

    child
}

fn ground_quad(
    at: Vec3,
    texture: usize,
    scale: f32,
    grow: f32,
    lift: f32,
    argb: u32,
    ordinal: usize,
) -> TimedQuad {
    TimedQuad::new(
        GroundQuad {
            owner: 0,
            slot: Slot::Placed,
            texture,
            pass: BillboardPass::Additive,
            anchor: Vec3::new(at.x, at.y + lift, at.z),
            base_scale: Vec3::splat(scale),
            grow,
            spin_period: 0,
            ordinal,
            argb,
        },
        QUAD_MS,
    )
}

const SCALE_STEP: f32 = 0.1;

const CHILD_FRAMES: u32 = 1;

const CHILD_FRAME_MS: u32 = 0x50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Raised {
    pub kind: i32,
    pub nodes: usize,
    pub billboards: usize,
    pub quads: usize,
    pub detail_gated: bool,
}

pub fn spawn(scene: &mut FieldScene, at: Vec3, kind: i32) -> Raised {
    let now_ms = scene.time.now_ms;

    let detail = scene.world.actors.effects_detail;

    let built = build(at, kind, now_ms, &mut scene.world.actors.effect_rng, detail);

    let raised = Raised {
        kind: built.emitter.kind,
        nodes: built.nodes(),
        billboards: built.billboards.len(),
        quads: built.quads.len(),
        detail_gated: !detail.effects,
    };

    let Built {
        emitter,
        billboards,
        quads,
        mut shade,
    } = built;

    for child in billboards {
        scene.world.effects.spawn(Box::new(child));
    }

    for quad in quads {
        scene.world.effects.spawn(Box::new(quad));
    }

    let heights = &scene.world.actors.heights;

    let mut ground: Vec<&formats::world::trn::Terrain> = vec![&scene.world.terrain];

    ground.extend(scene.world.streamed.iter().map(|s| &s.terrain));

    shade.place(&super::shade::Ground::new(
        &scene.world.terrain,
        heights,
        ground,
    ));

    scene.world.effects.spawn(Box::new(shade));

    scene.world.effects.spawn(Box::new(emitter));

    raised
}

pub fn describe(raised: &Raised) -> String {
    let gated = match raised.detail_gated {
        true => " (the effect detail setting closed both billboard loops)",
        false => "",
    };

    format!(
        "NWYD: EffectLevelUp kind {} — {} node(s), {} billboard(s), {} ground quad(s), \
         1 shade{gated}",
        raised.kind, raised.nodes, raised.billboards, raised.quads,
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TimedQuad {
    pub quad: GroundQuad,
    pub lifetime_ms: u32,
    pub birth_ms: Option<u32>,
    pub elapsed_ms: u32,
    pub gone: bool,
}

impl TimedQuad {
    pub fn new(quad: GroundQuad, lifetime_ms: u32) -> TimedQuad {
        TimedQuad {
            quad,
            lifetime_ms,
            birth_ms: None,
            elapsed_ms: 0,
            gone: false,
        }
    }
}

impl Effect for TimedQuad {
    fn class(&self) -> &'static Class {
        &super::quads::CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let birth = *self.birth_ms.get_or_insert(tick.now_ms);

        let elapsed = tick.now_ms.wrapping_sub(birth);

        self.elapsed_ms = if self.lifetime_ms != 0 && elapsed >= 0x8000_0000 {
            0
        } else {
            elapsed
        };

        if self.lifetime_ms != 0 && self.lifetime_ms < self.elapsed_ms {
            self.gone = true;
        }
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        let life = match self.lifetime_ms {
            0 => None,
            lifetime_ms => Some(Life {
                lifetime_ms,
                elapsed_ms: self.elapsed_ms,
            }),
        };

        self.quad.submit_with(cx, out, life);
    }

    fn cull(&self, eye: Vec3, field: bool) -> Cull {
        Effect::cull(&self.quad, eye, field)
    }
}

pub const FIRE_CLASS: Class = Class {
    name: "FireEffect",
    method_count: SCENE_NODE_METHOD_COUNT,
    base: Base::SceneNode,
    pass: Pass::Effect,
};

pub const FIRE_MS_PER_UNIT: f32 = 1000.0;

pub const FIRE_MS_BIAS: f32 = 10.0;

pub const FIRE_MAX_MS: u32 = 2500;

pub const FIRE_MIN_MS: u32 = 1;

pub const FIRE_TARGET_LIFT: f32 = 1.0;

pub const FIRE_EMIT_STEPS: f32 = 20.0;

pub const FIRE_CHILD_MS: u32 = 1000;

pub const FIRE_CHILD_SCALE: f32 = 0.5;

pub const FIRE_CHILD_GROWTH_STEP: f32 = 0.0002;

pub const FIRE_CHILD_MOTION: u32 = 0x11;

pub const FIRE_SOUND: i64 = 0x4D;

pub const FIRE_TEXTURE_BASE: usize = 0x2A;

pub const FIRE_TEXTURE_SPAN: i32 = 2;

#[derive(Debug, Clone, PartialEq)]
pub struct Fire {
    pub at: Vec3,
    pub target: Vec3,
    pub dir: Vec3,
    pub distance: f32,
    pub texture_base: usize,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub emitted: i32,
    pub detail: bool,
    pub gone: bool,
    sounds: Vec<i64>,
    emitted_children: Vec<Billboard>,
}

impl Fire {
    pub fn new(at: Vec3, target: Vec3, texture_base: usize, now_ms: u32, detail: bool) -> Fire {
        let target = Vec3::new(target.x, target.y + FIRE_TARGET_LIFT, target.z);

        let delta = target - at;

        let distance = delta.length();

        let dir = match distance {
            0.0 => Vec3::ZERO,
            _ => delta / distance,
        };

        Fire {
            at,
            target,
            dir,
            distance,
            texture_base,
            birth_ms: now_ms,
            lifetime_ms: Fire::lifetime_for(distance),
            emitted: 0,
            detail,
            gone: false,
            sounds: vec![FIRE_SOUND],
            emitted_children: Vec::new(),
        }
    }

    pub fn lifetime_for(distance: f32) -> u32 {
        let raw = (distance * FIRE_MS_PER_UNIT - FIRE_MS_BIAS) as i32 as u32;

        match raw {
            0 => FIRE_MIN_MS,
            n if n > FIRE_MAX_MS => FIRE_MAX_MS,
            n => n,
        }
    }

    pub fn child(&self, emitted: i32) -> Billboard {
        let slot = self.texture_base + (emitted % FIRE_TEXTURE_SPAN).unsigned_abs() as usize;

        let mut child = Billboard::new(
            slot,
            FIRE_CHILD_MS,
            Vec3::splat(FIRE_CHILD_SCALE),
            emitted as f32 * FIRE_CHILD_GROWTH_STEP,
            1,
            0x50,
        );

        child.pass = BillboardPass::Additive;

        child.origin = self.at;

        child.position = self.at;

        child.motion = Motion::from_field(FIRE_CHILD_MOTION);

        child.dir = self.dir;

        child
    }

    pub fn take_emitted(&mut self) -> Vec<Billboard> {
        std::mem::take(&mut self.emitted_children)
    }

    pub fn pending(&self) -> usize {
        self.emitted_children.len()
    }
}

impl Effect for Fire {
    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        self.detail = live.detail == 0;

        self.update(tick);
    }

    fn class(&self) -> &'static Class {
        &FIRE_CLASS
    }

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        self.take_emitted()
            .into_iter()
            .map(|child| Box::new(child) as Box<dyn Effect>)
            .collect()
    }

    fn take_sounds(&mut self) -> Vec<i64> {
        std::mem::take(&mut self.sounds)
    }

    fn update(&mut self, tick: &Tick) {
        let elapsed = tick.now_ms.wrapping_sub(self.birth_ms);

        let alive = self.lifetime_ms == 0 || elapsed <= self.lifetime_ms || (elapsed as i32) < 0;

        if !alive {
            self.gone = true;

            return;
        }

        if self.lifetime_ms == 0 {
            return;
        }

        let r = elapsed as f32 / self.lifetime_ms as f32;

        if !(0.0..1.0).contains(&r) || !self.detail {
            return;
        }

        if (self.emitted as f32) < r * FIRE_EMIT_STEPS {
            self.emitted += 1;

            let child = self.child(self.emitted);

            self.emitted_children.push(child);
        }
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, _cx: &Submit<'_>, _out: &mut Sink<'_>) {}
}
