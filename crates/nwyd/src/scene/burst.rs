use super::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use super::effects::{Billboard, BillboardScene, Motion};
use super::field::FieldScene;
use super::rng::Lcg;
use super::shade::Shade;
use glam::{Mat3, Vec3};
use render::billboard::BillboardPass;

pub const CLASS: Class = Class {
    name: "EffectParticle",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const TEXTURE_SLOT: usize = 0x38;

pub const FIRST_CHILD_MS: u32 = 1000;

pub const CHILD_STEP_MS: u32 = 300;

pub const CHILD_BONUS_MS: u32 = 1000;

pub const NO_BONUS_KIND: i32 = 4;

pub const SCALE_BASE: f32 = 0.1;

pub const AMP_H_STEP: f32 = 0.05;

pub const JITTER_XZ: f32 = 0.3;

pub const JITTER_Y: f32 = 0.1;

pub const JITTER_SPAN: u32 = 5;

pub const JITTER_BIAS: i32 = 3;

pub const PASTELS: [u32; 7] = [
    0xFFFF_FFFF,
    0xFFFF_AAAA,
    0xFFFF_FFAA,
    0xFFAA_FFAA,
    0xFFAA_AAFF,
    0xFFAA_FFFF,
    0xFFFF_AAFF,
];

pub const CHILD_FRAMES: u32 = 1;

pub const CHILD_FRAME_MS: u32 = 0x50;

pub const KIND2_RISE: f32 = -3.0;

pub const KIND3_RISE: f32 = 3.0;

pub const KIND4_RISE: f32 = 5.0;

pub const AXIS_KIND: i32 = 5;

pub const KIND5_COLOUR_MODE: u32 = 4;

pub const KIND5_YAW_SPAN: u32 = 300;

pub const KIND5_YAW_BASE: u32 = 700;

pub const KIND5_YAW_SCALE: f32 = 0.001;

pub const KIND5_TURN_SPAN: u32 = 365;

pub const KIND5_SEED_SPAN: u32 = 1000;

pub fn randomized_axis_direction(axis: Vec3, rng: &mut Lcg) -> Vec3 {
    let unit = axis.normalize_or_zero();

    let yaw = (rng.next() % KIND5_YAW_SPAN + KIND5_YAW_BASE) as f32 * KIND5_YAW_SCALE;

    let turned = Mat3::from_rotation_y(yaw) * axis;

    let theta =
        (rng.next() % KIND5_TURN_SPAN) as f32 / KIND5_TURN_SPAN as f32 * std::f32::consts::PI;

    (Mat3::from_axis_angle(unit, theta + theta) * turned).normalize_or_zero()
}

pub const DEATH_CLASS: i32 = 0x2C;

pub const EQUIP_SOUND: i64 = 0x9E;

pub const DEATH_SOUND: i64 = 0x135;

pub const EQUIP_SHADE_GRID: i32 = 4;

pub const EQUIP_SHADE_TEXTURE: usize = 7;

pub const EQUIP_SHADE_MS: u32 = 3000;

pub const EQUIP_SHADE_ARGB: u32 = 0xAAAA_AAAA;

pub const LEVEL_UP_KIND: i32 = 0;

pub const DEATH_SMOKE_TEXTURE: usize = 0x3B;

pub const DEATH_SMOKE_MS: u32 = 2500;

pub const DEATH_SMOKE_SCALE: f32 = 0.2;

pub const DEATH_SMOKE_GROWTH: f32 = f32::from_bits(0x3B44_9BA6);

pub const DEATH_LIFT: f32 = 2.0;

pub const BODY_LIFT: f32 = 1.0;

pub const DEATH_EXPLOSION_MODE: i32 = 0;

pub const DEATH_EXPLOSION_SCALE: f32 = 1.0;

pub const DEATH_EXPLOSION_INTERVAL_MS: i32 = 0xD2;

pub const DEATH_EXPLOSION_TINT: u32 = 0x4444_4444;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Recipe {
    pub at: Vec3,
    pub kind: i32,
    pub count: i32,
    pub spread: f32,
    pub argb: u32,
    pub random_colour: bool,
    pub texture: usize,
    pub scatter: f32,
    pub additive: bool,
    pub axis: Vec3,
    pub lifetime_ms: u32,
}

impl Recipe {
    pub const MEASURED_TAIL: Recipe = Recipe {
        at: Vec3::ZERO,
        kind: 0,
        count: 0,
        spread: 0.0,
        argb: 0,
        random_colour: false,
        texture: TEXTURE_SLOT,
        scatter: 1.0,
        additive: true,
        axis: Vec3::ZERO,
        lifetime_ms: FIRST_CHILD_MS,
    };
}

pub fn equip_recipe(at: Vec3) -> Recipe {
    Recipe {
        at: lifted(at, BODY_LIFT),
        kind: 1,
        count: 3,
        spread: 3.0,
        argb: 0,
        random_colour: true,
        ..Recipe::MEASURED_TAIL
    }
}

pub fn death_recipe(at: Vec3) -> Recipe {
    Recipe {
        at: lifted(at, DEATH_LIFT),
        kind: 4,
        count: 12,
        spread: 0.05,
        argb: 0xFFFF_AA00,
        random_colour: false,
        ..Recipe::MEASURED_TAIL
    }
}

pub fn air_move_recipe(at: Vec3) -> Recipe {
    Recipe {
        at: lifted(at, BODY_LIFT),
        kind: 1,
        count: 10,
        spread: 3.0,
        argb: 0,
        random_colour: true,
        ..Recipe::MEASURED_TAIL
    }
}

fn lifted(at: Vec3, by: f32) -> Vec3 {
    Vec3::new(at.x, at.y + by, at.z)
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Particle {
    pub at: Vec3,
    pub kind: i32,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub gone: bool,
}

impl Particle {
    pub fn new(recipe: &Recipe, now_ms: u32) -> Particle {
        Particle {
            at: recipe.at,
            kind: recipe.kind,
            birth_ms: now_ms,
            lifetime_ms: recipe.lifetime_ms,
            gone: false,
        }
    }
}

impl Effect for Particle {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        if self.lifetime_ms == 0 {
            return;
        }

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

pub fn build(recipe: &Recipe, now_ms: u32, rng: &mut Lcg) -> (Particle, Vec<Billboard>) {
    let emitter = Particle::new(recipe, now_ms);

    let mut children = Vec::new();

    for i in 0..recipe.count.max(0) {
        let i = i as u32;

        let base = FIRST_CHILD_MS + CHILD_STEP_MS * i;

        let life = match recipe.kind == NO_BONUS_KIND {
            true => base,
            false => base + CHILD_BONUS_MS,
        };

        let scale = (i & 1) as f32 * recipe.spread + SCALE_BASE;

        let mut child = Billboard::new(
            recipe.texture,
            life,
            Vec3::splat(scale),
            0.0,
            CHILD_FRAMES,
            CHILD_FRAME_MS,
        );

        child.amp_h = (i % 3) as f32 * AMP_H_STEP + SCALE_BASE;

        child.pass = match recipe.additive {
            true => BillboardPass::Additive,
            false => BillboardPass::Blended,
        };

        match recipe.kind {
            k if k < 2 => child.motion = Motion::from_field((i & 1) + k as u32 + 1),
            2 => {
                child.motion = Motion::Rise;

                child.rise = KIND2_RISE;
            }
            3 => {
                child.motion = Motion::Rise;

                child.rise = KIND3_RISE;
            }
            4 => {
                child.motion = Motion::SwayX;

                child.rise = KIND4_RISE;
            }
            AXIS_KIND => {
                child.colour_mode = KIND5_COLOUR_MODE;

                child.motion = Motion::Burst;

                child.dir = randomized_axis_direction(recipe.axis, rng);

                child.lifetime_ms = recipe.lifetime_ms;

                let _seed = rng.next();
            }
            _ => {}
        }

        let jitter = |rng: &mut Lcg| (rng.next() % JITTER_SPAN) as i32 - JITTER_BIAS;

        let x = jitter(rng) as f32 * JITTER_XZ * recipe.scatter + recipe.at.x;

        let y = jitter(rng) as f32 * JITTER_Y + recipe.at.y;

        let z = jitter(rng) as f32 * JITTER_XZ * recipe.scatter + recipe.at.z;

        let placed = match recipe.kind == AXIS_KIND {
            true => recipe.at,
            false => Vec3::new(x, y, z),
        };

        child.origin = placed;

        child.position = placed;

        child.set_color(match recipe.random_colour {
            true => PASTELS[(rng.next() % PASTELS.len() as u32) as usize],
            false => recipe.argb,
        });

        children.push(child);
    }

    (emitter, children)
}

pub fn spawn(scene: &mut BillboardScene, recipe: &Recipe, now_ms: u32, rng: &mut Lcg) -> usize {
    let (emitter, children) = build(recipe, now_ms, rng);

    let raised = children.len() + 1;

    for child in children {
        scene.spawn(Box::new(child));
    }

    scene.spawn(Box::new(emitter));

    raised
}

#[derive(Debug, Clone, PartialEq)]

pub struct Raised {
    pub path: &'static str,
    pub nodes: usize,
    pub sounds: Vec<i64>,
    pub explosion: Option<Vec3>,
    pub shades: usize,
    pub level_up: Option<super::timers::Raised>,
    pub refused: Option<&'static str>,
}

impl Raised {
    fn empty(path: &'static str, refused: &'static str) -> Raised {
        Raised {
            path,
            nodes: 0,
            sounds: Vec::new(),
            shades: 0,
            level_up: None,
            explosion: None,
            refused: Some(refused),
        }
    }
}

pub fn describe(raised: &Raised) -> String {
    if let Some(why) = raised.refused {
        return format!("NWYD: {} raised no effect — {why}", raised.path);
    }

    let level_up = match &raised.level_up {
        Some(up) => {
            format!(
                ", of which a EffectLevelUp kind {} contributed {}{}",
                up.kind,
                up.nodes,
                match up.detail_gated {
                    true => " with both billboard loops closed by the detail gate",
                    false => "",
                },
            )
        }
        None => String::new(),
    };

    format!(
        "NWYD: {} — {} effect node(s), {} shade(s), {} sound(s){level_up}",
        raised.path,
        raised.nodes,
        raised.shades,
        raised.sounds.len(),
    )
}

pub fn equip(scene: &mut FieldScene, index: usize) -> Raised {
    let now_ms = scene.time.now_ms;

    let Some(actor) = scene.world.actors.actors.get(index) else {
        return Raised::empty("0x36B", "no body at that index");
    };

    let (at, hidden) = (actor.position, super::cast::hidden(actor));

    let recipe = equip_recipe(at);

    let nodes = spawn(
        &mut scene.world.effects,
        &recipe,
        now_ms,
        &mut scene.world.actors.effect_rng,
    );

    scene.world.play_sound(EQUIP_SOUND);

    let shade = ground_shade(
        scene,
        EQUIP_SHADE_GRID,
        EQUIP_SHADE_TEXTURE,
        EQUIP_SHADE_ARGB,
        at,
        EQUIP_SHADE_MS,
    );

    scene.world.effects.spawn(Box::new(shade));

    let level_up = (!hidden).then(|| super::timers::spawn(scene, at, LEVEL_UP_KIND));

    Raised {
        path: "0x36B",
        nodes: nodes + level_up.as_ref().map_or(0, |r| r.nodes),
        sounds: vec![EQUIP_SOUND],
        shades: 1 + usize::from(level_up.is_some()),
        level_up,
        explosion: None,
        refused: None,
    }
}

pub fn death(scene: &mut FieldScene, index: usize) -> Raised {
    let now_ms = scene.time.now_ms;

    let Some(actor) = scene.world.actors.actors.get(index) else {
        return Raised::empty("0x338", "no body at that index");
    };

    if actor.class != DEATH_CLASS {
        return Raised::empty(
            "0x338",
            "appearance class is not 0x2C, so the arm is not taken",
        );
    }

    let at = lifted(actor.position, DEATH_LIFT);

    let recipe = death_recipe(actor.position);

    let mut nodes = spawn(
        &mut scene.world.effects,
        &recipe,
        now_ms,
        &mut scene.world.actors.effect_rng,
    );

    let mut boom = super::fire_ring::FireRing::new(
        at,
        DEATH_EXPLOSION_MODE,
        DEATH_EXPLOSION_SCALE,
        DEATH_EXPLOSION_INTERVAL_MS,
        DEATH_EXPLOSION_TINT,
        now_ms,
        Some(&death_ground(scene)),
    );

    let raised_at = boom.position;

    let mut shades = 0;

    for child in Effect::take_spawned(&mut boom) {
        shades += usize::from(child.class().name == "Shade");

        scene.world.effects.spawn(child);

        nodes += 1;
    }

    scene.world.effects.spawn(Box::new(boom));

    nodes += 1;

    let mut smoke = Billboard::new(
        DEATH_SMOKE_TEXTURE,
        DEATH_SMOKE_MS,
        Vec3::splat(DEATH_SMOKE_SCALE),
        DEATH_SMOKE_GROWTH,
        CHILD_FRAMES,
        CHILD_FRAME_MS,
    );

    smoke.position = at;

    smoke.origin = at;

    smoke.pass = BillboardPass::Blended;

    smoke.set_color(0xFFFF_FFFF);

    scene.world.effects.spawn(Box::new(smoke));

    nodes += 1;

    scene.world.play_sound(DEATH_SOUND);

    Raised {
        path: "0x338",
        nodes,
        sounds: vec![DEATH_SOUND],
        shades,
        level_up: None,
        explosion: Some(raised_at),
        refused: None,
    }
}

fn death_ground(scene: &FieldScene) -> super::shade::Ground<'_> {
    let heights = &scene.world.actors.heights;

    let mut zones: Vec<&formats::world::trn::Terrain> = vec![&scene.world.terrain];

    zones.extend(scene.world.streamed.iter().map(|s| &s.terrain));

    super::shade::Ground::new(&scene.world.terrain, heights, zones)
}

pub fn arrival(scene: &mut FieldScene) -> Raised {
    air_move(scene, "AirMove arrival")
}

pub fn departure(scene: &mut FieldScene) -> Raised {
    air_move(scene, "AirMove departure")
}

fn air_move(scene: &mut FieldScene, path: &'static str) -> Raised {
    let now_ms = scene.time.now_ms;

    let Some(index) = scene.player else {
        return Raised::empty(path, "no local player");
    };

    let Some(actor) = scene.world.actors.actors.get(index) else {
        return Raised::empty(path, "no body at the player's index");
    };

    let recipe = air_move_recipe(actor.position);

    let nodes = spawn(
        &mut scene.world.effects,
        &recipe,
        now_ms,
        &mut scene.world.actors.effect_rng,
    );

    Raised {
        path,
        nodes,
        sounds: Vec::new(),
        shades: 0,
        level_up: None,
        explosion: None,
        refused: None,
    }
}

fn ground_shade(
    scene: &FieldScene,
    grid: i32,
    texture: usize,
    argb: u32,
    centre: Vec3,
    lifetime_ms: u32,
) -> Shade {
    let mut shade = Shade::new(
        grid,
        texture,
        argb,
        centre,
        super::effects::SECTOR_UNASSIGNED,
    )
    .with_lifetime(lifetime_ms);

    let heights = &scene.world.actors.heights;

    let mut ground: Vec<&formats::world::trn::Terrain> = vec![&scene.world.terrain];

    ground.extend(scene.world.streamed.iter().map(|s| &s.terrain));

    shade.place(&super::shade::Ground::new(
        &scene.world.terrain,
        heights,
        ground,
    ));

    shade
}
