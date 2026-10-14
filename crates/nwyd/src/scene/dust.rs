use super::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use super::effects::{Billboard, Motion};
use super::rng::Lcg;
use formats::world::objects::{ObjRecord, ObjectClass};
use glam::Vec3;

pub const CLASS: Class = Class {
    name: "EffectDust",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

pub const GROUND_PERIOD_MS: u32 = 500;

pub const WATERFALL_PERIOD_MS: u32 = 0x0A8C;

pub const SEED_STEP: f32 = 0.2;

pub const SEED_MODULUS: u32 = 8;

pub const PLUME_WIDTH_STEP: f32 = 0.1;

pub const PLUME_HEIGHT_STEP: f32 = 0.3;

pub const PLUME_JITTER_X: f32 = 0.05;

pub const PLUME_JITTER_Z: f32 = 0.02;

pub const PLUME_MODULUS: u32 = 5;

pub const PLUME_BIAS: i32 = 3;

pub const PLUME_GROWTH: f32 = 1.0e-4;

pub const PLUME_TEXTURE: usize = 0x77;

pub const PLUME_LIFETIME_STEP_MS: u32 = 100;

pub const PLUME_LIFETIME_MS: u32 = 1_000;

pub const PLUME_LIFETIME_END_MS: u32 = 0x0578;

pub const BURST_LIFETIME_MS: u32 = 3_000;

pub const BURST_LIFETIME_END_MS: u32 = 0x0CE4;

pub const BURST_DROP: f32 = 0.4;

pub const PLUME_MOTION: Motion = Motion::Rise;

pub const PLUME_RISE_BASE: f32 = -6.0;

pub const PLUME_RISE_STEP: f32 = 0.1;

pub const PLUME_COUNT: usize = 4;

pub const BURST_COUNT: usize = 3;

pub const LANES: usize = 4;

pub const WATERFALL_MESH_BASE: u32 = 0x28;

pub const WATERFALL_MESH_IDS: [u32; LANES] = [40, 41, 42, 43];

pub const MESH_TIMEOUT_MS: u32 = 180_000;

pub const MESH_SCALE_XZ: f32 = 1.5;

pub const MESH_SCALE_Y: f32 = 2.3;

pub const LANE_LIFT: f32 = 6.0;

pub const LANE_STEP: f32 = 5.0;

pub const LANE_SPREAD: f32 = 0.5;

pub const LANE_SPREAD_MODULUS: u32 = 3;

pub const LANE_SPREAD_BIAS: i32 = 2;

pub const DESCENT_UNITS: f32 = 21.0;

pub const SPLASH_SPREAD_GROUND: f32 = 0.15;

pub const SPLASH_SPREAD_WATER: f32 = 0.1;

pub const SPLASH_RISE_GROUND: f32 = 0.0;

pub const SPLASH_RISE_WATER: f32 = 0.2;

pub const SPLASH_TEXTURE_GROUND: usize = 0x77;

pub const SPLASH_TEXTURE_WATER: usize = 0x97;

pub const SPLASH_GROWTH_GROUND: f32 = PLUME_GROWTH;

pub const SPLASH_GROWTH_WATER: f32 = 2.0e-4;

pub const SPLASH_COUNT: usize = 3;

pub const SPLASH_FIRST_INDEX: i32 = -1;

pub const SPLASH_LIFETIME_MS: u32 = 3_000;

pub const SPLASH_SCALE: Vec3 = Vec3::new(0.5, 1.8, 0.5);

pub const SPLASH_COLOUR_MODE: u32 = 3;

pub const FRAMES: u32 = 1;

pub const FRAME_MS: u32 = 0x50;

pub const SECTOR_KEYS: usize = 4;

pub const DUST_RECORD_KINDS: [(u32, i32, usize); 4] = [
    (0x0008, 0, 2_261),
    (0x0009, 2, 1_709),
    (0x000D, 1, 424),
    (0x0213, 0, 1_294),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Resolution {
    Present,
    RowWithoutFile,
    NoRow,
}

pub const CRST_BLOCK: std::ops::RangeInclusive<u32> = 3064..=3067;

pub fn resolve(
    list: &formats::mesh::mesh_list::MeshList,
    id: u32,
    has_file: impl Fn(&str) -> bool,
) -> Resolution {
    match list.get(id) {
        None => Resolution::NoRow,
        Some(path) if has_file(path) => Resolution::Present,
        Some(_) => Resolution::RowWithoutFile,
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Lane {
    pub at: Vec3,
    pub mesh_id: u32,
    pub splashed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Footing {
    Water,
    Ground,
}

impl Footing {
    pub fn splash_constants(self) -> (usize, f32, f32, f32) {
        match self {
            Footing::Water => (
                SPLASH_TEXTURE_WATER,
                SPLASH_GROWTH_WATER,
                SPLASH_SPREAD_WATER,
                SPLASH_RISE_WATER,
            ),
            Footing::Ground => (
                SPLASH_TEXTURE_GROUND,
                SPLASH_GROWTH_GROUND,
                SPLASH_SPREAD_GROUND,
                SPLASH_RISE_GROUND,
            ),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Unbuilt {
    SplashSound,
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct SplashEvent {
    pub slot: i64,
    pub at: Vec3,
}

fn water_quad(at: Vec3) -> super::timers::TimedQuad {
    super::timers::TimedQuad::new(
        super::quads::GroundQuad {
            owner: 0,
            slot: super::quads::Slot::Placed,
            texture: 150,
            pass: render::billboard::BillboardPass::Additive,
            anchor: at,
            base_scale: Vec3::splat(0.01),
            grow: 0.0015,
            spin_period: 0,
            ordinal: 0,
            argb: 0xAAAAAAAA,
        },
        1000,
    )
}

#[derive(Debug, Clone, PartialEq)]

pub struct Dust {
    pub at: Vec3,
    pub mode: i32,
    pub birth_ms: u32,
    pub period_ms: u32,
    pub seed: f32,
    pub lanes: [Lane; LANES],
    pub unregistered: bool,
    pending: Vec<Billboard>,
    water_quads: Vec<super::timers::TimedQuad>,
    pub splashes: Vec<SplashEvent>,
    pub unbuilt: Vec<Unbuilt>,
    rng: Lcg,
    detail: i32,
}

#[derive(Debug, Clone, PartialEq)]

pub struct Built {
    pub dust: Dust,
    pub children: Vec<Billboard>,
}

pub fn build(at: Vec3, plume_base_height: f32, mode: i32, now_ms: u32, rng: &mut Lcg) -> Built {
    let seed = (rng.next() % SEED_MODULUS) as f32 * SEED_STEP;

    let lifted_y = at.y + LANE_LIFT;

    let mut dust = Dust {
        at,
        mode,
        birth_ms: now_ms,
        period_ms: GROUND_PERIOD_MS,
        seed,
        lanes: [Lane {
            at: Vec3::ZERO,
            mesh_id: 0,
            splashed: false,
        }; LANES],
        unregistered: false,
        pending: Vec::new(),
        water_quads: Vec::new(),
        splashes: Vec::new(),
        unbuilt: Vec::new(),
        rng: *rng,
        detail: 0,
    };

    let mut children = Vec::new();

    match mode {
        0 => {
            let mut lifetime = PLUME_LIFETIME_MS;

            let mut i = 0i32;

            while lifetime < PLUME_LIFETIME_END_MS {
                let draw = rng.next() % PLUME_MODULUS;

                let wide = draw as f32 * PLUME_WIDTH_STEP + seed;

                let tall = draw as f32 * PLUME_HEIGHT_STEP + plume_base_height;

                let mut child = Billboard::new(
                    PLUME_TEXTURE,
                    lifetime,
                    Vec3::new(wide, tall, wide),
                    PLUME_GROWTH,
                    FRAMES,
                    FRAME_MS,
                );

                child.position = Vec3::new(
                    (draw as i32 - PLUME_BIAS) as f32 * PLUME_JITTER_X + at.x,
                    lifted_y,
                    at.z - (i + PLUME_BIAS) as f32 * PLUME_JITTER_Z,
                );

                child.origin = child.position;

                child.lift = false;

                child.motion = PLUME_MOTION;

                child.rise = i as f32 * PLUME_RISE_STEP + PLUME_RISE_BASE;

                children.push(child);

                i += 1;

                lifetime += PLUME_LIFETIME_STEP_MS;
            }
        }
        1 => {
            dust.period_ms = WATERFALL_PERIOD_MS;

            for (i, lane) in dust.lanes.iter_mut().enumerate() {
                lane.mesh_id = WATERFALL_MESH_BASE + i as u32;

                lane.splashed = false;

                let dx = lane_jitter(rng.next(), i);

                let dz = lane_jitter(rng.next(), i);

                lane.at = Vec3::new(
                    at.x + dx,
                    lifted_y + LANE_LIFT + i as f32 * LANE_STEP,
                    at.z + dz,
                );
            }
        }
        _ => {}
    }

    dust.rng = *rng;

    Built { dust, children }
}

impl Dust {
    pub fn t(&self, now_ms: u32) -> f32 {
        now_ms.wrapping_sub(self.birth_ms) as f32 / self.period_ms as f32
    }

    pub fn expiring(&self, now_ms: u32) -> bool {
        self.t(now_ms) > 1.0
    }

    pub fn lane_y(&self, i: usize, t: f32) -> f32 {
        self.at.y + LANE_LIFT + i as f32 * LANE_STEP - t * DESCENT_UNITS
    }

    pub fn lane_splashes(&self, i: usize, t: f32) -> bool {
        self.lane_y(i, t) < self.at.y && !self.lanes[i].splashed
    }

    pub fn lane_floor(&self, y: f32) -> f32 {
        y.max(self.at.y)
    }

    pub fn burst(&self, rng: &mut Lcg) -> Vec<Billboard> {
        let mut out = Vec::with_capacity(BURST_COUNT);

        let mut lifetime = BURST_LIFETIME_MS;

        while lifetime < BURST_LIFETIME_END_MS {
            let draw = rng.next() % PLUME_MODULUS;

            let wide = draw as f32 * PLUME_WIDTH_STEP + self.seed;

            let tall = draw as f32 * PLUME_HEIGHT_STEP + self.seed;

            let mut child = Billboard::new(
                PLUME_TEXTURE,
                lifetime,
                Vec3::new(wide, tall, wide),
                PLUME_GROWTH,
                FRAMES,
                FRAME_MS,
            );

            child.position = Vec3::new(self.at.x, self.at.y - BURST_DROP, self.at.z);

            child.origin = child.position;

            child.lift = true;

            out.push(child);

            lifetime += PLUME_LIFETIME_STEP_MS;
        }

        out
    }

    pub fn splash(&self, i: usize, footing: Footing) -> (Vec<Billboard>, Vec<Unbuilt>) {
        let (texture, growth, spread, rise) = footing.splash_constants();

        let lane = self.lanes[i].at;

        let mut out = Vec::with_capacity(SPLASH_COUNT);

        for j in SPLASH_FIRST_INDEX..SPLASH_FIRST_INDEX + SPLASH_COUNT as i32 {
            let mut child = Billboard::new(
                texture,
                SPLASH_LIFETIME_MS,
                SPLASH_SCALE,
                growth,
                FRAMES,
                FRAME_MS,
            );

            child.colour_mode = SPLASH_COLOUR_MODE;

            child.position = Vec3::new(
                lane.x + j as f32 * spread,
                self.at.y + i as f32 * rise,
                lane.z + j as f32 * spread,
            );

            child.origin = child.position;

            out.push(child);
        }

        (out, vec![Unbuilt::SplashSound])
    }

    pub fn tick_with(&mut self, now_ms: u32, rng: &mut Lcg, footing: impl Fn(usize) -> Footing) {
        if self.unregistered {
            return;
        }

        let t = self.t(now_ms);

        if t > 1.0 {
            if self.mode != 1 {
                self.pending.extend(self.burst(rng));
            }

            self.unregistered = true;

            return;
        }

        if self.mode != 1 {
            return;
        }

        for i in 0..LANES {
            let y = self.lane_y(i, t);

            self.lanes[i].at.y = y;

            if self.lane_splashes(i, t) {
                self.lanes[i].splashed = true;

                let footing = footing(i);

                let (children, unbuilt) = self.splash(i, footing);

                self.pending.extend(children);

                self.unbuilt.extend(unbuilt);

                let lane = self.lanes[i].at;

                self.splashes.push(SplashEvent {
                    slot: if footing == Footing::Water { 14 } else { 13 },
                    at: lane,
                });

                if footing == Footing::Water {
                    self.water_quads
                        .push(water_quad(Vec3::new(lane.x, self.at.y + 0.275, lane.z)));
                }
            }

            self.lanes[i].at.y = self.lane_floor(self.lanes[i].at.y);
        }
    }

    pub fn undrawn_lanes(&self) -> usize {
        0
    }

    fn tick_on(&mut self, now: u32, rng: &mut Lcg, ground: Option<&super::shade::Ground<'_>>) {
        let lanes = self.lanes;

        self.tick_with(now, rng, |i| {
            if ground
                .and_then(|g| g.water_height(lanes[i].at.x, lanes[i].at.z))
                .is_some()
            {
                Footing::Water
            } else {
                Footing::Ground
            }
        });

        for quad in &mut self.water_quads {
            let at = quad.quad.anchor;

            if let Some(height) = ground.and_then(|g| g.surface_height(at.x, at.z)) {
                quad.quad.anchor.y = height + 0.275;
            }
        }
    }
}

pub fn lane_jitter(draw: u32, lane: usize) -> f32 {
    ((draw % LANE_SPREAD_MODULUS) as i32 + lane as i32 - LANE_SPREAD_BIAS) as f32 * LANE_SPREAD
}

impl Effect for Dust {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let mut rng = self.rng;

        self.tick_with(tick.now_ms, &mut rng, |_| Footing::Ground);

        self.rng = rng;
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        self.detail = live.detail;

        self.tick_on(tick.now_ms, live.rng, live.ground);
    }

    fn needs_ground(&self) -> bool {
        self.mode == 1
    }

    fn update_grounded(&mut self, tick: &Tick, ground: &super::shade::Ground<'_>) {
        let mut rng = self.rng;

        self.tick_on(tick.now_ms, &mut rng, Some(ground));

        self.rng = rng;
    }

    fn expired(&self) -> bool {
        self.unregistered
    }

    fn submit(&self, _cx: &Submit<'_>, out: &mut Sink<'_>) {
        if self.mode != 1 || self.unregistered || self.detail == 1 {
            return;
        }

        use render::camera::Matrix4;

        for lane in self.lanes {
            let rotation = Matrix4::rotation_yaw_pitch_roll(0.0, -std::f32::consts::FRAC_PI_2, 0.0);

            let scale = Matrix4::scaling(MESH_SCALE_XZ, MESH_SCALE_Y, MESH_SCALE_XZ);

            let world = Matrix4::multiply(
                &Matrix4::multiply(&rotation, &scale),
                &Matrix4::translation(lane.at.x, lane.at.y, lane.at.z),
            );

            out.mesh.push(super::effect_tree::MeshSubmission::Authored(
                super::effect_tree::AuthoredMesh {
                    mesh_id: lane.mesh_id,
                    world,
                    material: super::effect_tree::MeshMaterial::Waterfall,
                    texture: None,
                },
            ));
        }
    }

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        let taken = std::mem::take(&mut self.pending);

        taken
            .into_iter()
            .map(|b| Box::new(b) as Box<dyn Effect>)
            .chain(
                std::mem::take(&mut self.water_quads)
                    .into_iter()
                    .map(|quad| Box::new(quad) as Box<dyn Effect>),
            )
            .collect()
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}

pub fn report(objects: &[Dust]) -> String {
    let lanes: usize = objects.iter().map(Dust::undrawn_lanes).sum();

    let sounds = objects
        .iter()
        .flat_map(|d| &d.unbuilt)
        .filter(|u| **u == Unbuilt::SplashSound)
        .count();

    format!(
        "NWYD: EffectDust — {} object(s), {lanes} waterfall lane(s) not drawn, \
         {sounds} sound cue(s) retained for audio",
        objects.len(),
    )
}

pub const SOURCE_PERIOD_MS: u32 = 10_000;

pub const SOURCE_BUBBLE_PERIOD_MS: u32 = 0x09C4;

pub const SOURCE_MODULUS: u32 = 5;

pub const SOURCE_BIAS: i32 = 3;

pub const SOURCE_STEP_X: f32 = 0.7;

pub const SOURCE_STEP_Z: f32 = 0.5;

pub const SOURCE_SCALE: f32 = 1.0;

pub const MODE_PLUME: i32 = 0;

pub const MODE_WATERFALL: i32 = 1;

pub const MODE_BUBBLE: i32 = 2;

pub const SOUND_DUST: i64 = 2;

pub const SOUND_BUBBLE: i64 = 7;

pub const BUBBLE_AUDIBLE_RANGE: f32 = 4.0;

pub const BUBBLE_KIND: i32 = 0;

pub const BUBBLE_COUNT: i32 = 6;

pub const BUBBLE_SPREAD: f32 = 5.0e-4;

pub const BUBBLE_ARGB: u32 = 0xAAAA_AAAA;

pub const BUBBLE_TEXTURE: usize = 0x78;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Source {
    pub at: Vec3,
    pub mode: i32,
    pub scale: f32,
    pub stamp_ms: u32,
}

#[derive(Debug, Clone, Default, PartialEq)]

pub struct Spawned {
    pub dust: Vec<Dust>,
    pub bubbles: Vec<super::burst::Particle>,
    pub billboards: Vec<Billboard>,
    pub sounds: Vec<i64>,
}

impl Source {
    pub fn new(at: Vec3, scale: f32, mode: i32) -> Source {
        Source {
            at,
            mode,
            scale,
            stamp_ms: 0,
        }
    }

    pub fn period_ms(&self) -> u32 {
        match self.mode == MODE_BUBBLE {
            true => SOURCE_BUBBLE_PERIOD_MS,
            false => SOURCE_PERIOD_MS,
        }
    }

    pub fn due(&self, now_ms: u32) -> bool {
        now_ms.wrapping_sub(self.stamp_ms) > self.period_ms()
    }

    pub fn tick(&mut self, now_ms: u32, rng: &mut Lcg, target: Option<Vec3>) -> Spawned {
        let mut out = Spawned::default();

        if !self.due(now_ms) {
            return out;
        }

        let n = (rng.next() % SOURCE_MODULUS) as i32;

        if n % 2 == 1 {
            if self.mode < MODE_BUBBLE {
                out.sounds.push(SOUND_DUST);

                for k in (n - SOURCE_BIAS)..(n - SOURCE_BIAS + n) {
                    let at = Vec3::new(
                        SOURCE_STEP_X * k as f32 + self.at.x,
                        self.at.y,
                        SOURCE_STEP_Z * k as f32 + self.at.z,
                    );

                    let built = build(at, self.scale, self.mode, now_ms, rng);

                    out.billboards.extend(built.children);

                    out.dust.push(built.dust);
                }
            } else if self.mode == MODE_BUBBLE {
                let (particle, children) = super::burst::build(&self.bubble(), now_ms, rng);

                out.billboards.extend(children);

                out.bubbles.push(particle);

                if target.is_some_and(|t| self.audible_from(t)) {
                    out.sounds.push(SOUND_BUBBLE);
                }
            }
        }

        self.stamp_ms = now_ms;

        out
    }

    pub fn bubble(&self) -> super::burst::Recipe {
        super::burst::Recipe {
            at: self.at,
            kind: BUBBLE_KIND,
            count: BUBBLE_COUNT,
            spread: BUBBLE_SPREAD,
            argb: BUBBLE_ARGB,
            random_colour: false,
            texture: BUBBLE_TEXTURE,
            ..super::burst::Recipe::MEASURED_TAIL
        }
    }

    pub fn audible_from(&self, target: Vec3) -> bool {
        (self.at.x - target.x).hypot(self.at.z - target.z) < BUBBLE_AUDIBLE_RANGE
    }
}

pub fn build_sources(records: &[ObjRecord], zone: (u8, u8)) -> Vec<Source> {
    let mut out = Vec::new();

    for record in records {
        if record.class() != ObjectClass::Dust {
            continue;
        }

        let Some((_, mode, _)) = DUST_RECORD_KINDS.iter().find(|(k, _, _)| *k == record.kind)
        else {
            continue;
        };

        let world = record.world_position(u32::from(zone.0), u32::from(zone.1));

        let scale = match record.kind {
            0x0213 => record.extra.map_or(SOURCE_SCALE, |e| e[1]),
            _ => SOURCE_SCALE,
        };

        out.push(Source::new(
            Vec3::new(world[0], world[1], world[2]),
            scale,
            *mode,
        ));
    }

    out
}
