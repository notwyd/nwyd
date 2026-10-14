use crate::scene::burst::{self, Particle};
use crate::scene::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use crate::scene::effects::{Billboard, Motion, SECTOR_UNASSIGNED};
use crate::scene::field::FieldScene;
use crate::scene::lod::{DetailGate, Ladder};
use crate::scene::rng::Lcg;
use crate::scene::shade::{Ground, Shade};
use crate::scene::vfx::{Ambient, Cx, Declined, Recipe};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Axis, Spawn, SpawnArgument};
use glam::Vec3;
use render::billboard::BillboardPass;

pub const CLASS: Class = Class {
    name: "SkillCure",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const LIFETIME_MS: u32 = 3000;

pub const PARTICLES: Ladder = Ladder {
    rungs: &[(10.0, 2), (20.0, 8), (30.0, 12)],
    default: 16,
};

pub const BILLBOARDS: Ladder = Ladder {
    rungs: &[(10.0, 2), (25.0, 4)],
    default: 5,
};

pub const PARTICLE_GATE: DetailGate = DetailGate::Full;

pub const BILLBOARD_GATE: DetailGate = DetailGate::Full;

pub const SHADE_GATE: DetailGate = DetailGate::Always;

pub const TEXTURE_SLOT: usize = 0x38;

pub const MOTE_SPREAD: f32 = 0.1;

pub const MOTE_ARGB: u32 = 0;

pub const MOTE_KIND: i32 = 0;

pub const FIRST_BILLBOARD_MS: u32 = 2000;

pub const BILLBOARD_STEP_MS: u32 = 300;

pub const BILLBOARD_SCALE_STEP: f32 = 0.3;

pub const BILLBOARD_AMP_H: f32 = 0.7;

pub const BILLBOARD_MOTION: u32 = 3;

pub const BILLBOARD_FRAMES: u32 = 1;

pub const BILLBOARD_FRAME_MS: u32 = 0x50;

pub const BILLBOARD_GROWTH: f32 = 0.0;

pub const BILLBOARD_ARGB: u32 = 0xFFFF_FFFF;

pub const JITTER_SPAN: u32 = 5;

pub const JITTER_BIAS: i32 = 3;

pub const JITTER_XZ: f32 = 0.3;

pub const JITTER_Y: f32 = 0.1;

pub const SHADE_GRID: i32 = 4;

pub const SHADE_TEXTURE: usize = 7;

pub const SHADE_RADIUS: f32 = 1.0;

pub const SHADE_ARGB: u32 = 0x5555_5555;

pub const SHADE_MS: u32 = 3000;

pub const SOUND: i64 = 0x04;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Cure {
    pub at: Vec3,
    pub reserved_parameter: i32,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub gone: bool,
}

impl Cure {
    pub fn new(at: Vec3, reserved_parameter: i32, now_ms: u32) -> Cure {
        Cure {
            at,
            reserved_parameter,
            birth_ms: now_ms,
            lifetime_ms: LIFETIME_MS,
            gone: false,
        }
    }
}

impl Effect for Cure {
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
    pub particle: Option<Particle>,
    pub motes: Vec<Billboard>,
    pub billboards: Vec<Billboard>,
    pub shade: Shade,
    pub emitter: Cure,
}

impl Built {
    pub fn nodes(&self) -> usize {
        self.motes.len() + usize::from(self.particle.is_some()) + self.billboards.len() + 2
    }

    pub fn push_into(self, ground: Option<&Ground<'_>>, out: &mut Vec<Box<dyn Effect>>) {
        let Built {
            particle,
            motes,
            billboards,
            mut shade,
            emitter,
        } = self;

        for mote in motes {
            out.push(Box::new(mote));
        }

        if let Some(particle) = particle {
            out.push(Box::new(particle));
        }

        for billboard in billboards {
            out.push(Box::new(billboard));
        }

        if let Some(ground) = ground {
            shade.place(ground);
        }

        out.push(Box::new(shade));

        out.push(Box::new(emitter));
    }
}

pub fn particle_recipe(at: Vec3, count: i32) -> burst::Recipe {
    burst::Recipe {
        at,
        kind: MOTE_KIND,
        count,
        spread: MOTE_SPREAD,
        argb: MOTE_ARGB,
        random_colour: true,
        ..burst::Recipe::MEASURED_TAIL
    }
}

pub fn build(
    at: Vec3,
    reserved_parameter: i32,
    now_ms: u32,
    rate: f32,
    detail: i32,
    rng: &mut Lcg,
) -> Built {
    let emitter = Cure::new(at, reserved_parameter, now_ms);

    let count = PARTICLES.at(rate);

    let (particle, motes) = match PARTICLE_GATE.admits(detail) {
        true => {
            let (emitter, children) = burst::build(&particle_recipe(at, count), now_ms, rng);

            (Some(emitter), children)
        }
        false => (None, Vec::new()),
    };

    let trips = BILLBOARDS.at(rate);

    let mut billboards = Vec::new();

    if BILLBOARD_GATE.admits(detail) && 0 < trips {
        let mut life_ms = FIRST_BILLBOARD_MS;

        for i in 0..trips as u32 {
            billboards.push(billboard(at, i, life_ms, rng));

            life_ms += BILLBOARD_STEP_MS;
        }
    }

    let shade = Shade::new(SHADE_GRID, SHADE_TEXTURE, SHADE_ARGB, at, SECTOR_UNASSIGNED)
        .with_lifetime(SHADE_MS);

    Built {
        particle,
        motes,
        billboards,
        shade,
        emitter,
    }
}

fn billboard(at: Vec3, i: u32, life_ms: u32, rng: &mut Lcg) -> Billboard {
    let scale = (i & 1) as f32 * BILLBOARD_SCALE_STEP + BILLBOARD_SCALE_STEP;

    let mut child = Billboard::new(
        TEXTURE_SLOT,
        life_ms,
        Vec3::splat(scale),
        BILLBOARD_GROWTH,
        BILLBOARD_FRAMES,
        BILLBOARD_FRAME_MS,
    );

    child.pass = BillboardPass::Additive;

    child.amp_h = BILLBOARD_AMP_H;

    child.motion = Motion::from_field(BILLBOARD_MOTION);

    let draw = |rng: &mut Lcg| (rng.next() % JITTER_SPAN) as i32 - JITTER_BIAS;

    let x = draw(rng) as f32 * JITTER_XZ + at.x;

    let y = draw(rng) as f32 * JITTER_Y + at.y;

    let z = draw(rng) as f32 * JITTER_XZ + at.z;

    let placed = Vec3::new(x, y, z);

    child.origin = placed;

    child.position = placed;

    child.set_color(BILLBOARD_ARGB);

    child
}

pub const RECIPE: Recipe = Recipe {
    class: SkillClass::SkillCure,
    build: build_spawn,
};

pub fn build_spawn(
    spawn: &Spawn,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let Some(reserved_parameter) = read_spawn_parameter(spawn) else {
        return Err(Declined::Arguments);
    };

    let Some(at) = cx.at.at(spawn.at) else {
        return Err(Declined::Unplaced);
    };

    build(
        at,
        reserved_parameter,
        cx.now_ms,
        cx.rate,
        cx.detail,
        env.rng,
    )
    .push_into(env.ground, out);

    Ok(())
}

fn read_spawn_parameter(spawn: &Spawn) -> Option<i32> {
    match spawn.arguments {
        [
            SpawnArgument::Allocation(_),
            SpawnArgument::Pos(Axis::X),
            SpawnArgument::Pos(Axis::Y),
            SpawnArgument::Pos(Axis::Z),
            SpawnArgument::Int(reserved_parameter),
        ] => Some(*reserved_parameter),
        _ => None,
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Raised {
    pub nodes: usize,
    pub motes: usize,
    pub particle: bool,
    pub billboards: usize,
    pub sounds: Vec<i64>,
    pub detail_gated: bool,
}

pub fn spawn(scene: &mut FieldScene, at: Vec3, reserved_parameter: i32, rate: f32) -> Raised {
    let now_ms = scene.time.now_ms;

    let detail = match scene.world.actors.effects_detail.effects {
        true => 0,
        false => 1,
    };

    let built = build(
        at,
        reserved_parameter,
        now_ms,
        rate,
        detail,
        &mut scene.world.actors.effect_rng,
    );

    let raised = Raised {
        nodes: built.nodes(),
        motes: built.motes.len(),
        particle: built.particle.is_some(),
        billboards: built.billboards.len(),
        sounds: vec![SOUND],
        detail_gated: detail != 0,
    };

    let Built {
        particle,
        motes,
        billboards,
        mut shade,
        emitter,
    } = built;

    for mote in motes {
        scene.world.effects.spawn(Box::new(mote));
    }

    if let Some(particle) = particle {
        scene.world.effects.spawn(Box::new(particle));
    }

    for billboard in billboards {
        scene.world.effects.spawn(Box::new(billboard));
    }

    let heights = &scene.world.actors.heights;

    let mut ground: Vec<&formats::world::trn::Terrain> = vec![&scene.world.terrain];

    ground.extend(scene.world.streamed.iter().map(|s| &s.terrain));

    shade.place(&crate::scene::shade::Ground::new(
        &scene.world.terrain,
        heights,
        ground,
    ));

    scene.world.effects.spawn(Box::new(shade));

    scene.world.effects.spawn(Box::new(emitter));

    scene.world.play_sound(SOUND);

    raised
}

pub fn describe(raised: &Raised) -> String {
    let gated = match raised.detail_gated {
        true => " (the effect detail setting closed the particle and the billboard loop)",
        false => "",
    };

    format!(
        "NWYD: cure effect — {} node(s), {} mote(s) under {} emitter(s), {} billboard(s), \
         1 shade, {} sound(s){gated}",
        raised.nodes,
        raised.motes,
        usize::from(raised.particle),
        raised.billboards,
        raised.sounds.len(),
    )
}
