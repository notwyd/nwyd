use super::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use glam::Vec3;

pub mod kind {

    pub const BOW: i32 = 0x68;

    pub const BOLT: i32 = 0x69;

    pub const DART: i32 = 0x97;

    pub const SHOT: i32 = 0x98;

    pub const SHAFT: i32 = 0x99;

    pub const INVISIBLE: i32 = 10000;

    pub const BOLT_BLUE: i32 = 0x2711;

    pub const SWARM: i32 = 0x2712;

    pub const SWARM_WIDE: i32 = 0x2713;
}

pub const MS_PER_UNIT: [(i32, i32); 7] = [
    (kind::BOW, 0x96),
    (kind::BOLT, 100),
    (kind::DART, 0x32),
    (kind::SHOT, 0x32),
    (kind::SHAFT, 0x50),
    (kind::INVISIBLE, 0x32),
    (kind::BOLT_BLUE, 100),
];

pub const SHOT_SUBMODE2_MS_PER_UNIT: i32 = 0x46;

pub const LIFETIME_MS: std::ops::RangeInclusive<i32> = 1..=5000;

pub const SHAFT_MODELS: [(i32, i32); 24] = [
    (0, 0x369),
    (1, 0x36A),
    (2, 0x36B),
    (3, 0x36C),
    (4, 0x36D),
    (5, 0x37C),
    (6, 0x38B),
    (7, 0x38C),
    (8, 0x38D),
    (9, 0x25),
    (10, 0x2FF),
    (11, 0xAFE),
    (12, 0xB54),
    (13, 0xB75),
    (14, 0xB7E),
    (15, 0xB89),
    (16, 0xB94),
    (17, 0xB9F),
    (18, 0xBAD),
    (19, 3000),
    (20, 0xBC3),
    (21, 0xBCE),
    (22, 0xBFD),
    (99, 0xB69),
];

pub const SHOT_MODELS: [(i32, i32); 3] = [(0, 0x367), (1, 0x368), (2, 0x35F)];

pub const PLANE_MESH_ID: i32 = 0x1C;

pub const PLANE_SCALE_DART: f32 = 1.3;

pub const PLANE_SCALE_SHOT: f32 = 2.0;

pub const PLANE_COLOUR_SHOT: u32 = 0xFF00_1020;

pub const DART_COLOURS: [u32; 3] = [0x0088_5522, 0x0000_AA00, 0x00AA_00FF];

pub const TRAIL_TEXTURE: usize = 0x19A;

pub const TRAIL_LIFETIME_MS: u32 = 1000;

pub const TRAIL_COLOURS: [(i32, u32); 5] = [
    (0, 0x0077_7777),
    (5, 0x0022_2288),
    (6, 0x0033_8843),
    (7, 0x0088_4388),
    (8, 0x0088_3333),
];

pub const TRAIL_COLOURS_BY_KIND: [(i32, u32); 2] = [
    (kind::INVISIBLE, 0xFFAA_AAFF),
    (kind::BOLT_BLUE, 0xFF77_77FF),
];

pub const TRAIL_WIDTHS: [(i32, f32); 5] = [
    (0, 0.08),
    (kind::SHOT, 0.1),
    (kind::SHAFT, 0.4),
    (kind::INVISIBLE, 0.05),
    (kind::BOLT_BLUE, 0.5),
];

pub const TRAIL_KINDS: [i32; 5] = [
    kind::DART,
    kind::SHOT,
    kind::SHAFT,
    kind::INVISIBLE,
    kind::BOLT_BLUE,
];

pub const TRAIL_DROP: f32 = 0.1;

pub const TRAIL_START_T: f32 = 0.2;

pub const TRAIL_GRACE_MS: u32 = 2000;

pub const SPARK_LIFETIME_MS: u32 = 1000;

pub const SPARK_GROWTH: f32 = 0.001;

pub const SPARK_FRAMES: u32 = 1;

pub const SPARK_FRAME_MS: u32 = 0x50;

pub const SPARK_SIZE_STEP: f32 = 0.2;

pub const SPARK_SIZE_BASE: f32 = 0.2;

pub const SPARK_DROP: f32 = 0.5;

pub const SPARK_COLOURS: [(i32, u32); 3] = [(0, 0xFFAA_AAEE), (1, 0xFFFF_AA00), (2, 0xFF00_AA00)];

pub const SECOND_SPARK_MODEL: i32 = 0x397;

pub const SECOND_SPARK_KIND: usize = 0xB;

pub const SECOND_SPARK_COLOUR: u32 = 0xFFAA_8855;

pub const SECOND_SPARK_SIZE_STEP: f32 = 0.04;

pub const SECOND_SPARK_SIZE_BASE: f32 = 0.3;

pub const PLANE_FRAME_PERIOD_MS: u32 = 0x140;

pub const PLANE_FRAME_STEP_MS: u32 = 0x50;

pub const PLANE_FRAME_BIAS_SHOT: i32 = 0x65;

pub const PLANE_FRAME_BIAS: i32 = 0xB;

pub const SHAFT_SPIN_PERIOD_MS: u32 = 300;

pub const SHOT_WOBBLE: f32 = 0.1;

pub const SWARM_LIFETIME_MS: i32 = 1000;

pub const SWARM_WIDE_LIFETIME_MS: i32 = 800;

pub const SWARM_WIDE_MODEL: i32 = 800;

pub const DART_MODEL: i32 = 0x320;

pub const BOW_MODEL: i32 = 0x36F;

pub const BOLT_MODELS: [(i32, i32); 2] = [(0, 0x375), (1, 0x397)];

pub const BOLT_BLUE_MODEL: i32 = 0xB18;

pub const VARIANT_FORCED_5: [i32; 3] = [kind::BOLT_BLUE, kind::SWARM, kind::SWARM_WIDE];

pub const VARIANTS: [i32; 4] = [5, 6, 7, 8];

pub const VARIANT_TEXTURES: [(i32, i32); 4] = [(5, 0x99), (6, 0xA6), (7, 0xF6), (8, 0x104)];

pub const DRAW_PITCH_DEFAULT: f32 = std::f32::consts::FRAC_PI_2;

pub const DRAW_PITCH_BOLT_BLUE: f32 = -0.698_131_74;

pub const DRAW_ROLL_BOLT_BLUE: f32 = std::f32::consts::FRAC_PI_4;

pub const DRAW_SCALE_SHAFT: f32 = 2.0;

pub const CLASS: Class = Class {
    name: "Arrow",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

pub fn model_id(kind: i32, submode: i32) -> Option<i32> {
    let pick = |table: &[(i32, i32)]| table.iter().find(|(k, _)| *k == submode).map(|(_, v)| *v);

    match kind {
        kind::BOW => Some(BOW_MODEL),
        kind::BOLT => pick(&BOLT_MODELS),
        kind::DART => Some(DART_MODEL),
        kind::SHOT => pick(&SHOT_MODELS),
        kind::SHAFT => pick(&SHAFT_MODELS),
        kind::BOLT_BLUE | kind::SWARM => Some(BOLT_BLUE_MODEL),
        kind::SWARM_WIDE => Some(SWARM_WIDE_MODEL),
        kind::INVISIBLE => None,
        _ => None,
    }
}

pub fn lifetime_ms(kind: i32, submode: i32, distance: f32) -> i32 {
    match kind {
        kind::SWARM => SWARM_LIFETIME_MS,
        kind::SWARM_WIDE => SWARM_WIDE_LIFETIME_MS,
        _ => {
            let per_unit = match (kind, submode) {
                (kind::SHOT, 2) => SHOT_SUBMODE2_MS_PER_UNIT,
                _ => match MS_PER_UNIT.iter().find(|(k, _)| *k == kind) {
                    Some((_, ms)) => *ms,
                    None => 1,
                },
            };

            let scaled = f64::from(distance.trunc()) * f64::from(per_unit);

            scaled.clamp(f64::from(i32::MIN), f64::from(i32::MAX)) as i32
        }
    }
    .clamp(*LIFETIME_MS.start(), *LIFETIME_MS.end())
}

pub fn plane_colour(submode: i32, distance: f32) -> u32 {
    match submode.rem_euclid(3) {
        0 => distance.to_bits(),
        i => DART_COLOURS[i as usize - 1],
    }
}

pub fn trail_colour(kind: i32, variant: i32) -> u32 {
    if let Some((_, argb)) = TRAIL_COLOURS_BY_KIND.iter().find(|(k, _)| *k == kind) {
        return *argb;
    }

    match TRAIL_COLOURS.iter().find(|(v, _)| *v == variant) {
        Some((_, argb)) => *argb,
        None => TRAIL_COLOURS[0].1,
    }
}

pub fn trail_width(kind: i32) -> f32 {
    match TRAIL_WIDTHS.iter().find(|(k, _)| *k == kind) {
        Some((_, w)) => *w,
        None => TRAIL_WIDTHS[0].1,
    }
}

#[derive(Debug)]

pub struct Arrow {
    pub source: Vec3,
    pub dest: Vec3,
    pub position: Vec3,
    pub kind: i32,
    pub submode: i32,
    pub model: Option<i32>,
    pub lifetime_ms: i32,
    pub birth_ms: u32,
    pub variant: i32,
    pub owner: u32,
    pub flag: u8,
    pub yaw: f32,
    pub pitch: f32,
    pub visible: bool,
    pub distance: f32,
    pub plane: Option<super::effect_mesh::EffectMesh>,
    pub trail: Option<super::beam::Beam>,
    pub swarm: [f32; 6],
    gone: bool,
    spawned: Vec<Box<dyn Effect>>,
    sounds: Vec<i64>,
    critical: bool,
    hits: Vec<(u16, u32)>,
}

impl Arrow {
    #[allow(clippy::too_many_arguments)]

    pub fn new(
        source: Vec3,
        dest: Vec3,
        submode: i32,
        kind: i32,
        flag: u8,
        variant: i32,
        owner: u32,
        now_ms: u32,
    ) -> Arrow {
        let distance = (dest - source).length();

        let variant = match VARIANT_FORCED_5.contains(&kind) {
            true => 5,
            false => variant,
        };

        let variant = match VARIANTS.contains(&variant) {
            true => variant,
            false => 0,
        };

        let mut arrow = Arrow {
            source,
            dest,
            position: source,
            kind,
            submode,
            model: model_id(kind, submode),
            lifetime_ms: lifetime_ms(kind, submode, distance),
            birth_ms: now_ms,
            variant,
            owner,
            flag,
            yaw: 0.0,
            pitch: 0.0,
            visible: false,
            distance,
            plane: None,
            trail: None,
            swarm: [0.0; 6],
            gone: false,
            spawned: Vec::new(),
            sounds: Vec::new(),
            critical: false,
            hits: Vec::new(),
        };

        arrow.plane = arrow.build_plane(now_ms);

        arrow.trail = arrow.build_trail(now_ms);

        arrow
    }

    fn build_plane(&self, now_ms: u32) -> Option<super::effect_mesh::EffectMesh> {
        let (argb, scale) = match (self.kind, self.submode) {
            (kind::DART, s) if s > 0 => (plane_colour(s, self.distance), PLANE_SCALE_DART),
            (kind::SHOT, 2) => (PLANE_COLOUR_SHOT, PLANE_SCALE_SHOT),
            _ => return None,
        };

        let mut mesh = super::effect_mesh::EffectMesh::new(PLANE_MESH_ID, argb, 0.0, 1, now_ms);

        mesh.scale_xz = scale;

        mesh.scale_y = scale;

        Some(mesh)
    }

    fn build_trail(&self, now_ms: u32) -> Option<super::beam::Beam> {
        if !TRAIL_KINDS.contains(&self.kind) {
            return None;
        }

        let width = trail_width(self.kind);

        let far = Vec3::new(self.source.x, self.source.y - TRAIL_DROP, self.source.z);

        let mut trail = super::beam::Beam::new(
            self.source,
            far,
            TRAIL_TEXTURE,
            trail_colour(self.kind, self.variant),
            TRAIL_LIFETIME_MS,
            width,
            width,
            now_ms,
        )
        .additive();

        trail.lifetime_ms = self.lifetime_ms.max(0) as u32 + TRAIL_GRACE_MS;

        Some(trail)
    }

    pub fn progress(&self, now_ms: u32) -> f32 {
        if (now_ms.wrapping_sub(self.birth_ms) as i32) < 0 {
            return 0.0;
        }

        let age = f64::from(now_ms.wrapping_sub(self.birth_ms));

        let life = f64::from(self.lifetime_ms as u32);

        match life {
            0.0 => 0.0,
            _ => (age / life) as f32,
        }
    }

    pub fn draw(&self) -> Option<(Vec3, f32, f32, f32, f32)> {
        let half = std::f32::consts::FRAC_PI_2;

        let (yaw, pitch, roll, scale) = match self.kind {
            kind::SHAFT => (self.yaw, half, 0.0, DRAW_SCALE_SHAFT),
            kind::DART => (self.yaw, self.pitch - half, 0.0, 1.0),
            kind::BOW | kind::SHOT => (self.yaw - half, half, 0.0, 1.0),
            kind::BOLT_BLUE => (
                self.yaw - half,
                DRAW_PITCH_BOLT_BLUE,
                DRAW_ROLL_BOLT_BLUE,
                1.0,
            ),
            kind::INVISIBLE => return None,
            kind::SWARM | kind::SWARM_WIDE => return None,
            _ => (self.yaw, half, 0.0, 1.0),
        };

        Some((self.position, yaw, pitch, roll, scale))
    }

    pub fn spark_colour(&self) -> u32 {
        if self.kind != kind::DART {
            return SPARK_COLOURS[0].1;
        }

        match SPARK_COLOURS.iter().find(|(s, _)| *s == self.submode) {
            Some((_, argb)) => *argb,
            None => SPARK_COLOURS[0].1,
        }
    }

    pub fn fly(&mut self, now_ms: u32, rng: &mut super::rng::Lcg) {
        self.fly_grounded(now_ms, rng, None);
    }

    fn fly_grounded(
        &mut self,
        now_ms: u32,
        rng: &mut super::rng::Lcg,
        ground: Option<&super::shade::Ground<'_>>,
    ) {
        if self.gone {
            return;
        }

        if self.model.is_none() && self.kind != kind::INVISIBLE {
            self.gone = true;

            return;
        }

        self.visible = true;

        let t = self.progress(now_ms);

        if matches!(self.kind, kind::SWARM | kind::SWARM_WIDE) {
            let radius = if self.kind == kind::SWARM { 1.5 } else { 1.9 } - t;

            for i in 0..3 {
                let angle = (t - i as f32 * 0.5) * 4.0;

                self.swarm[i * 2] = angle.sin() * radius;

                self.swarm[i * 2 + 1] = angle.cos() * radius;
            }

            if t > 1.0 {
                self.impact(now_ms, ground, rng);

                self.gone = true;

                self.visible = false;
            }

            return;
        }

        self.angles(now_ms);

        if t >= 1.0 {
            self.plane = None;

            if let Some(mut trail) = self.trail.take() {
                if now_ms.wrapping_sub(self.birth_ms) < self.lifetime_ms as u32 + TRAIL_GRACE_MS {
                    trail.lifetime_ms = now_ms.wrapping_sub(trail.birth_ms).wrapping_add(100);

                    trail.collapse_from_ms = now_ms;
                }

                trail.tick(now_ms);

                self.spawned.push(Box::new(trail));
            }

            self.impact(now_ms, ground, rng);

            self.gone = true;

            self.visible = false;

            return;
        }

        let inv = 1.0 - t;

        self.position = self.source * inv + self.dest * t;

        if self.kind == kind::SHOT && self.submode == 2 {
            self.position.y += (4.0 * std::f32::consts::PI * t).sin() * SHOT_WOBBLE;
        }

        self.drive_plane(now_ms);

        self.emit_sparks(t, rng);

        self.drive_trail(t, now_ms);

        if let Some(trail) = &mut self.trail {
            trail.tick(now_ms);
        }
    }

    fn angles(&mut self, now_ms: u32) {
        let d = self.dest - self.source;

        self.yaw = d.x.atan2(d.z) + std::f32::consts::FRAC_PI_2;

        self.pitch = d.y.atan2(Vec3::new(d.x, 0.0, d.z).length());

        if self.kind == kind::SHAFT {
            let phase = (now_ms % SHAFT_SPIN_PERIOD_MS) as f32 / SHAFT_SPIN_PERIOD_MS as f32;

            self.yaw = 2.0 * phase * std::f32::consts::PI;
        }
    }

    fn drive_plane(&mut self, now_ms: u32) {
        let (yaw, position) = (self.yaw + std::f32::consts::PI, self.position);

        let bias = match self.kind == kind::SHOT {
            true => PLANE_FRAME_BIAS_SHOT,
            false => PLANE_FRAME_BIAS,
        };

        let frame = (now_ms % PLANE_FRAME_PERIOD_MS) / PLANE_FRAME_STEP_MS;

        if let Some(plane) = &mut self.plane {
            plane.yaw = yaw;

            plane.position = position;

            plane.texture_bias = frame as i32 + bias;

            plane.tick(now_ms);
        }
    }

    fn emit_sparks(&mut self, t: f32, rng: &mut super::rng::Lcg) {
        if let Some(spark) = self.spark(rng.next()) {
            self.spawned.push(Box::new(spark));
        }

        if self.model == Some(SECOND_SPARK_MODEL) && t > TRAIL_START_T {
            let spark = self.second_spark(rng.next());

            self.spawned.push(Box::new(spark));
        }
    }

    pub fn spark(&self, roll: u32) -> Option<super::effects::Billboard> {
        if self.kind == kind::INVISIBLE {
            return None;
        }

        let size = (roll % 5) as f32 * SPARK_SIZE_STEP + SPARK_SIZE_BASE;

        let mut spark = super::effects::Billboard::new(
            0,
            SPARK_LIFETIME_MS,
            Vec3::new(size, size, size),
            SPARK_GROWTH,
            SPARK_FRAMES,
            SPARK_FRAME_MS,
        );

        spark.set_color(self.spark_colour());

        spark.position = Vec3::new(
            self.position.x,
            self.position.y - SPARK_DROP,
            self.position.z,
        );

        spark.pass = render::billboard::BillboardPass::Additive;

        spark.lift = true;

        Some(spark)
    }

    pub fn second_spark(&self, roll: u32) -> super::effects::Billboard {
        let size = (roll % 5) as f32 * SECOND_SPARK_SIZE_STEP + SECOND_SPARK_SIZE_BASE;

        let mut spark = super::effects::Billboard::new(
            SECOND_SPARK_KIND,
            SPARK_LIFETIME_MS,
            Vec3::new(size, size, size),
            SPARK_GROWTH,
            SPARK_FRAMES,
            SPARK_FRAME_MS,
        );

        spark.set_color(SECOND_SPARK_COLOUR);

        spark.position = self.position;

        spark.origin = self.position;

        spark.rise = 1.0;

        spark.motion = super::effects::Motion::Rise;

        spark.pass = render::billboard::BillboardPass::Additive;

        spark
    }

    fn drive_trail(&mut self, t: f32, now_ms: u32) {
        if t <= TRAIL_START_T
            || now_ms.wrapping_sub(self.birth_ms) >= self.lifetime_ms.max(0) as u32 + TRAIL_GRACE_MS
        {
            return;
        }

        let two_thirds = match self.kind == kind::BOLT_BLUE {
            true => self.source,
            false => (self.source * 2.0 + self.dest) / 3.0,
        };

        let far = self.source * (1.0 - t) + two_thirds * t;

        let (near, position) = (self.position, far);

        if let Some(trail) = &mut self.trail {
            trail.place(near, position);
        }
    }

    fn impact(
        &mut self,
        now: u32,
        ground: Option<&super::shade::Ground<'_>>,
        rng: &mut super::rng::Lcg,
    ) {
        if matches!(self.kind, kind::INVISIBLE | kind::SWARM) {
            return;
        }

        if self.kind == kind::SWARM_WIDE {
            self.impact_shade(self.position, 3, 7, 0x80FFFFFF, 400, now, ground);

            return;
        }

        if self.kind == kind::SHOT && self.submode == 2 {
            self.impact_shade(self.position, 1, 7, 0xFF003377, 800, now, ground);

            for (i, colour) in [0xFFFFFFFF, 0xFF003377].into_iter().enumerate() {
                let mut b = super::effects::Billboard::new(
                    56,
                    700,
                    Vec3::splat(i as f32 * 0.2 + 0.5),
                    0.001,
                    1,
                    80,
                );

                b.pass = render::billboard::BillboardPass::Additive;

                b.position = self.position;

                b.colour_mode = 1;

                b.set_color(colour);

                self.spawned.push(Box::new(b));
            }
        } else if self.kind == kind::BOLT_BLUE {
            self.spawned.push(Box::new(super::timers::TimedQuad::new(
                super::quads::GroundQuad {
                    owner: 0,
                    slot: super::quads::Slot::Placed,
                    texture: 8,
                    pass: render::billboard::BillboardPass::Additive,
                    anchor: self.dest - Vec3::Y * 0.6,
                    base_scale: Vec3::splat(0.01),
                    grow: 0.003,
                    spin_period: 0,
                    ordinal: 0,
                    argb: 0xFFFFFFFF,
                },
                1000,
            )));

            self.impact_shade(self.dest, 4, 7, 0xFF7777FF, 800, now, ground);
        }

        if matches!(self.kind, kind::BOLT | kind::BOLT_BLUE) {
            self.sounds.push(26);

            return;
        }

        self.sounds.push(24);

        let (mut colour, mut shade) = match self.variant {
            8 => (0x80FFCCCC, 0x00883333),
            7 => (0x80FFCCFF, 0x00884388),
            6 => (0x80CCFFCC, 0x00338843),
            5 => (0x80CCCCFF, 0x00222288),
            _ => (0x80FFFFFF, 0x00334388),
        };

        if self.flag == 1 {
            colour = 0xFFFFFFFF;

            shade = 0xFFFFFFFF;
        }

        if self.kind == kind::SHAFT {
            let delta = self.dest - self.source;

            self.yaw = delta.x.atan2(delta.z) + std::f32::consts::FRAC_PI_2;
        }

        let mut mesh = super::effect_mesh::EffectMesh::new(531, colour, self.yaw, 4, now);

        mesh.texture_bias = 229;

        mesh.lifetime_ms = 200;

        mesh.period_ms = 200;

        mesh.position = self.position;

        mesh.pulse = false;

        mesh.scale_xz = 1.0;

        mesh.scale_y = 1.0;

        self.spawned.push(Box::new(mesh));

        let delta = self.dest - self.position;

        let dir = Vec3::new(delta.x, 0.0, delta.z).normalize_or_zero();

        if self.critical {
            shade = 0x00883333;
        }

        self.impact_shade(self.position + dir * 0.5, 7, 118, shade, 200, now, ground);

        if let Ok(entity) = u16::try_from(self.owner) {
            if entity != 0 {
                self.hits.push((entity, now));
            }
        }

        if self.critical {
            let (token, children) = super::burst::build(
                &super::burst::Recipe {
                    at: self.position + dir * 0.3,
                    kind: 5,
                    count: 50,
                    spread: 1.0,
                    argb: 0x00883333,
                    random_colour: false,
                    texture: 231,
                    scatter: 1.0,
                    additive: true,
                    axis: dir,
                    lifetime_ms: 300,
                },
                now,
                rng,
            );

            for child in children {
                self.spawned.push(Box::new(child));
            }

            self.spawned.push(Box::new(token));

            for (motion, offset) in [
                (super::effects::Motion::Spin, 0.05),
                (super::effects::Motion::SpinBack, -0.05),
            ] {
                let mut b =
                    super::effects::Billboard::new(230, 600, Vec3::splat(3.0), 0.0005, 1, 80);

                b.pass = render::billboard::BillboardPass::Additive;

                b.position = self.position + dir * 0.5;

                b.axis = dir;

                b.facing = false;

                b.motion = motion;

                b.roll = self.yaw + std::f32::consts::FRAC_PI_2 + offset;

                b.growth = Vec3::splat(0.002);

                b.set_color(0x00883333);

                self.spawned.push(Box::new(b));
            }
        }
    }

    fn impact_shade(
        &mut self,
        at: Vec3,
        grid: i32,
        texture: usize,
        colour: u32,
        life: u32,
        now: u32,
        ground: Option<&super::shade::Ground<'_>>,
    ) {
        let mut shade =
            super::shade::Shade::new(grid, texture, colour, at, super::effects::SECTOR_UNASSIGNED)
                .with_lifetime(life);

        shade.birth_ms = Some(now);

        if let Some(ground) = ground {
            shade.place(ground);
        }

        self.spawned.push(Box::new(shade));
    }
}

impl Effect for Arrow {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let mut rng = super::rng::Lcg::new(tick.now_ms);

        self.fly(tick.now_ms, &mut rng);
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        if !self.gone
            && let Some(id) = self.model
            && live.mesh.is_some_and(|available| !available(id))
        {
            tracing::warn!(
                mesh = id,
                kind = self.kind,
                submode = self.submode,
                "arrow mesh unavailable; retiring without impact"
            );

            self.gone = true;

            self.visible = false;

            return;
        }

        self.critical = live.viewer.is_some_and(|viewer| viewer.critical);

        self.fly_grounded(tick.now_ms, live.rng, live.ground);
    }

    fn needs_ground(&self) -> bool {
        true
    }

    fn update_grounded(&mut self, tick: &Tick, ground: &super::shade::Ground<'_>) {
        self.fly_grounded(
            tick.now_ms,
            &mut super::rng::Lcg::new(tick.now_ms),
            Some(ground),
        );
    }

    fn submit_effect_children(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        if let Some(trail) = &self.trail {
            trail.submit(cx, out);
        }
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn removed(
        &mut self,
        reason: super::effect_tree::Removal,
        cx: &super::effect_tree::RemovalContext<'_>,
    ) {
        if matches!(
            reason,
            super::effect_tree::Removal::Forced | super::effect_tree::Removal::Replaced
        ) && let Some(mut trail) = self.trail.take()
        {
            trail.tick(cx.now_ms);

            if !trail.expired() {
                self.spawned.push(Box::new(trail));
            }
        }
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        use super::effect_tree::{AuthoredMesh, MeshMaterial, MeshSubmission};
        use render::camera::Matrix4;

        if !self.visible {
            return;
        }

        let stage1 = if self.kind == kind::SWARM_WIDE {
            Some(6)
        } else if self.variant == 0 {
            None
        } else {
            Some(
                VARIANT_TEXTURES
                    .iter()
                    .find(|(variant, _)| *variant == self.variant)
                    .map_or(153, |(_, texture)| *texture) as usize
                    + 10,
            )
        };

        let mut draws = self.draw().into_iter().collect::<Vec<_>>();

        if matches!(self.kind, kind::SWARM | kind::SWARM_WIDE) {
            let wide = self.kind == kind::SWARM_WIDE;

            let y = self.position.y + self.swarm[0] / 3.0 + if wide { 0.1 } else { 0.3 };

            for i in 0..3 {
                let x = self.swarm[i * 2];

                let z = self.swarm[i * 2 + 1];

                let (yaw, pitch, roll) = if wide {
                    (
                        match i {
                            0 => x * 2.0 - 1.6,
                            1 => x * 2.0,
                            _ => self.swarm[2] * 2.0 + 4.2,
                        },
                        x * 2.0,
                        x * 0.5,
                    )
                } else {
                    (x * 4.0, 0.0, 0.0)
                };

                draws.push((
                    Vec3::new(self.position.x + x, y, self.position.z + z),
                    yaw,
                    pitch,
                    roll,
                    if wide { 1.5 } else { 0.55 },
                ));
            }
        }

        if let Some(model) = self.model.and_then(|id| u32::try_from(id).ok()) {
            for (at, yaw, pitch, roll, scale) in draws {
                let rotate = Matrix4::rotation_yaw_pitch_roll(
                    yaw,
                    pitch - std::f32::consts::FRAC_PI_2,
                    roll,
                );

                let scaled = Matrix4::multiply(&rotate, &Matrix4::scaling(scale, scale, scale));

                let world = Matrix4::multiply(&scaled, &Matrix4::translation(at.x, at.y, at.z));

                out.mesh.push(MeshSubmission::Authored(AuthoredMesh {
                    mesh_id: model,
                    world,
                    material: MeshMaterial::Arrow { stage1 },
                    texture: None,
                }));
            }
        }

        if let Some(plane) = &self.plane {
            plane.submit(cx, out);
        }
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        std::mem::take(&mut self.spawned)
    }

    fn take_sounds(&mut self) -> Vec<i64> {
        std::mem::take(&mut self.sounds)
    }

    fn take_hits(&mut self) -> Vec<(u16, u32)> {
        std::mem::take(&mut self.hits)
    }
}
