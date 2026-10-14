use super::beam::Beam;
use super::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use super::rng::Lcg;
use super::shade::{Ground, Shade};
use glam::Vec3;
use render::billboard::{BillboardDraw, BillboardPass, BillboardUniforms};
use render::camera::Matrix4;

pub const SEGMENTS: usize = 5;

pub const CHILD_TEXTURE: usize = 0x80;

pub const SHADE_GRID: i32 = 3;

pub const SHADE_TEXTURE: usize = 7;

pub const SOUND: i64 = 0xB2;

pub const PARENT_LIFETIME_LEAD_MS: u32 = 100;

pub const BONE_SPARK_LIFETIME_MS: u32 = 900;

pub const BONE_SPARK_CHILD_LIFETIME_MS: u32 = 1000;

pub const WIDTH_TO_HALF_EXTENT: f32 = 0.8;

pub const AMPLITUDE_STEP: f32 = 0.15;

pub const AMPLITUDE_THRESHOLD: f32 = 0.3;

pub const AMPLITUDE_CLAMPED: f32 = 0.5;

pub const JITTER_DEFAULT: f32 = 1.0;

pub const TRAIL_JITTER: f32 = 0.13;

pub const TARGET_LIFT: f64 = 1.0;

pub fn amplitude(count: usize, i: usize) -> f32 {
    let amp = (count as f32 - i as f32) * AMPLITUDE_STEP;

    if AMPLITUDE_THRESHOLD < amp {
        AMPLITUDE_CLAMPED
    } else {
        amp
    }
}

pub fn kink(rng: &mut Lcg, amp: f32, jitter: f32) -> Vec3 {
    let draw = |rng: &mut Lcg| (rng.next_mod(9) as i32 - 5) as f32 * amp * jitter;

    let x = draw(rng);

    let y = draw(rng);

    let z = draw(rng);

    Vec3::new(x, y, z)
}

fn lerp_weighted(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    let u = 1.0 - t;

    Vec3::new(a.x * u + b.x * t, a.y * u + b.y * t, u * a.z + b.z * t)
}

fn lerp_delta(a: Vec3, b: Vec3, t: f32) -> Vec3 {
    Vec3::new(
        t * (b.x - a.x) + a.x,
        (b.y - a.y) * t + a.y,
        a.z + (b.z - a.z) * t,
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrackedBody {
    pub entity: u16,
    pub position: Vec3,
}

#[derive(Debug, Clone)]
pub struct Spark {
    pub source: Vec3,
    pub dest: Vec3,
    pub target: Option<u16>,
    pub target_key: Option<super::effect_context::ActorKey>,
    pub argb: u32,
    pub shade_argb: u32,
    pub lifetime_ms: u32,
    pub birth_ms: u32,
    pub width_scale: f32,
    pub jitter: f32,
    pub segments: usize,
    pub beams: Vec<Beam>,
    pub shades: Vec<Shade>,
    rng: Lcg,
    gone: bool,
}

impl Spark {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source: Vec3,
        target: Option<TrackedBody>,
        dest: Vec3,
        argb: u32,
        shade_argb: u32,
        lifetime_ms: u32,
        width_scale: f32,
        initial_t: f32,
        now_ms: u32,
        seed: u32,
    ) -> Spark {
        let dest = match target {
            Some(target) => Vec3::new(
                target.position.x,
                (f64::from(target.position.y) + TARGET_LIFT) as f32,
                target.position.z,
            ),
            None => dest,
        };

        let mut spark = Spark {
            source,
            dest,
            target: target.map(|target| target.entity),
            target_key: None,
            argb,
            shade_argb,
            lifetime_ms: lifetime_ms.wrapping_sub(PARENT_LIFETIME_LEAD_MS),
            birth_ms: now_ms,
            width_scale,
            jitter: JITTER_DEFAULT,
            segments: SEGMENTS,
            beams: Vec::with_capacity(SEGMENTS),
            shades: Vec::with_capacity(if target.is_some() { SEGMENTS } else { 0 }),
            rng: Lcg::new(seed),
            gone: false,
        };

        let half = width_scale * WIDTH_TO_HALF_EXTENT;

        let mut prev = source;

        let mut next = lerp_weighted(source, dest, initial_t);

        for i in 0..SEGMENTS {
            spark.beams.push(
                Beam::new(
                    prev,
                    next,
                    CHILD_TEXTURE,
                    argb,
                    lifetime_ms,
                    half,
                    half,
                    now_ms,
                )
                .additive(),
            );

            if target.is_some() {
                let mut shade = Shade::new(
                    SHADE_GRID,
                    SHADE_TEXTURE,
                    shade_argb,
                    next,
                    super::effects::SECTOR_UNASSIGNED,
                )
                .with_lifetime(lifetime_ms);

                shade.birth_ms = Some(now_ms);

                spark.shades.push(shade);
            }

            prev = next;

            next = lerp_weighted(source, dest, (i + 1) as f32 / SEGMENTS as f32);
        }

        spark
    }

    pub fn place_constructor_shades(&mut self, ground: &Ground<'_>) {
        for shade in &mut self.shades {
            shade.place(ground);
        }
    }

    pub fn with_jitter(mut self, jitter: f32) -> Spark {
        self.jitter = jitter;

        self
    }

    pub fn track(&mut self, body: Vec3) {
        self.dest = Vec3::new(body.x, (f64::from(body.y) + TARGET_LIFT) as f32, body.z);
    }

    pub fn set_lifetime(&mut self, lifetime_ms: u32) {
        self.lifetime_ms = lifetime_ms;

        for beam in &mut self.beams {
            beam.lifetime_ms = lifetime_ms;
        }
    }

    pub fn joints(&mut self) -> Vec<Vec3> {
        let (source, dest, segments, jitter) = (self.source, self.dest, self.segments, self.jitter);

        (0..segments)
            .map(|i| {
                let t = (i + 1) as f32 / segments as f32;

                let mut joint = lerp_weighted(source, dest, t);

                if i + 1 < segments {
                    joint += kink(&mut self.rng, amplitude(segments, i), jitter);
                }

                joint
            })
            .collect()
    }
}

pub const CLASS: Class = Class {
    name: "EffectSpark",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

impl Effect for Spark {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn tracked_entity(&self) -> Option<u16> {
        self.target.filter(|_| self.target_key.is_none())
    }

    fn resolve_tracked_entity(&mut self, position: Option<Vec3>) {
        match position {
            Some(position) => self.track(position),
            None => self.target = None,
        }
    }

    fn needs_ground(&self) -> bool {
        !self.shades.is_empty()
    }

    fn update(&mut self, tick: &Tick) {
        self.update_inner(tick, None);
    }

    fn update_grounded(&mut self, tick: &Tick, ground: &Ground<'_>) {
        self.update_inner(tick, Some(ground));
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        if let Some(key) = self.target_key {
            match (live.body)(key) {
                Some(body) => self.track(body.position),
                None => {
                    self.target_key = None;

                    self.target = None;
                }
            }
        }

        self.update_inner(tick, live.ground);
    }

    fn expired(&self) -> bool {
        self.gone && self.beams.iter().all(Beam::expired) && self.shades.iter().all(Effect::expired)
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        if cx.scene.detail == 1 {
            return;
        }

        let live: Vec<&Beam> = self.beams.iter().filter(|b| b.drawn()).collect();

        out.effect.culled += self.beams.len() - live.len();

        if !live.is_empty() {
            match cx.scene.placement(CHILD_TEXTURE) {
                Some((array, layer)) => {
                    let object = out.effect.objects.len() as u32;

                    out.effect.objects.push(
                        BillboardUniforms::new(Matrix4::IDENTITY, BillboardPass::Additive)
                            .with_layer(layer)
                            .with_opaque(cx.scene.alpha_at(CHILD_TEXTURE)),
                    );

                    let base_vertex = out.effect.vertices.len() as u32;

                    let first = out.effect.indices.len() as u32;

                    for (n, beam) in live.iter().enumerate() {
                        let bias = (n * super::beam::BEAM_VERTICES) as u16;

                        out.effect.vertices.extend_from_slice(&beam.vertices);

                        out.effect
                            .indices
                            .extend(super::beam::FAN_INDICES.map(|i| i + bias));

                        out.effect.submitted(&super::beam::CLASS);
                    }

                    out.effect.draws.push(BillboardDraw {
                        pass: BillboardPass::Additive,
                        object,
                        texture: array,
                        base_vertex,
                        indices: first..out.effect.indices.len() as u32,
                    });
                }
                None => out.effect.unbound += live.len(),
            }
        }

        for shade in &self.shades {
            Effect::submit(shade, cx, out);
        }
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        match self.gone {
            false => Cull::Kept,
            true => Cull::Rejected,
        }
    }
}

impl Spark {
    fn update_inner(&mut self, tick: &Tick, ground: Option<&Ground<'_>>) {
        let now = tick.now_ms;

        if self.lifetime_ms != 0 && self.lifetime_ms < now.wrapping_sub(self.birth_ms) {
            self.gone = true;
        }

        if !self.gone {
            let mut prev = self.source;

            for (i, joint) in self.joints().into_iter().enumerate() {
                if let Some(beam) = self.beams.get_mut(i) {
                    beam.place(prev, joint);
                }

                if let (Some(shade), Some(ground)) = (self.shades.get_mut(i), ground)
                    && (self.target.is_some() || self.target_key.is_some())
                {
                    shade.centre =
                        Vec3::new((prev.x + joint.x) * 0.5, 0.0, (prev.z + joint.z) * 0.5);

                    shade.place(ground);
                }

                prev = joint;
            }
        }

        for beam in &mut self.beams {
            beam.tick(now);
        }

        for shade in &mut self.shades {
            Effect::update(shade, tick);
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BoneSpark {
    pub owner_key: Option<super::effect_context::ActorKey>,
    pub bone_id: i32,
    pub near_local: Vec3,
    pub far_local: Vec3,
    pub argb: u32,
    pub dead_argb: u32,
    pub width_scale: f32,
    pub jitter: f32,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub bone: Option<Matrix4>,
    pub beams: Vec<Beam>,
    rng: Lcg,
    gone: bool,
}

impl BoneSpark {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        bone_id: i32,
        near_len: f32,
        far_len: f32,
        argb: u32,
        dead_argb: u32,
        width_scale: f32,
        now_ms: u32,
        seed: u32,
    ) -> BoneSpark {
        let near_local = Vec3::new(0.0, 0.0, -near_len);

        let far_local = Vec3::new(0.0, 0.0, far_len);

        let half = width_scale * WIDTH_TO_HALF_EXTENT;

        let beams = (0..SEGMENTS)
            .map(|_| {
                Beam::new(
                    near_local,
                    far_local,
                    CHILD_TEXTURE,
                    argb,
                    BONE_SPARK_CHILD_LIFETIME_MS,
                    half,
                    half,
                    now_ms,
                )
                .additive()
            })
            .collect();

        BoneSpark {
            owner_key: None,
            bone_id,
            near_local,
            far_local,
            argb,
            dead_argb,
            width_scale,
            jitter: JITTER_DEFAULT,
            birth_ms: now_ms,
            lifetime_ms: BONE_SPARK_LIFETIME_MS,
            bone: None,
            beams,
            rng: Lcg::new(seed),
            gone: false,
        }
    }

    pub fn with_jitter(mut self, jitter: f32) -> BoneSpark {
        self.jitter = jitter;

        self
    }

    pub fn set_bone(&mut self, bone: Option<Matrix4>) {
        self.bone = bone;
    }

    pub fn joints(&mut self, bone: &Matrix4) -> Vec<Vec3> {
        let start = bone.transform_coord(self.near_local);

        let end = bone.transform_coord(self.far_local);

        let jitter = self.jitter;

        (0..SEGMENTS)
            .map(|i| {
                let t = ((i + 1) as f64 / SEGMENTS as f64) as f32;

                let mut joint = lerp_delta(start, end, t);

                if i < SEGMENTS - 1 {
                    joint += kink(&mut self.rng, amplitude(SEGMENTS, i), jitter);
                }

                joint
            })
            .collect()
    }
}

pub const BONE_SPARK_CLASS: Class = Class {
    name: "BoneSpark",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

impl Effect for BoneSpark {
    fn class(&self) -> &'static Class {
        &BONE_SPARK_CLASS
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        if let Some(key) = self.owner_key {
            if (live.body)(key).is_none() {
                self.owner_key = None;

                self.bone = None;
            } else {
                self.bone = usize::try_from(self.bone_id)
                    .ok()
                    .and_then(|bone| (live.bone)(key, bone));
            }
        }

        self.rng = *live.rng;

        self.update(tick);

        *live.rng = self.rng;
    }

    fn update(&mut self, tick: &Tick) {
        let now = tick.now_ms;

        if self.lifetime_ms != 0 && self.lifetime_ms < now.wrapping_sub(self.birth_ms) {
            self.gone = true;
        }

        if !self.gone
            && let Some(bone) = self.bone
        {
            let start = bone.transform_coord(self.near_local);

            let mut prev = start;

            for (i, joint) in self.joints(&bone).into_iter().enumerate() {
                if let Some(beam) = self.beams.get_mut(i) {
                    beam.place(prev, joint);
                }

                prev = joint;
            }
        }

        for beam in &mut self.beams {
            beam.tick(now);
        }
    }

    fn expired(&self) -> bool {
        self.gone && self.beams.iter().all(Beam::expired)
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        if cx.scene.detail == 1 {
            return;
        }

        let live: Vec<&Beam> = self.beams.iter().filter(|b| b.drawn()).collect();

        out.effect.culled += self.beams.len() - live.len();

        if live.is_empty() {
            return;
        }

        let Some((array, layer)) = cx.scene.placement(CHILD_TEXTURE) else {
            out.effect.unbound += live.len();

            return;
        };

        let object = out.effect.objects.len() as u32;

        out.effect.objects.push(
            BillboardUniforms::new(Matrix4::IDENTITY, BillboardPass::Additive)
                .with_layer(layer)
                .with_opaque(cx.scene.alpha_at(CHILD_TEXTURE)),
        );

        let base_vertex = out.effect.vertices.len() as u32;

        let first = out.effect.indices.len() as u32;

        for (n, beam) in live.iter().enumerate() {
            let bias = (n * super::beam::BEAM_VERTICES) as u16;

            out.effect.vertices.extend_from_slice(&beam.vertices);

            out.effect
                .indices
                .extend(super::beam::FAN_INDICES.map(|i| i + bias));

            out.effect.submitted(&super::beam::CLASS);
        }

        out.effect.draws.push(BillboardDraw {
            pass: BillboardPass::Additive,
            object,
            texture: array,
            base_vertex,
            indices: first..out.effect.indices.len() as u32,
        });
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        match self.expired() {
            false => Cull::Kept,
            true => Cull::Rejected,
        }
    }
}
