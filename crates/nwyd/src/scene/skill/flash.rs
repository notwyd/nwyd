use crate::scene::effect_tree::{
    Base, Class, Cull, Effect, Pass, Sink, Submit, Tick, VISUAL_EFFECT_METHOD_COUNT,
};
use crate::scene::effects::{Billboard, SECTOR_UNASSIGNED};
use crate::scene::lod::DetailGate;
use crate::scene::quads::{GroundQuad, Slot};
use crate::scene::shade::{Ground, Shade};
use crate::scene::timers::TimedQuad;
use crate::scene::vfx::{Ambient, Cx, Declined, Recipe};
use formats::vfx::Spawn;
use formats::vfx::dispatch::Class as SkillClass;
use glam::Vec3;
use render::billboard::BillboardPass;

pub const CLASS: Class = Class {
    name: "SkillFlash",
    method_count: VISUAL_EFFECT_METHOD_COUNT,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const SPAWN_ARGUMENT_COUNT: usize = 5;

pub const LIFETIME_MS: u32 = 1000;

pub const SOUND: u16 = 0x9F;

pub const RUN_TEXTURE: usize = 0x3A;

pub const RUN_FIRST_MS: u32 = 1500;

pub const RUN_STEP_MS: u32 = 400;

pub const RUN_LIMIT_MS: u32 = 3100;

pub const RUN_SCALE: Vec3 = Vec3::new(0.6, 0.8, 0.6);

pub const RUN_OFFSET_X: [i32; 4] = [-2, 0, -1, -1];

pub const RUN_OFFSET_Z: [i32; 4] = [0, -1, 0, -1];

pub const RUN_OFFSET_STEP: f32 = 0.5;

pub const RUN_GROWTH_Y_BASE: f32 = 0.002;

pub const RUN_GROWTH_Y_STEP: f32 = 0.001;

pub const RUN_ARGB: u32 = 0xFFFF_FF00;

pub const CHILD_FRAMES: u32 = 1;

pub const CHILD_FRAME_MS: u32 = 0x50;

pub const QUAD_ONE_TEXTURE: usize = 0x5D;

pub const QUAD_MS: u32 = 2000;

pub const QUAD_ONE_SCALE: f32 = 0.01;

pub const QUAD_GROWTH: f32 = 0.002;

pub const QUAD_ONE_LIFT: f32 = 0.1;

pub const QUAD_ONE_ARGB: u32 = 0xFFFF_FFFF;

pub const QUAD_TWO_TEXTURE: usize = 2;

pub const QUAD_TWO_SCALE: f32 = 2.0;

pub const QUAD_TWO_LIFT: f32 = 0.3;

pub const QUAD_TWO_ARGB: u32 = 0xFFAA_AAAA;

pub const SHADE_GRID: i32 = 4;

pub const SHADE_TEXTURE: usize = 7;

pub const SHADE_RADIUS: f32 = 1.0;

pub const SHADE_ARGB: u32 = 0x5555_5555;

pub const SHADE_MS: u32 = 3000;

pub const DETAIL_GATE: DetailGate = DetailGate::Always;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Flash {
    pub at: Vec3,
    pub reserved_parameter: i32,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub gone: bool,
}

impl Flash {
    pub fn new(at: Vec3, reserved_parameter: i32, now_ms: u32) -> Flash {
        Flash {
            at,
            reserved_parameter,
            birth_ms: now_ms,
            lifetime_ms: LIFETIME_MS,
            gone: false,
        }
    }
}

impl Effect for Flash {
    fn class(&self) -> &'static Class {
        &CLASS
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
    pub billboards: Vec<Billboard>,
    pub quads: Vec<TimedQuad>,
    pub shade: Shade,
    pub token: Flash,
}

impl Built {
    pub fn nodes(&self) -> usize {
        self.billboards.len() + self.quads.len() + 2
    }

    pub fn push_into(self, out: &mut Vec<Box<dyn Effect>>) {
        let Built {
            billboards,
            quads,
            shade,
            token,
        } = self;

        for child in billboards {
            out.push(Box::new(child));
        }

        for quad in quads {
            out.push(Box::new(quad));
        }

        out.push(Box::new(shade));

        out.push(Box::new(token));
    }
}

pub fn build(at: Vec3, reserved_parameter: i32, now_ms: u32) -> Built {
    let mut billboards = Vec::new();

    let mut life = RUN_FIRST_MS;

    let mut i = 0usize;

    while life < RUN_LIMIT_MS {
        billboards.push(child(at, i, life));

        i += 1;

        life += RUN_STEP_MS;
    }

    let quads = vec![
        ground_quad(
            at,
            QUAD_ONE_TEXTURE,
            QUAD_ONE_SCALE,
            QUAD_ONE_LIFT,
            QUAD_ONE_ARGB,
            0,
        ),
        ground_quad(
            at,
            QUAD_TWO_TEXTURE,
            QUAD_TWO_SCALE,
            QUAD_TWO_LIFT,
            QUAD_TWO_ARGB,
            1,
        ),
    ];

    let shade = Shade::new(SHADE_GRID, SHADE_TEXTURE, SHADE_ARGB, at, SECTOR_UNASSIGNED)
        .with_lifetime(SHADE_MS);

    Built {
        billboards,
        quads,
        shade,
        token: Flash::new(at, reserved_parameter, now_ms),
    }
}

fn child(at: Vec3, i: usize, life: u32) -> Billboard {
    let mut child = Billboard::new(
        RUN_TEXTURE,
        life,
        RUN_SCALE,
        0.0,
        CHILD_FRAMES,
        CHILD_FRAME_MS,
    );

    let (dx, dz) = (RUN_OFFSET_X[i % 4], RUN_OFFSET_Z[i % 4]);

    child.position = Vec3::new(
        at.x + dx as f32 * RUN_OFFSET_STEP,
        at.y,
        at.z + dz as f32 * RUN_OFFSET_STEP,
    );

    child.growth.y = growth_y(i);

    child.pass = BillboardPass::Additive;

    child.lift = true;

    child.set_color(RUN_ARGB);

    child
}

pub fn growth_y(i: usize) -> f32 {
    i as f32 * RUN_GROWTH_Y_STEP + RUN_GROWTH_Y_BASE
}

fn ground_quad(
    at: Vec3,
    texture: usize,
    scale: f32,
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
            grow: QUAD_GROWTH,
            spin_period: 0,
            ordinal,
            argb,
        },
        QUAD_MS,
    )
}

pub const RECIPE: Recipe = Recipe {
    class: SkillClass::SkillFlash,
    build: build_spawn,
};

pub fn build_spawn(
    spawn: &Spawn,
    cx: &Cx<'_>,
    _env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    if spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return Err(Declined::Arguments);
    }

    let Some(at) = cx.at.at(spawn.at) else {
        return Err(Declined::Unplaced);
    };

    build(at, 0, cx.now_ms).push_into(out);

    Ok(())
}

pub fn spawn_with_ground(
    at: Vec3,
    reserved_parameter: i32,
    now_ms: u32,
    ground: &Ground<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) {
    let mut built = build(at, reserved_parameter, now_ms);

    built.shade.place(ground);

    built.push_into(out);
}

pub fn describe(built: &Built) -> String {
    format!(
        "NWYD: SkillFlash — {} node(s), {} billboard(s), {} ground quad(s), 1 shade, \
         sound {:#04X}",
        built.nodes(),
        built.billboards.len(),
        built.quads.len(),
        SOUND,
    )
}
