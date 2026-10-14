use super::actors::{Chirality, Model, skin_world};
use super::effects::Billboard;
use super::objects::BoundTexture;
use super::rng::Lcg;
use formats::mesh::pose::FrameTree;
use glam::Vec3;
use render::camera::{self, Matrix4};
use render::skin::{BonePalette, SkinDraw, SkinFog, SkinObject, SkinParams, SkinPass};

pub const KIND: i32 = 0x14B;

pub const AFTERIMAGE_PARTS: usize = formats::mesh::appearance::PARTS;

pub const TRAVEL_MS_PER_UNIT: u32 = 200;

pub const TRAVEL_MIN_MS: u32 = 1;

pub const TRAVEL_MAX_MS: u32 = 5000;

pub const HARD_CEILING_MS: u32 = 10_000;

pub const MIN_PROGRESS: f32 = 0.01;

pub const AFTERIMAGE_MS: u32 = 3000;

pub fn travel_time_ms(src: Vec3, dst: Vec3) -> u32 {
    let d = dst - src;

    let units = (d.x * d.x + d.y * d.y + d.z * d.z).sqrt() as i32;

    let ms = (units as i64) * i64::from(TRAVEL_MS_PER_UNIT);

    ms.clamp(i64::from(TRAVEL_MIN_MS), i64::from(TRAVEL_MAX_MS)) as u32
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Blend {
    Alpha,
    Additive,
    AdditiveTinted,
}

pub const ADDITIVE_EMISSIVE: f32 = 0.3;

pub const TINTED_EMISSIVE_SCALE: f32 = 0.7;

impl Blend {
    pub const ALL: [Blend; 3] = [Blend::Alpha, Blend::Additive, Blend::AdditiveTinted];

    pub fn of(value: i32) -> Option<Blend> {
        match value {
            0 => Some(Blend::Alpha),
            1 => Some(Blend::Additive),
            2 => Some(Blend::AdditiveTinted),
            _ => None,
        }
    }

    pub fn pass(self) -> SkinPass {
        match self {
            Blend::Alpha => SkinPass::EffectAlpha,
            Blend::Additive | Blend::AdditiveTinted => SkinPass::EffectAdditive,
        }
    }

    pub fn emissive(self, diffuse: [f32; 4]) -> [f32; 4] {
        match self {
            Blend::Alpha => [0.0; 4],
            Blend::Additive => [ADDITIVE_EMISSIVE, ADDITIVE_EMISSIVE, ADDITIVE_EMISSIVE, 0.0],
            Blend::AdditiveTinted => [
                diffuse[0] * TINTED_EMISSIVE_SCALE,
                diffuse[1] * TINTED_EMISSIVE_SCALE,
                diffuse[2] * TINTED_EMISSIVE_SCALE,
                0.0,
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum ColourAnim {
    None,
    Cosine,
    Cross,
}

impl ColourAnim {
    pub fn of(value: i32) -> ColourAnim {
        match value {
            1 => ColourAnim::Cosine,
            2 => ColourAnim::Cross,
            _ => ColourAnim::None,
        }
    }

    pub fn apply(self, t: f32, a: [f32; 3], b: [f32; 3]) -> Option<[f32; 3]> {
        let half_pi = camera::PI * 0.5;

        match self {
            ColourAnim::None => None,
            ColourAnim::Cosine => {
                let k = (half_pi * t).cos();

                Some([a[0] * k, a[1] * k, a[2] * k])
            }
            ColourAnim::Cross => {
                let k1 = (half_pi * t).cos();

                let k2 = (half_pi * (1.0 - t)).cos();

                Some([
                    a[0] * k1 + b[0] * k2,
                    a[1] * k1 + b[1] * k2,
                    a[2] * k1 + b[2] * k2,
                ])
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Motion {
    None,
    Rise,
    Projectile,
    Orbit,
    WobbleProjectile,
    OwnerHalo,
    OwnerHaloLow,
    OwnerSeek,
    Sink,
    Drop,
    WeaponTrack,
}

impl Motion {
    pub const VALUES: [i32; 10] = [1, 2, 3, 4, 5, 7, 6, 8, 9, 10];

    pub fn of(value: i32) -> Motion {
        match value {
            1 => Motion::Rise,
            2 => Motion::Projectile,
            3 => Motion::Orbit,
            4 => Motion::WobbleProjectile,
            5 => Motion::OwnerHalo,
            7 => Motion::OwnerHaloLow,
            6 => Motion::OwnerSeek,
            8 => Motion::Sink,
            9 => Motion::Drop,
            10 => Motion::WeaponTrack,
            _ => Motion::None,
        }
    }

    pub fn needs_owner(self) -> bool {
        matches!(self, Motion::OwnerHalo | Motion::OwnerHaloLow)
    }
}

pub const TRAIL_COLOURS: [u32; 3] = [0xFFFF_AA00, 0xFFFF_0000, 0xFFFF_AAFF];

pub fn trail_colour(variant: i32) -> Option<u32> {
    usize::try_from(variant)
        .ok()
        .and_then(|i| TRAIL_COLOURS.get(i).copied())
}

pub const BEAD_COUNT: i32 = 2;

pub const BEAD_SOUND: i32 = 1;

pub const BEAD_STYLE: i32 = 0;

pub const ORBIT_BEAD_ARGB: u32 = 0xFFFF_0000;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Mote {
    pub kind: usize,
    pub lifetime_ms: u32,
    pub size: Vec3,
    pub growth: f32,
    pub rise: f32,
    pub tint: Option<u32>,
}

pub const MOTE_FRAMES: u32 = 1;

pub const MOTE_FRAME_MS: u32 = 0x50;

pub const MOTE_JITTER_STEP: f32 = 0.02;

pub const MOTE_JITTER_BIAS: i32 = -5;

pub const MOTE_JITTER_MOD: u32 = 10;

pub fn owner_halo_mote(variant: i32, scale: f32, roll: u32) -> Mote {
    match variant {
        5 => Mote {
            kind: 2,
            lifetime_ms: 0x9C4,
            size: Vec3::splat(scale * 0.03),
            growth: 0.0,
            rise: -1.5,
            tint: Some(0xFFEE_EEFF),
        },
        6 => Mote {
            kind: 200,
            lifetime_ms: 0x5DC,
            size: Vec3::splat(scale * 0.02),
            growth: 0.0,
            rise: -0.5,
            tint: None,
        },
        _ => {
            let r = (roll % 5) as f32;

            let f2 = r * 0.01 + scale * 0.01;

            Mote {
                kind: 0,
                lifetime_ms: 0x5DC,
                size: Vec3::new(f2, r * 0.1 + scale * 0.01, f2),
                growth: 1.0e-4,
                rise: -0.5,
                tint: match variant {
                    3 => Some(0xFFFF_AAFF),
                    4 => Some(0xFFAA_FFEE),
                    _ => None,
                },
            }
        }
    }
}

pub fn low_owner_halo_mote(variant: i32, roll: u32) -> Mote {
    let r = (roll % 5) as f32;

    let f2 = r * 0.01 + 0.02;

    Mote {
        kind: 0,
        lifetime_ms: 0x5DC,
        size: Vec3::new(f2, r * 0.1 + 0.02, f2),
        growth: 1.0e-4,
        rise: 0.5,
        tint: (variant == 3).then_some(0xFFFF_0000),
    }
}

pub fn owner_seek_mote(roll: u32) -> Mote {
    let r = (roll % 5) as f32;

    let f1 = r * 0.05 + 0.02;

    Mote {
        kind: 0,
        lifetime_ms: 0x5DC,
        size: Vec3::new(f1, r * 0.1 + 0.02, f1),
        growth: 5.0e-4,
        rise: 1.0,
        tint: Some(0xFFAA_8877),
    }
}

#[derive(Debug, Default, Clone)]

pub struct Emissions {
    pub beads: Vec<super::skill::poison::Built>,
    pub motes: Vec<Billboard>,
}

impl Emissions {
    pub fn is_empty(&self) -> bool {
        self.beads.is_empty() && self.motes.is_empty()
    }

    pub fn nodes(&self) -> usize {
        self.motes.len()
            + self
                .beads
                .iter()
                .map(super::skill::poison::Built::nodes)
                .sum::<usize>()
    }
}

pub fn mote_billboard(mote: &Mote, at: Vec3, rng: &mut Lcg) -> Billboard {
    let mut b = Billboard::new(
        mote.kind,
        mote.lifetime_ms,
        mote.size,
        mote.growth,
        MOTE_FRAMES,
        MOTE_FRAME_MS,
    );

    let jitter = |rng: &mut Lcg| {
        ((rng.next() % MOTE_JITTER_MOD) as i32 + MOTE_JITTER_BIAS) as f32 * MOTE_JITTER_STEP
    };

    let dx = jitter(rng);

    let dz = jitter(rng);

    b.position = Vec3::new(at.x + dx, at.y, at.z + dz);

    b.origin = b.position;

    b.pass = render::billboard::BillboardPass::Additive;

    b.lift = false;

    b.motion = super::effects::Motion::from_field(1);

    b.rise = mote.rise;

    if let Some(argb) = mote.tint {
        b.set_color(argb);
    }

    b
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Owner {
    pub position: Vec3,
    pub yaw: f32,
    pub scale: f32,
    pub weapon: Option<(Vec3, Vec3)>,
}

#[derive(Debug, Clone)]

pub struct Rig {
    pub model: usize,
    pub mesh_id: i32,
    pub tree: FrameTree,
    pub anim_speed_ms: u32,
    pub phase_ms: u32,
    pub animation: super::actors::Anim,
    pub frozen: bool,
    pub clock_started: bool,
    pub scale_factor: f32,
    pub parent_anchor: Option<(formats::mesh::appearance::AnchorRule, (f32, f32))>,
}

pub const ANIM_SPEED_MS: u32 = 0x50;

impl Rig {
    pub fn new(model: usize, mesh_id: i32, tree: FrameTree) -> Rig {
        Rig {
            model,
            mesh_id,
            tree,
            anim_speed_ms: ANIM_SPEED_MS,
            phase_ms: 0,
            animation: super::actors::Anim::new(0, ANIM_SPEED_MS),
            frozen: false,
            clock_started: false,
            scale_factor: 1.0,
            parent_anchor: None,
        }
    }
}

#[derive(Debug, Clone)]

pub struct SkinMeshEffect {
    pub construction: Option<SkinConstruction>,
    pub owner: Option<super::effect_context::ActorKey>,
    pub held_slot: Option<u16>,
    pub held_item: Option<u16>,
    pub visible: bool,
    pub mesh_id: i32,
    pub mesh_id2: Option<i32>,
    pub src: Vec3,
    pub dst: Vec3,
    pub variant: i32,
    pub lifetime_ms: u32,
    pub start_ms: Option<u32>,
    pub blend: Blend,
    pub motion: Motion,
    pub colour_anim: ColourAnim,
    pub colour_a: [f32; 3],
    pub colour_b: [f32; 3],
    pub colour: [f32; 4],
    pub position: Vec3,
    pub yaw: f32,
    pub spin: f32,
    pub scale: f32,
    pub scale2: f32,
    pub rig: Option<Rig>,
    pub rig2: Option<Rig>,
    pub gone: bool,
    pub progress: f32,
    emissions: Emissions,
}

impl SkinMeshEffect {
    pub fn new(mesh_id: i32, src: Vec3, dst: Vec3, variant: i32) -> SkinMeshEffect {
        SkinMeshEffect {
            construction: None,
            owner: None,
            held_slot: None,
            held_item: None,
            visible: true,
            mesh_id,
            mesh_id2: None,
            src,
            dst,
            variant,
            lifetime_ms: travel_time_ms(src, dst),
            start_ms: None,
            blend: Blend::Alpha,
            motion: Motion::None,
            colour_anim: ColourAnim::None,
            colour_a: [1.0; 3],
            colour_b: [1.0; 3],
            colour: [1.0, 1.0, 1.0, 0.0],
            position: Vec3::ZERO,
            yaw: 0.0,
            spin: 0.0,
            scale: 1.0,
            scale2: 1.0,
            rig: None,
            rig2: None,
            gone: false,
            progress: 0.0,
            emissions: Emissions::default(),
        }
    }

    pub fn place(&mut self, point: Vec3) {
        self.src = point;

        self.position = point;
    }

    pub fn build_skins(&mut self, primary: Rig, secondary: Option<Rig>) {
        self.rig = Some(primary);

        self.rig2 = match self.mesh_id2 {
            Some(_) => secondary,
            None => None,
        };
    }

    pub fn age_ms(&self, now_ms: u32) -> Option<u32> {
        let start = self.start_ms?;

        let age = now_ms.wrapping_sub(start);

        (self.lifetime_ms == 0 || age < 0x8000_0000).then_some(age)
    }

    pub fn progress_at(&self, age_ms: u32) -> f32 {
        match self.lifetime_ms {
            0 => 1.0,
            life => age_ms as f32 / life as f32,
        }
    }

    pub fn update(&mut self, now_ms: u32, owner: Option<Owner>) -> bool {
        let mut rng = Lcg::new(self.start_ms.unwrap_or(now_ms).max(1));

        self.update_with(now_ms, owner, 0, &mut rng)
    }

    pub fn update_with(
        &mut self,
        now_ms: u32,
        owner: Option<Owner>,
        detail: i32,
        rng: &mut Lcg,
    ) -> bool {
        if self.gone {
            return false;
        }

        self.start_ms.get_or_insert(now_ms);

        let Some(age) = self.age_ms(now_ms) else {
            return true;
        };

        if self.lifetime_ms != 0 && (self.lifetime_ms < age || age > HARD_CEILING_MS) {
            self.gone = true;

            return false;
        }

        let t = self.progress_at(age);

        if self.lifetime_ms != 0 && t < MIN_PROGRESS {
            return true;
        }

        self.progress = t;

        if self.lifetime_ms != 0
            && let Some(rgb) = self.colour_anim.apply(t, self.colour_a, self.colour_b)
        {
            self.colour[0] = rgb[0];

            self.colour[1] = rgb[1];

            self.colour[2] = rgb[2];
        }

        if t < 1.0
            && let Some(o) = owner
        {
            self.dst = o.position;
        }

        self.update_motion(age, t, owner, detail, rng, now_ms);

        true
    }

    pub fn take_emissions(&mut self) -> Emissions {
        std::mem::take(&mut self.emissions)
    }

    fn emit_bead(&mut self, argb: Option<u32>, now_ms: u32, detail: i32, rng: &mut Lcg) {
        let Some(argb) = argb else {
            return;
        };

        let recipe = super::skill::poison::Recipe {
            at: self.position,
            argb,
            count: BEAD_COUNT,
            play_sound: BEAD_SOUND,
            style: BEAD_STYLE,
        };

        self.emissions
            .beads
            .push(super::skill::poison::build(&recipe, now_ms, rng, detail));
    }

    fn emit_mote(&mut self, mote: &Mote, detail: i32, rng: &mut Lcg) {
        if !super::lod::DetailGate::Full.admits(detail) {
            return;
        }

        let at = self.position;

        self.emissions.motes.push(mote_billboard(mote, at, rng));
    }

    fn update_motion(
        &mut self,
        age_ms: u32,
        t: f32,
        owner: Option<Owner>,
        detail: i32,
        rng: &mut Lcg,
        now_ms: u32,
    ) {
        let age_s = age_ms as f32 / 1000.0;

        match self.motion {
            Motion::Rise => self.position.y = self.src.y + age_s,
            Motion::Sink => {
                self.position.y = self.src.y - (age_ms + age_ms) as f32 / 1000.0;
            }
            Motion::Drop => {
                if t < 1.0 {
                    self.position.y = (1.0 - t) * 3.0 + self.src.y;
                }
            }
            Motion::Projectile => {
                let (dx, dz) = match owner {
                    Some(o) => (o.position.x - self.src.x, o.position.z - self.src.z),
                    None => (self.dst.x - self.src.x, self.dst.z - self.src.z),
                };

                self.yaw = bearing(dx, dz);

                if t < 1.0 {
                    self.position = lerp(self.src, self.dst, t);

                    self.emit_bead(trail_colour(self.variant), now_ms, detail, rng);
                }

                self.set_anim_speed(if self.variant == 0 { 5 } else { 4 });
            }
            Motion::WobbleProjectile => {
                self.yaw = bearing(self.dst.x - self.src.x, self.dst.z - self.src.z);

                if t < 1.0 {
                    let phase = age_s * std::f32::consts::TAU + self.spin;

                    self.position = lerp(self.src, self.dst, t)
                        + Vec3::new(phase.cos() * 0.2, 0.0, phase.sin() * 0.2);

                    self.emit_bead(trail_colour(self.variant), now_ms, detail, rng);
                }

                self.set_anim_speed(5);
            }
            Motion::Orbit => {
                self.yaw = ((age_ms % 1000) as f32 * camera::PI * 2.0) / 1000.0 + self.spin;

                let (cos, sin) = (self.yaw.cos(), self.yaw.sin());

                match owner {
                    Some(o) => {
                        self.position =
                            Vec3::new(o.position.x + cos, o.position.y + 0.5, o.position.z + sin);
                    }
                    None => {
                        self.position = Vec3::new(
                            self.src.x + cos,
                            (self.dst.y + self.src.y) * 0.5,
                            self.src.z + sin,
                        );
                    }
                }

                self.emit_bead(Some(ORBIT_BEAD_ARGB), now_ms, detail, rng);

                self.set_anim_speed(5);
            }
            Motion::OwnerSeek => {
                if let Some(o) = owner {
                    self.yaw = (o.position.x - self.src.x).atan2(o.position.z - self.src.z)
                        + camera::PI * 0.5;
                }

                if t < 1.0 {
                    self.position = lerp(self.src, self.dst, t);
                }

                if super::lod::DetailGate::Full.admits(detail) {
                    let mote = owner_seek_mote(rng.next());

                    self.emit_mote(&mote, detail, rng);
                }
            }
            Motion::OwnerHalo | Motion::OwnerHaloLow => {
                let Some(o) = owner else {
                    return;
                };

                let h = match self.motion_code() {
                    7 => 1.9,
                    _ => 2.0,
                };

                let phase = age_s * std::f32::consts::TAU + self.spin;

                self.position = Vec3::new(
                    o.position.x + phase.cos() * 0.3 + (phase * 0.5).cos() * 0.1,
                    o.scale * h + o.position.y + phase.sin() * 0.05,
                    o.position.z + phase.sin() * 0.3 + (phase * 0.5).sin() * 0.1,
                );

                self.yaw = o.yaw;

                let wide = (4..=6).contains(&self.variant);

                let motion5 = self.motion_code() == 5;

                self.set_anim_speed(match (motion5, wide) {
                    (false, _) => 4,
                    (true, true) => 10,
                    (true, false) if self.variant == 3 => 5,
                    (true, false) => self.rig.as_ref().map_or(80, |rig| rig.anim_speed_ms),
                });

                let roll = rng.next();

                let mote = match motion5 {
                    true => owner_halo_mote(self.variant, if wide { 2.0 } else { 1.0 }, roll),
                    false => low_owner_halo_mote(self.variant, roll),
                };

                self.emit_mote(&mote, detail, rng);
            }
            Motion::WeaponTrack => {
                if let Some(o) = owner
                    && let Some((a, b)) = o.weapon
                {
                    self.position = (a + b) * 0.5;

                    self.yaw = o.yaw;
                }

                if t > WEAPON_FADE_START {
                    let k = (camera::PI * (t - WEAPON_FADE_START) / WEAPON_FADE_SPAN).cos();

                    for channel in 0..3 {
                        self.colour[channel] *= k;
                    }

                    self.scale = (t - WEAPON_FADE_START) + 1.0;
                }

                self.set_anim_speed(self.lifetime_ms.saturating_mul(15) / 1000);
            }
            Motion::None => {}
        }
    }

    fn motion_code(&self) -> i32 {
        match self.motion {
            Motion::None => 0,
            Motion::Rise => 1,
            Motion::Projectile => 2,
            Motion::Orbit => 3,
            Motion::WobbleProjectile => 4,
            Motion::OwnerHalo => 5,
            Motion::OwnerHaloLow => 7,
            Motion::OwnerSeek => 6,
            Motion::Sink => 8,
            Motion::Drop => 9,
            Motion::WeaponTrack => 10,
        }
    }

    fn set_anim_speed(&mut self, ms: u32) {
        if let Some(rig) = self.rig.as_mut() {
            rig.anim_speed_ms = ms;
        }
    }

    pub fn animate(
        &mut self,
        now_ms: u32,
        models: &[Model],
        sounds: &formats::audio::ani_sound::AniSound,
    ) {
        self.animate_with_smoothing(now_ms, models, sounds, formats::mesh::pose::Smoothing::On);
    }

    pub fn animate_with_smoothing(
        &mut self,
        now_ms: u32,
        models: &[Model],
        sounds: &formats::audio::ani_sound::AniSound,
        smoothing: formats::mesh::pose::Smoothing,
    ) {
        let Some(age) = self.age_ms(now_ms) else {
            return;
        };

        if self.gone
            || (self.lifetime_ms != 0
                && (age > self.lifetime_ms || self.progress_at(age) < MIN_PROGRESS))
        {
            return;
        }

        let action = match self.motion {
            Motion::Projectile => Some(if self.variant == 0 { 2 } else { 3 }),
            Motion::Orbit | Motion::OwnerHaloLow => Some(3),
            Motion::WobbleProjectile | Motion::OwnerHalo | Motion::WeaponTrack => Some(2),
            _ => None,
        };

        for (index, rig) in [self.rig.as_mut(), self.rig2.as_mut()]
            .into_iter()
            .enumerate()
        {
            let Some(rig) = rig else {
                continue;
            };

            if rig.frozen {
                continue;
            }

            let Some(model) = models.get(rig.model) else {
                continue;
            };

            if !rig.clock_started {
                rig.animation.start_ms = self.start_ms.unwrap_or(now_ms);

                rig.clock_started = true;
            }

            if index == 0
                && let Some(action) = action
            {
                if let Some(slot) = sounds
                    .class(model.model_id as usize)
                    .and_then(|table| table.frame.get(action))
                    .and_then(|slot| usize::try_from(*slot).ok())
                {
                    rig.animation.set_clip(slot, now_ms, model);
                }
            }

            rig.animation.sub_frame_ms = if rig.anim_speed_ms == 0 {
                30
            } else {
                rig.anim_speed_ms
            };

            super::actors::pose_anim(
                &mut rig.tree,
                model,
                &mut rig.animation,
                now_ms.wrapping_add(rig.phase_ms),
                &Matrix4::IDENTITY,
                smoothing,
            );
        }
    }

    pub fn halo_height(&self) -> f32 {
        match self.motion_code() {
            7 => 1.9,
            _ => 2.0,
        }
    }

    pub fn draws_at_detail(&self, detail: i32) -> bool {
        self.visible && (detail != 1 || matches!(self.variant, 3..=6))
    }

    pub fn world(&self, model_id: u32) -> Matrix4 {
        skin_world(
            model_id,
            [0.0, self.yaw, 0.0],
            self.scale,
            self.position,
            Chirality::AsAuthored,
        )
    }

    pub fn submit(
        &self,
        cx: &Emit<'_>,
        (objects, palettes, draws): (
            &mut Vec<SkinObject>,
            &mut Vec<BonePalette>,
            &mut Vec<SkinDraw>,
        ),
    ) -> usize {
        if !self.visible {
            return 0;
        }

        let mut emitted = 0;

        for rig in [self.rig.as_ref(), self.rig2.as_ref()]
            .into_iter()
            .flatten()
        {
            let world = if let Some((rule, (yaw, pitch))) = rig.parent_anchor {
                let Some(primary) = self.rig.as_ref() else {
                    continue;
                };

                let mut tree = primary.tree.clone();

                tree.combine(&self.rig_world(primary).as_d3dx());

                let Some(anchor) = tree.last_combined(rule.bones) else {
                    continue;
                };

                if !self.scale2.is_finite() || self.scale2 == 0.0 {
                    continue;
                }

                super::actors::attached_world(
                    [self.scale2 * 0.2, self.scale2 * -0.37],
                    1.0 / self.scale2,
                    yaw,
                    pitch,
                    &Matrix4(rule.adjust.apply(&anchor)),
                )
            } else {
                self.rig_world(rig)
            };

            emitted += self.emit_rig(rig, cx, &world, (objects, palettes, draws));
        }

        emitted
    }

    fn rig_world(&self, rig: &Rig) -> Matrix4 {
        skin_world(
            rig.mesh_id as u32,
            [0.0, self.yaw, 0.0],
            self.scale * rig.scale_factor,
            self.position,
            Chirality::AsAuthored,
        )
    }

    fn emit_rig(
        &self,
        rig: &Rig,
        cx: &Emit<'_>,
        world: &Matrix4,
        (objects, palettes, draws): (
            &mut Vec<SkinObject>,
            &mut Vec<BonePalette>,
            &mut Vec<SkinDraw>,
        ),
    ) -> usize {
        let Some(model) = cx.models.get(rig.model) else {
            return 0;
        };

        let mut tree = rig.tree.clone();

        tree.combine(&world.as_d3dx());

        let emissive = self.blend.emissive(self.colour);

        let mut emitted = 0;

        for part in &model.parts {
            let texture = part.texture;

            let bound = texture.and_then(|index| cx.textures.get(index));

            if texture.is_some() && bound.is_none() {
                continue;
            }

            let params = SkinParams {
                diffuse: self.colour,
                ambient: [0.0; 4],
                emissive,
                unlit: model.model_id == 0x3D,
                fog_start: cx.fog.start,
                fog_end: cx.fog.end,
                fog_enabled: cx.fog.enabled,
                force_opaque: bound.is_some_and(|bound| !bound.flag.has_alpha()),
                modulate_alpha: false,
                glow: None,
            };

            objects.push(SkinObject::new(
                &params,
                part.mesh.header.num_matrices,
                EFFECT_ALPHA_REFERENCE,
            ));

            palettes.push(BonePalette::build(&part.mesh, cx.view, |bone| {
                Matrix4(tree.combined(bone as usize))
            }));

            draws.push(SkinDraw {
                pass: self.blend.pass(),
                part: part.alloc,
                object: objects.len() as u32 - 1,
                palette: palettes.len() as u32 - 1,
                texture,
            });

            emitted += 1;
        }

        emitted
    }
}

#[derive(Debug, Clone)]

pub struct SkinInput {
    pub model_id: i32,
    pub parts: [formats::mesh::appearance::PartRef; 8],
    pub pose: Option<RigSnapshot>,
    pub animation: Option<super::actors::Anim>,
    pub clip_action: Option<usize>,
    pub scale: f32,
}

#[derive(Debug, Clone)]

pub struct SkinConstruction {
    pub primary: SkinInput,
    pub secondary: Option<SkinInput>,
    pub cape: bool,
    pub body: Option<super::effect_context::PosedBody>,
}

pub const WEAPON_FADE_START: f32 = 0.57;

pub const WEAPON_FADE_SPAN: f32 = 0.86;

fn bearing(dx: f32, dz: f32) -> f32 {
    dz.atan2(dx) + camera::PI * 0.5
}

fn lerp(src: Vec3, dst: Vec3, t: f32) -> Vec3 {
    let inv = 1.0 - t;

    Vec3::new(
        inv * src.x + dst.x * t,
        src.y * inv + dst.y * t,
        inv * src.z + dst.z * t,
    )
}

pub struct Emit<'a> {
    pub models: &'a [Model],
    pub textures: &'a [BoundTexture],
    pub view: &'a Matrix4,
    pub fog: SkinFog,
}

pub const EFFECT_ALPHA_REFERENCE: f32 = 0.0;

#[derive(Debug)]

pub enum Removal {
    Fire(Vec3),
    Orbit(Box<SkinMeshEffect>),
}

impl SkinMeshEffect {
    pub fn removal(&self) -> Option<Removal> {
        if self.variant == 0 && self.motion == Motion::Projectile {
            return Some(Removal::Fire(self.dst));
        }

        if self.variant != 1 {
            return None;
        }

        let mut child = SkinMeshEffect::new(20, self.dst, Vec3::ZERO, 0);

        child.owner = self.owner;

        child.lifetime_ms = 3000;

        child.motion = Motion::Orbit;

        child.spin = self.yaw;

        let parts = match &self.construction {
            Some(request) => request.primary.parts,
            None => [formats::mesh::appearance::PartRef::EMPTY; 8],
        };

        child.construction = Some(SkinConstruction {
            primary: SkinInput {
                model_id: 20,
                parts,
                pose: None,
                animation: None,
                clip_action: None,
                scale: 0.2,
            },
            secondary: None,
            cape: false,
            body: None,
        });

        Some(Removal::Orbit(Box::new(child)))
    }
}

#[derive(Debug, Default)]

pub struct SkinFxScene {
    pub objects: Vec<SkinMeshEffect>,
    pub released: usize,
    emitted: Emissions,
    removed: Vec<SkinMeshEffect>,
}

impl SkinFxScene {
    pub fn spawn(&mut self, effect: SkinMeshEffect) {
        self.objects.push(effect);
    }

    pub fn len(&self) -> usize {
        self.objects.len()
    }

    pub fn is_empty(&self) -> bool {
        self.objects.is_empty()
    }

    pub fn remove_where(&mut self, mut matches: impl FnMut(&SkinMeshEffect) -> bool) -> usize {
        let before = self.objects.len();

        self.objects.retain(|effect| {
            if !matches(effect) {
                return true;
            }

            self.removed.push(effect.clone());

            false
        });

        let count = before - self.objects.len();

        self.released += count;

        count
    }

    pub fn update(&mut self, now_ms: u32, owner_of: impl Fn(&SkinMeshEffect) -> Option<Owner>) {
        let mut rng = Lcg::new(now_ms.max(1));

        self.update_with(now_ms, 0, &mut rng, owner_of);
    }

    pub fn update_with(
        &mut self,
        now_ms: u32,
        detail: i32,
        rng: &mut Lcg,
        owner_of: impl Fn(&SkinMeshEffect) -> Option<Owner>,
    ) {
        for effect in &mut self.objects {
            let owner = owner_of(effect);

            if owner.is_none() {
                effect.owner = None;
            }

            effect.update_with(now_ms, owner, detail, rng);

            let mut out = effect.take_emissions();

            if effect.held_slot.is_some() {
                if !effect.visible {
                    continue;
                }

                for mote in &mut out.motes {
                    mote.conceal_with = effect.owner;
                }
            }

            self.emitted.beads.append(&mut out.beads);

            self.emitted.motes.append(&mut out.motes);
        }

        self.remove_where(|effect| effect.gone);
    }

    pub fn take_emissions(&mut self) -> Emissions {
        std::mem::take(&mut self.emitted)
    }

    pub fn take_removed(&mut self) -> Vec<SkinMeshEffect> {
        std::mem::take(&mut self.removed)
    }

    pub fn submit(
        &self,
        cx: &Emit<'_>,
        detail: i32,
        (objects, palettes, draws): (
            &mut Vec<SkinObject>,
            &mut Vec<BonePalette>,
            &mut Vec<SkinDraw>,
        ),
    ) -> usize {
        let mut emitted = 0;

        for effect in &self.objects {
            if effect.draws_at_detail(detail) {
                emitted += effect.submit(cx, (objects, palettes, draws));
            }
        }

        emitted
    }
}

pub fn afterimage(body: BodySnapshot) -> SkinMeshEffect {
    let mut effect = SkinMeshEffect::new(body.mesh_id, Vec3::ZERO, Vec3::ZERO, 0);

    if let Some(mount) = body.mount.as_ref() {
        effect.mesh_id = mount.mesh_id;

        effect.mesh_id2 = Some(body.mesh_id);
    }

    effect.colour_a = [1.0; 3];

    effect.scale = body.scale;

    effect.colour_anim = ColourAnim::Cosine;

    effect.lifetime_ms = AFTERIMAGE_MS;

    effect.place(Vec3::new(
        body.position.x,
        body.position.y + 0.1,
        body.position.z,
    ));

    effect.spin = body.yaw;

    effect.yaw = match body.weapon_class == YAW_EXEMPT_WEAPON_CLASS {
        true => body.yaw,
        false => body.yaw + std::f32::consts::TAU,
    };

    effect.blend = Blend::Additive;

    effect.motion = Motion::None;

    let (primary, secondary) = match body.mount {
        Some(mount) => (
            Rig::new(mount.model, mount.mesh_id, mount.tree),
            Some(Rig::new(body.model, body.mesh_id, body.tree)),
        ),
        None => (Rig::new(body.model, body.mesh_id, body.tree), None),
    };

    effect.build_skins(primary, secondary);

    effect
}

pub const YAW_EXEMPT_WEAPON_CLASS: i32 = 0x65;

#[derive(Debug, Clone)]

pub struct RigSnapshot {
    pub model: usize,
    pub mesh_id: i32,
    pub tree: FrameTree,
}

#[derive(Debug, Clone)]

pub struct BodySnapshot {
    pub mesh_id: i32,
    pub model: usize,
    pub tree: FrameTree,
    pub position: Vec3,
    pub yaw: f32,
    pub scale: f32,
    pub weapon_class: i32,
    pub mount: Option<RigSnapshot>,
}

impl BodySnapshot {
    pub fn of(actor: &super::actors::Actor) -> BodySnapshot {
        let mount = match (actor.mounted, actor.mount.as_ref()) {
            (true, Some(mount)) => Some(RigSnapshot {
                model: mount.model,
                mesh_id: actor.mount_model.unwrap_or_default() as i32,
                tree: mount.tree.clone(),
            }),
            _ => None,
        };

        BodySnapshot {
            mesh_id: actor.model_id as i32,
            model: actor.model,
            tree: actor.tree.clone(),
            position: actor.position,
            yaw: actor.yaw,
            scale: actor.presentation_scale(),
            weapon_class: actor.weapon_class,
            mount,
        }
    }
}
