use super::burst::{self, Recipe as Burst};
use super::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use super::rng::Lcg;
use super::shade::{Ground, Shade};
use super::skinfx::{Blend, ColourAnim, Motion, SkinMeshEffect};
use glam::Vec3;

pub const SPAWN_ARGUMENT_COUNT: usize = 6;

pub const OBJECT_SIZE: u16 = 168;

pub const MODE_TELEPORT: i32 = 0;

pub const MODE_SPIN: i32 = 1;

pub const LIFETIME_MS: u32 = 2_000;

pub const LIFETIME_SPIN_MS: u32 = 1_500;

pub const EXPIRE_T: f32 = 1.0;

pub const FADE_T: f32 = 0.7;

pub const FADE_SPAN: f32 = 0.3;

pub const CHANNEL_FULL: f64 = 255.0;

pub const EMIT_T: f32 = 0.5;

pub const OWNER_LIFT: f32 = 2.0;

pub const AFTERIMAGES: usize = 3;

pub const AFTERIMAGE_MESH: i32 = 0x56;

pub const AFTERIMAGE_VARIANT: i32 = 3;

pub const AFTERIMAGE_STAGGER_MS: u32 = 400;

pub const AFTERIMAGE_SHORTEN_MS: u32 = 200;

pub const AFTERIMAGE_MOTION: Motion = Motion::WeaponTrack;

pub const AFTERIMAGE_BLEND: Blend = Blend::AdditiveTinted;

pub const AFTERIMAGE_COLOUR_ANIM: ColourAnim = ColourAnim::Cross;

pub const AFTERIMAGE_TINT_STEP: f64 = 0.4000000059604645;

pub const AFTERIMAGE_TINT_BASE: f64 = 0.20000000298023224;

pub const SHADE_GRID: i32 = 6;

pub const SHADE_GRID_SPIN: i32 = 3;

pub const SHADE_TEXTURE: usize = 7;

pub const SHADE_ARGB: u32 = 0xFFFF_0000;

pub const SHADE_ARGB_SPIN: u32 = 0x3355_5555;

pub const BURST_TEXTURE: usize = 0xE7;

pub const BURST_KIND: i32 = 5;

pub const BURST_COUNT: i32 = 20;

pub const BURST_MS: u32 = 2_400;

pub const BURST_ARGB: u32 = 0x00FF_7777;

pub const BURST_SPREAD: f32 = 0.8;

pub const BURST_SCATTER: f32 = 1.5;

pub const SPIN_BURST_COUNT: i32 = 2;

pub const SPIN_BURST_MS: u32 = 700;

pub const SPIN_BURST_SPREAD: f32 = 0.3;

pub const SPIN_BURST_SCATTER: f32 = 1.1;

pub const SPIN_TURN_RATE: f64 = 14.0;

pub const SPIN_COLOUR_SCALE: f64 = 4_286_019_328.0;

pub const CLASS: Class = Class {
    name: "SkillSpChange",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub fn lifetime_of(mode: i32) -> u32 {
    match mode == MODE_SPIN {
        true => LIFETIME_SPIN_MS,
        false => LIFETIME_MS,
    }
}

pub fn axis_of(angle: f32) -> Vec3 {
    Vec3::new(angle.cos(), 0.0, -angle.sin())
}

pub fn ramp(t: f32) -> u32 {
    let t = f64::from(t);

    let mut alpha: i64 = 0xFF;

    let mut green: i64 = 0;

    let mut red = (t * CHANNEL_FULL) as i64;

    let mut blue = ((1.0 - t) * CHANNEL_FULL) as i64;

    if t > f64::from(FADE_T) {
        let span = f64::from(FADE_SPAN);

        let u = t - f64::from(FADE_T);

        alpha -= ((u * CHANNEL_FULL) / span) as i64;

        red -= ((red as f64 * u) / span) as i64;

        green = -(((u * 0.0) / span) as i64);

        blue -= ((blue as f64 * u) / span) as i64;
    }

    ((alpha as u32) << 24) | ((red as u32) << 16) | ((green as u32) << 8) | (blue as u32)
}

pub fn spin_argb(t: f32) -> u32 {
    ((f64::from(t) * SPIN_COLOUR_SCALE) as i64) as u32
}

pub fn afterimage_tint(index: usize) -> f32 {
    (index as f64 * AFTERIMAGE_TINT_STEP + AFTERIMAGE_TINT_BASE) as f32
}

pub fn afterimage(
    at: Vec3,
    yaw: f32,
    index: usize,
    parent_life_ms: u32,
    now_ms: u32,
) -> SkinMeshEffect {
    let mut ghost = SkinMeshEffect::new(AFTERIMAGE_MESH, at, at, AFTERIMAGE_VARIANT);

    ghost.motion = AFTERIMAGE_MOTION;

    ghost.start_ms = Some(now_ms.wrapping_add(index as u32 * AFTERIMAGE_STAGGER_MS));

    ghost.lifetime_ms = parent_life_ms - index as u32 * AFTERIMAGE_SHORTEN_MS;

    ghost.blend = AFTERIMAGE_BLEND;

    ghost.colour_anim = AFTERIMAGE_COLOUR_ANIM;

    let tint = afterimage_tint(index);

    ghost.colour_a = [tint; 3];

    ghost.colour_b = [tint; 3];

    ghost.yaw = yaw;

    ghost
}

pub fn teleport_burst(at: Vec3, yaw: f32) -> Burst {
    Burst {
        at,
        kind: BURST_KIND,
        count: BURST_COUNT,
        spread: BURST_SPREAD,
        argb: BURST_ARGB,
        random_colour: false,
        texture: BURST_TEXTURE,
        scatter: BURST_SCATTER,
        additive: true,
        axis: axis_of(yaw),
        lifetime_ms: BURST_MS,
    }
}

pub fn spin_burst(at: Vec3, t: f32) -> Burst {
    Burst {
        at,
        kind: BURST_KIND,
        count: SPIN_BURST_COUNT,
        spread: SPIN_BURST_SPREAD,
        argb: spin_argb(t),
        random_colour: false,
        texture: BURST_TEXTURE,
        scatter: SPIN_BURST_SCATTER,
        additive: true,
        axis: axis_of((f64::from(t) * SPIN_TURN_RATE) as f32),
        lifetime_ms: SPIN_BURST_MS,
    }
}

#[derive(Debug)]
pub struct Built {
    pub emitter: SpChange,
    pub afterimages: Vec<SkinMeshEffect>,
}

pub fn build(at: Vec3, mode: i32, yaw: f32, now_ms: u32) -> Built {
    let life = lifetime_of(mode);

    let afterimages = match mode == MODE_TELEPORT {
        true => (0..AFTERIMAGES)
            .map(|i| afterimage(at, yaw, i, life, now_ms))
            .collect(),
        false => Vec::new(),
    };

    Built {
        emitter: SpChange::new(at, mode, yaw, now_ms),
        afterimages,
    }
}

#[derive(Debug)]
pub struct SpChange {
    pub owner: Option<super::effect_context::ActorKey>,
    pub mode: i32,
    pub position: Vec3,
    pub yaw: f32,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub t: f32,
    pub shade: Option<Shade>,
    pub emitted: bool,
    spawned: Vec<Box<dyn Effect>>,
    gone: bool,
}

impl SpChange {
    pub fn new(position: Vec3, mode: i32, yaw: f32, now_ms: u32) -> SpChange {
        let decal = match mode {
            MODE_TELEPORT => Some((SHADE_GRID, SHADE_ARGB)),
            MODE_SPIN => Some((SHADE_GRID_SPIN, SHADE_ARGB_SPIN)),
            _ => None,
        };

        SpChange {
            owner: None,
            mode,
            position,
            yaw,
            birth_ms: now_ms,
            lifetime_ms: lifetime_of(mode),
            t: 0.0,
            shade: decal.map(|(grid, argb)| {
                Shade::new(
                    grid,
                    SHADE_TEXTURE,
                    argb,
                    position,
                    super::effects::SECTOR_UNASSIGNED,
                )
            }),
            emitted: false,
            spawned: Vec::new(),
            gone: false,
        }
    }

    pub fn place(&mut self, ground: &Ground<'_>) {
        if let Some(shade) = &mut self.shade {
            shade.place(ground);
        }
    }

    pub fn progress_at(&self, now_ms: u32) -> f32 {
        now_ms.wrapping_sub(self.birth_ms) as f32 / self.lifetime_ms as f32
    }

    pub fn emit_point(&self, owner: Option<Vec3>) -> Vec3 {
        match owner {
            Some(body) => Vec3::new(body.x, body.y + OWNER_LIFT, body.z),
            None => self.position,
        }
    }

    pub fn tick_with(&mut self, now_ms: u32, owner: Option<Vec3>, rng: &mut Lcg) {
        let t = self.progress_at(now_ms);

        self.t = t;

        if t > EXPIRE_T {
            self.gone = true;

            return;
        }

        if let Some(shade) = &mut self.shade {
            shade.set_color(ramp(t));
        }

        if t > EMIT_T && !self.emitted && self.mode == MODE_TELEPORT {
            let recipe = teleport_burst(self.emit_point(owner), self.yaw);

            self.emit(&recipe, now_ms, rng);
        }

        if self.mode == MODE_SPIN {
            let recipe = spin_burst(self.position, t);

            self.emit(&recipe, now_ms, rng);
        }
    }

    fn emit(&mut self, recipe: &Burst, now_ms: u32, rng: &mut Lcg) {
        let (emitter, children) = burst::build(recipe, now_ms, rng);

        for child in children {
            self.spawned.push(Box::new(child));
        }

        self.spawned.push(Box::new(emitter));

        self.emitted = true;
    }
}

impl Effect for SpChange {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let mut rng = Lcg::new(self.birth_ms);

        self.tick_with(tick.now_ms, None, &mut rng);
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        if live.scene == 0x7534 {
            self.owner = None;
        }

        let owner = self.owner.and_then(|key| (live.body)(key));

        if owner.is_none() {
            self.owner = None;
        }

        self.tick_with(tick.now_ms, owner.map(|body| body.position), live.rng);
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        if let Some(shade) = &self.shade {
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
