use super::effect_tree::{Base, Class, Cull, Effect, OwnedGroup, Pass, Sink, Submit, Tick, Tree};
use super::objects::{BoundTexture, EFFECT_TEXTURE_LIST};
use super::shade::Shade;
use anyhow::{Context, Result};
use assets::GameDir;
use formats::texture::texture_list::{self, TextureList};
use glam::Vec3;
use render::billboard::{
    BillboardArray, BillboardDraw, BillboardPass, BillboardRenderer, BillboardUniforms,
    BillboardVertex, QUAD_INDICES, QUAD_VERTICES,
};
use render::camera::Matrix4;
use render::device::Gpu;
use render::texture::AlphaPolicy;
use rustc_hash::FxHashMap;

#[cfg(test)]
mod old_skill_vfx_tests;

#[cfg(test)]
mod view_distance_tests;

pub(crate) fn visible_in_scene(scene: u32, enabled: bool) -> bool {
    scene != 30000 || enabled
}

pub const CULL_DISTANCE: f32 = 45.0;

pub const CULL_DISTANCE_SQUARED: f32 = CULL_DISTANCE * CULL_DISTANCE;

pub const MIN_AGE: f32 = 0.05;

pub const AGE_FLOOR: f32 = 0.01;

pub const PULSE_FLOOR: f32 = 0.7;

pub const PULSE_PERIOD_MS: u32 = 3000;

pub const PULSE_PHASE_STEP_MS: u32 = 200;

pub const PULSE_PHASE_SEEDS: u32 = 100;

pub const BAHAMUT_GLOW_SLOTS: std::ops::RangeInclusive<usize> = 464..=511;

pub const SKY_SLOTS: std::ops::RangeInclusive<usize> = 0x43..=0x46;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Skipped {
    BahamutGlow,
    Sky,
}

pub fn skipped(slot: usize) -> Option<Skipped> {
    if BAHAMUT_GLOW_SLOTS.contains(&slot) {
        Some(Skipped::BahamutGlow)
    } else if SKY_SLOTS.contains(&slot) {
        Some(Skipped::Sky)
    } else {
        None
    }
}

pub const SECTOR_UNASSIGNED: i32 = -1;

pub const UNIT_QUAD: [BillboardVertex; 4] = [
    BillboardVertex {
        position: [-0.5, -0.5, 0.0],
        diffuse: 0xFFFF_FFFF,
        uv: [0.02, 0.98],
    },
    BillboardVertex {
        position: [0.5, -0.5, 0.0],
        diffuse: 0xFFFF_FFFF,
        uv: [0.98, 0.98],
    },
    BillboardVertex {
        position: [0.5, 0.5, 0.0],
        diffuse: 0xFFFF_FFFF,
        uv: [0.98, 0.02],
    },
    BillboardVertex {
        position: [-0.5, 0.5, 0.0],
        diffuse: 0xFFFF_FFFF,
        uv: [0.02, 0.02],
    },
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Motion {
    Still,
    Rise,
    SwayX,
    Circle,
    SwayXCreepZ,
    CreepXSwayZ,
    MoteX,
    MoteZ,
    MoteSpiral,
    Follow,
    Halo,
    Creep,
    DiagonalRise,
    Burst,
    Spin,
    SpinBack,
    LocalHalo,
    Travel,
    Unhandled(u32),
}

impl Motion {
    pub fn from_field(field: u32) -> Motion {
        match field {
            0 => Motion::Still,
            1 => Motion::Rise,
            2 => Motion::SwayX,
            3 => Motion::Circle,
            4 => Motion::SwayXCreepZ,
            5 => Motion::CreepXSwayZ,
            6 => Motion::MoteX,
            7 => Motion::MoteZ,
            8 => Motion::MoteSpiral,
            9 => Motion::Follow,
            0xA => Motion::Halo,
            0xB => Motion::Creep,
            0xC => Motion::DiagonalRise,
            0xD => Motion::Burst,
            0xE => Motion::Spin,
            0xF => Motion::SpinBack,
            0x10 => Motion::LocalHalo,
            0x11 => Motion::Travel,
            other => Motion::Unhandled(other),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Billboard {
    pub kind: usize,
    pub texture_frame: usize,
    pub frames: u32,
    pub frame_ms: u32,
    pub position: Vec3,
    pub world: Matrix4,
    pub pass: BillboardPass,
    pub quad: [BillboardVertex; 4],
    pub visible: bool,
    pub ready: bool,
    pub lifetime_ms: u32,
    pub birth_ms: Option<u32>,
    pub age: f32,
    pub scale: Vec3,
    pub growth: Vec3,
    pub colour_mode: u32,
    pub facing: bool,
    pub lift: bool,
    pub roll: f32,
    pub motion: Motion,
    pub origin: Vec3,
    pub rise: f32,
    pub amp_h: f32,
    pub phase: f32,
    pub dir: Vec3,
    pub axis: Vec3,
    pub spin: f32,
    pub glow: super::glow::Glow,
    pub base: [u8; 4],
    pub phase_seed: u32,
    pub pulse_floor: f32,
    pub sector: i32,
    pub owner: Option<super::motion::Owner>,
    pub owner_key: Option<super::effect_context::ActorKey>,
    pub conceal_with: Option<super::effect_context::ActorKey>,
    pub body_owned: Option<(super::effect_context::ActorKey, u16)>,
    pub(crate) held_disabled: bool,
}

impl Billboard {
    pub fn new(
        kind: usize,
        lifetime_ms: u32,
        scale: Vec3,
        growth: f32,
        frames: u32,
        frame_ms: u32,
    ) -> Billboard {
        Billboard {
            kind,
            texture_frame: 0,
            frames,
            frame_ms,
            position: Vec3::ZERO,
            world: Matrix4::IDENTITY,
            pass: BillboardPass::Blended,
            quad: UNIT_QUAD,
            visible: false,
            ready: true,
            lifetime_ms,
            birth_ms: None,
            age: 0.0,
            scale,
            growth: Vec3::splat(growth),
            colour_mode: 1,
            facing: true,
            lift: false,
            roll: 0.0,
            motion: Motion::Still,
            origin: Vec3::ZERO,
            rise: 2.0,
            amp_h: 1.0,
            phase: 6.0,
            dir: Vec3::ZERO,
            axis: Vec3::ZERO,
            spin: 0.0,
            glow: super::glow::Glow::Off,
            base: [0xFF; 4],
            phase_seed: 0,
            pulse_floor: PULSE_FLOOR,
            sector: SECTOR_UNASSIGNED,
            owner: None,
            owner_key: None,
            conceal_with: None,
            body_owned: None,
            held_disabled: false,
        }
    }

    pub fn set_color(&mut self, argb: u32) {
        for v in &mut self.quad {
            v.diffuse = argb;
        }

        self.base = argb.to_be_bytes();
    }

    pub fn update(&mut self, now_ms: u32, yaw: f32, pitch: f32) {
        self.visible = true;

        self.birth_ms.get_or_insert(now_ms);

        let elapsed = self.elapsed_ms(now_ms);

        if self.lifetime_ms != 0 && self.lifetime_ms <= elapsed {
            self.ready = false;

            return;
        }

        super::motion::owner_follow(
            self.motion,
            &self.motion_fields(now_ms),
            self.scale.y,
            &mut self.position,
        );

        let cycle = (self.frames * self.frame_ms).max(1);

        self.texture_frame = match self.frame_ms {
            0 => 0,
            frame_ms => ((elapsed % cycle) / frame_ms) as usize,
        };

        if self.lifetime_ms != 0 {
            self.age = elapsed as f32 / self.lifetime_ms as f32;
        }

        if self.age < 0.0 {
            self.age = AGE_FLOOR;
        }

        if self.age > 1.0 {
            self.age = 1.0;
        }

        let argb = self.colour(now_ms);

        for v in &mut self.quad {
            v.diffuse = argb;
        }

        let fields = self.motion_fields(now_ms);

        super::motion::apply(self.motion, &fields, &mut self.position, &mut self.spin);

        self.ready = visible_in_scene(30000, !self.held_disabled);

        if !self.ready {
            return;
        }

        self.world = self.world_matrix(elapsed as f32, yaw, pitch);

        if self.facing && self.glow.suppresses_facing() {
            let grown = if self.lifetime_ms == 0 {
                self.scale
            } else {
                self.scale + self.growth * elapsed as f32
            };

            let size = if let Some(glow) = super::glow::frame(self.glow, now_ms, grown) {
                self.set_color(glow.argb);

                glow.size
            } else {
                grown
            };

            let y = self.position.y + if self.lift { size.y * 0.5 } else { 0.0 };

            self.world = Matrix4::multiply(
                &Matrix4::scaling(size.x, size.y, size.z),
                &Matrix4::translation(self.position.x, y, self.position.z),
            );

            self.glow = super::glow::next_mode(self.glow, now_ms);
        }
    }

    pub fn motion_fields(&self, now_ms: u32) -> super::motion::Fields {
        super::motion::Fields {
            age: self.age,
            origin: self.origin,
            amp_h: self.amp_h,
            amp_v: self.rise,
            phase: self.phase,
            dir: self.dir,
            owner: self.owner,
            now_ms,
            elapsed_ms: self.elapsed_ms(now_ms),
        }
    }

    fn elapsed_ms(&self, now_ms: u32) -> u32 {
        let Some(birth) = self.birth_ms else {
            return 0;
        };

        let elapsed = now_ms.wrapping_sub(birth);

        if now_ms < birth && elapsed >= 0x8000_0000 {
            self.lifetime_ms
        } else {
            elapsed
        }
    }

    pub fn colour(&self, now_ms: u32) -> u32 {
        let [a, r, g, b] = self.base;

        let scaled = |c: u8, f: f32| ((c as f32 * f) as u32) & 0xFF;

        match self.colour_mode {
            1 => {
                let f = (std::f32::consts::PI * self.age).sin();

                scaled(a, f) << 24 | scaled(r, f) << 16 | scaled(g, f) << 8 | scaled(b, f)
            }
            2 => {
                let phase = (self.phase_seed % PULSE_PHASE_SEEDS) * PULSE_PHASE_STEP_MS;

                let t = phase.wrapping_add(now_ms) % PULSE_PERIOD_MS;

                let turn = std::f32::consts::TAU * t as f32 / PULSE_PERIOD_MS as f32;

                let f = self.pulse_floor + (1.0 - self.pulse_floor) * turn.sin().abs();

                scaled(a, f) << 24 | scaled(r, f) << 16 | scaled(g, f) << 8 | scaled(b, f)
            }
            3 => {
                let f = (std::f32::consts::PI * self.age).sin();

                scaled(a, f) << 24 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
            }
            4 => {
                let f = if self.age >= 0.3 {
                    1.0 - (self.age - 0.3) * 1.428
                } else {
                    self.age * 3.33
                };

                scaled(a, f) << 24 | u32::from(r) << 16 | u32::from(g) << 8 | u32::from(b)
            }
            _ => u32::from_be_bytes(self.base),
        }
    }

    pub fn world_matrix(&self, elapsed_ms: f32, yaw: f32, pitch: f32) -> Matrix4 {
        let scale = match self.lifetime_ms {
            0 => self.scale,
            _ => self.scale + self.growth * elapsed_ms,
        };

        let rotation = match self.facing {
            true => Matrix4::rotation_yaw_pitch_roll(
                std::f32::consts::FRAC_PI_2 - yaw,
                -pitch,
                self.roll,
            ),
            false => {
                let spun = Matrix4::rotation_y(self.roll);

                match self.motion {
                    Motion::Spin | Motion::SpinBack => {
                        Matrix4::multiply(&spun, &Matrix4::rotation_axis(self.axis, self.spin))
                    }
                    _ => spun,
                }
            }
        };

        let y = match self.lift {
            true => self.position.y + scale.y * 0.5,
            false => self.position.y,
        };

        Matrix4::multiply(
            &Matrix4::multiply(&rotation, &Matrix4::scaling(scale.x, scale.y, scale.z)),
            &Matrix4::translation(self.position.x, y, self.position.z),
        )
    }

    pub fn camera_distance(&self, eye: Vec3) -> f32 {
        self.camera_distance_squared(eye).sqrt()
    }

    pub fn camera_distance_squared(&self, eye: Vec3) -> f32 {
        self.position.distance_squared(Vec3::new(eye.x, 0.0, eye.z))
    }

    pub fn drawn(&self, eye: Vec3) -> bool {
        self.drawn_with_distance(eye, CULL_DISTANCE)
    }

    fn drawn_with_distance(&self, eye: Vec3, distance: f32) -> bool {
        self.visible
            && self.camera_distance_squared(eye) <= distance * distance
            && (self.lifetime_ms == 0 || self.age >= MIN_AGE)
            && self.ready
    }

    pub fn quality_allows(&self, detail: i32) -> bool {
        detail != 1 || (matches!(self.kind, 56 | 60 | 45) && self.colour_mode == 0)
    }

    pub fn texture_slot(&self) -> usize {
        self.kind + self.texture_frame
    }
}

pub const CLASS: Class = Class {
    name: "EffectBillboard",
    method_count: 18,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

impl Effect for Billboard {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        Billboard::update(self, tick.now_ms, tick.yaw, tick.pitch);
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        if let Some(key) = self.owner_key {
            self.owner = match (live.body)(key) {
                None => {
                    self.owner_key = None;

                    None
                }
                Some(body) => {
                    let mounted = body.appearance.is_some_and(|appearance| appearance.mounted);

                    if mounted && body.skill_anchor.is_none() {
                        None
                    } else {
                        Some(super::motion::Owner {
                            position: body.position,
                            seat: mounted.then_some(body.skill_anchor).flatten(),
                        })
                    }
                }
            };
        }

        Billboard::update(self, tick.now_ms, tick.yaw, tick.pitch);
    }

    fn expired(&self) -> bool {
        !self.ready && !(self.held_disabled && self.lifetime_ms == 0)
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        let group = self
            .body_owned
            .map(|(key, slot)| OwnedGroup::HumanAffect(key, slot));

        if !cx.scene.skill_vfx_visible(group)
            || !self.drawn_with_distance(cx.eye, cx.scene.billboard_distance())
            || !self.quality_allows(cx.scene.detail)
        {
            out.effect.culled += 1;

            return;
        }

        let slot = self.texture_slot();

        let Some((array, layer)) = cx.scene.placement(slot) else {
            out.effect.unbound += 1;

            return;
        };

        out.effect.push_quad(
            self.pass,
            BillboardUniforms::new(self.world, self.pass)
                .with_layer(layer)
                .with_opaque(cx.scene.alpha_at(slot)),
            &self.quad,
            array,
        );

        out.effect.billboards += 1;

        out.effect.submitted(&CLASS);
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        match self.ready {
            true => Cull::Kept,
            false => Cull::Rejected,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ArrayKey {
    pub width: u32,
    pub height: u32,
    pub format: formats::texture::PixelFormat,
    pub mips: usize,
}

impl ArrayKey {
    pub fn of(texture: &formats::texture::Texture) -> ArrayKey {
        ArrayKey {
            width: texture.width,
            height: texture.height,
            format: texture.format,
            mips: texture.mips.len(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TextureArray {
    pub key: ArrayKey,
    pub layers: Vec<usize>,
}

#[derive(Debug)]
pub struct BillboardScene {
    pub list: TextureList,
    pub textures: Vec<BoundTexture>,
    by_slot: FxHashMap<usize, usize>,
    by_path: FxHashMap<String, usize>,
    pub arrays: Vec<TextureArray>,
    placements: Vec<(usize, u32)>,
    pub billboards: Vec<Billboard>,
    pub detail: i32,
    pub environment: i32,
    pub scene_id: u32,
    pub view_distance: super::view_distance::ViewDistance,
    pub old_skill_vfx: bool,
    pub shades: Vec<Shade>,
    pub quads: Vec<super::quads::GroundQuad>,
    pub tree: Tree,
    pub missing: Vec<usize>,
    pub skipped: Vec<(usize, Skipped)>,
}

impl BillboardScene {
    pub fn build(game: &GameDir) -> Result<BillboardScene> {
        let list = texture_list::parse(
            &game
                .read(EFFECT_TEXTURE_LIST)
                .context("reading Effect\\EffectTextureList.bin")?,
        )
        .context("parsing EffectTextureList.bin")?;

        Ok(BillboardScene::with_list(list))
    }

    pub fn empty() -> BillboardScene {
        BillboardScene::with_list(
            texture_list::parse(&[]).expect("an empty table has no records to reject"),
        )
    }

    fn with_list(list: TextureList) -> BillboardScene {
        BillboardScene {
            list,
            textures: Vec::new(),
            by_slot: FxHashMap::default(),
            by_path: FxHashMap::default(),
            arrays: Vec::new(),
            placements: Vec::new(),
            billboards: Vec::new(),
            detail: 0,
            environment: 0,
            scene_id: 0,
            view_distance: super::view_distance::ViewDistance::Retail,
            old_skill_vfx: true,
            shades: Vec::new(),
            quads: Vec::new(),
            tree: Tree::default(),
            missing: Vec::new(),
            skipped: Vec::new(),
        }
    }

    pub fn spawn(&mut self, node: Box<dyn Effect>) {
        self.tree.spawn(node);
    }

    pub(crate) fn skill_vfx_visible(&self, group: Option<OwnedGroup>) -> bool {
        visible_in_scene(
            self.scene_id,
            self.old_skill_vfx || group.is_none_or(|group| !group.uses_old_skill_vfx()),
        )
    }

    fn billboard_distance(&self) -> f32 {
        CULL_DISTANCE
            * if self.scene_id == super::world::SCENE_FIELD {
                self.view_distance.multiplier()
            } else {
                1.0
            }
    }

    pub fn bind_slot(&mut self, game: &GameDir, slot: usize) -> Option<usize> {
        if let Some(&i) = self.by_slot.get(&slot) {
            return Some(i);
        }

        let miss = |scene: &mut BillboardScene| {
            if !scene.missing.contains(&slot) {
                scene.missing.push(slot);
            }

            None
        };

        let Some(entry) = self.list.get(slot) else {
            return miss(self);
        };

        let (path, flag, path_str) = (entry.path.clone(), entry.format_flag, entry.path_str());

        let Some(resolved) = game.resolve(&path).map(|p| p.display().to_string()) else {
            return miss(self);
        };

        if let Some(&i) = self.by_path.get(&resolved) {
            self.by_slot.insert(slot, i);

            return Some(i);
        }

        let Ok(bytes) = game.read(&path) else {
            return miss(self);
        };

        let Ok(texture) = formats::texture::decode(&bytes) else {
            return miss(self);
        };

        let index = self.push_texture(BoundTexture {
            mip_policy_override: None,
            prepared: None,
            pending: false,
            effect_table: true,
            slot,
            path: path_str,
            flag,
            texture: texture.into(),
        });

        self.by_path.insert(resolved, index);

        Some(index)
    }

    pub fn push_texture(&mut self, texture: BoundTexture) -> usize {
        let index = self.textures.len();

        let key = ArrayKey::of(&texture.texture);

        let array = match self.arrays.iter().position(|a| a.key == key) {
            Some(a) => a,
            None => {
                self.arrays.push(TextureArray {
                    key,
                    layers: Vec::new(),
                });

                self.arrays.len() - 1
            }
        };

        let layer = self.arrays[array].layers.len() as u32;

        self.arrays[array].layers.push(index);

        self.placements.push((array, layer));

        self.by_slot.insert(texture.slot, index);

        self.textures.push(texture);

        index
    }

    pub fn texture_index(&self, slot: usize) -> Option<usize> {
        self.by_slot.get(&slot).copied()
    }

    pub fn placement(&self, slot: usize) -> Option<(usize, u32)> {
        let index = *self.by_slot.get(&slot)?;

        self.placements.get(index).copied()
    }

    pub fn alpha_at(&self, slot: usize) -> AlphaPolicy {
        match self.by_slot.get(&slot).and_then(|i| self.textures.get(*i)) {
            Some(texture) => texture.alpha_policy(),
            None => AlphaPolicy::Stored,
        }
    }

    pub fn make_resident(&mut self, game: &GameDir) {
        for slot in 0..self.list.len() {
            if self.list.get(slot).is_none() {
                continue;
            }

            if let Some(why) = skipped(slot) {
                self.skipped.push((slot, why));

                continue;
            }

            self.bind_slot(game, slot);
        }
    }

    pub fn skipped_count(&self, why: Skipped) -> usize {
        self.skipped.iter().filter(|(_, w)| *w == why).count()
    }

    pub fn resident_bytes(&self) -> usize {
        self.textures
            .iter()
            .flat_map(|t| t.texture.mips.iter().map(|m| m.len))
            .sum()
    }

    pub fn upload_arrays(&self, gpu: &Gpu, renderer: &BillboardRenderer) -> Vec<BillboardArray> {
        let mut bound = Vec::with_capacity(self.arrays.len());

        for (i, array) in self.arrays.iter().enumerate() {
            let layers: Vec<&formats::texture::Texture> = array
                .layers
                .iter()
                .filter_map(|&l| self.textures.get(l))
                .map(|t| t.texture.as_ref())
                .collect();

            let Some(uploaded) = render::texture::upload_array(
                gpu,
                &layers,
                render::texture::AlphaPolicy::Stored,
                &format!(
                    "billboard array {i} ({}x{})",
                    array.key.width, array.key.height
                ),
            ) else {
                tracing::info!(
                    "NWYD: effect texture array {i} ({}x{}, {:?}, {} mip(s), {} layer(s)) did not \
                     upload; {} later array(s) dropped",
                    array.key.width,
                    array.key.height,
                    array.key.format,
                    array.key.mips,
                    array.layers.len(),
                    self.arrays.len() - i - 1,
                );

                break;
            };

            bound.push(renderer.bind_array(gpu, &uploaded.view));
        }

        bound
    }

    pub fn update(&mut self, now_ms: u32, yaw: f32, pitch: f32) -> usize {
        self.update_with_resolver(now_ms, yaw, pitch, None, |_| {}, None)
    }

    pub fn needs_ground(&self) -> bool {
        self.tree.needs_ground()
    }

    pub fn update_with_entities(
        &mut self,
        now_ms: u32,
        yaw: f32,
        pitch: f32,
        position_of: impl FnMut(u16) -> Option<Vec3>,
    ) -> usize {
        self.update_with_resolver(
            now_ms,
            yaw,
            pitch,
            None,
            |tree| {
                tree.resolve_tracked_entities(position_of);
            },
            None,
        )
    }

    pub fn update_with_entities_grounded(
        &mut self,
        now_ms: u32,
        yaw: f32,
        pitch: f32,
        ground: &super::shade::Ground<'_>,
        position_of: impl FnMut(u16) -> Option<Vec3>,
    ) -> usize {
        self.update_with_resolver(
            now_ms,
            yaw,
            pitch,
            Some(ground),
            |tree| {
                tree.resolve_tracked_entities(position_of);
            },
            None,
        )
    }

    pub fn update_live(
        &mut self,
        now_ms: u32,
        live: &mut super::effect_context::Live<'_>,
        position_of: impl FnMut(u16) -> Option<Vec3>,
    ) -> usize {
        self.detail = live.detail;

        self.update_with_resolver(
            now_ms,
            live.camera.yaw,
            live.camera.pitch,
            None,
            |tree| {
                tree.resolve_tracked_entities(position_of);
            },
            Some(live),
        )
    }

    fn update_with_resolver(
        &mut self,
        now_ms: u32,
        yaw: f32,
        pitch: f32,
        ground: Option<&super::shade::Ground<'_>>,
        resolve: impl FnOnce(&mut Tree),
        live: Option<&mut super::effect_context::Live<'_>>,
    ) -> usize {
        let tick = Tick { now_ms, yaw, pitch };

        let before = self.billboards.len() + self.shades.len() + self.quads.len();

        for billboard in &mut self.billboards {
            Effect::update(billboard, &tick);
        }

        for shade in &mut self.shades {
            Effect::update(shade, &tick);
        }

        for quad in &mut self.quads {
            Effect::update(quad, &tick);
        }

        self.billboards.retain(|b| !b.expired());

        self.shades.retain(|s| !s.expired());

        self.quads.retain(|q| !q.expired());

        let dropped = before - (self.billboards.len() + self.shades.len() + self.quads.len());

        resolve(&mut self.tree);

        dropped
            + match live {
                Some(live) => self.tree.update_live(&tick, live),
                None => match ground {
                    Some(ground) => self.tree.update_grounded(&tick, ground),
                    None => self.tree.update(&tick),
                },
            }
    }

    pub fn take_sounds(&mut self) -> Vec<i64> {
        self.tree.take_sounds()
    }

    pub fn seeded(&self, now_ms: u32, yaw: f32, pitch: f32) -> Vec<Billboard> {
        let mut seeded = self.billboards.clone();

        for billboard in &mut seeded {
            billboard.update(now_ms, yaw, pitch);
        }

        seeded
    }

    pub fn frame(&self, reach: super::actors::Reach, now_ms: u32) -> Frame {
        let mut frame = Frame::default();

        self.frame_into(&mut frame, reach, now_ms);

        frame
    }

    pub fn frame_into(&self, out: &mut Frame, reach: super::actors::Reach, now_ms: u32) {
        out.clear();

        let mut mesh = Vec::new();

        let cx = Submit {
            scene: self,
            eye: reach.eye,
            now_ms,
        };

        let mut sink = Sink {
            effect: out,
            mesh: &mut mesh,
        };

        self.submit_billboards(&self.billboards, &cx, &mut sink);

        self.submit_shades(reach, &cx, &mut sink);

        self.submit_quads(&self.quads, reach, &cx, &mut sink);

        self.tree.submit(Pass::Effect, &cx, &mut sink);

        debug_assert!(
            mesh.is_empty(),
            "a Pass::Effect class writes into Sink::effect alone; see BillboardScene::mesh_frame",
        );
    }

    pub fn mesh_frame(&self, eye: Vec3, now_ms: u32) -> Vec<super::effect_tree::MeshSubmission> {
        let mut frame = Frame::default();

        let mut mesh = Vec::new();

        let cx = Submit {
            scene: self,
            eye,
            now_ms,
        };

        self.tree.submit(
            Pass::Mesh,
            &cx,
            &mut Sink {
                effect: &mut frame,
                mesh: &mut mesh,
            },
        );

        debug_assert_eq!(
            frame,
            Frame::default(),
            "a Pass::Mesh class writes into Sink::mesh alone; see BillboardScene::frame",
        );

        mesh
    }

    pub fn frame_of(&self, billboards: &[Billboard], eye: Vec3) -> Frame {
        let mut frame = Frame::default();

        let mut mesh = Vec::new();

        let cx = Submit {
            scene: self,
            eye,
            now_ms: 0,
        };

        let mut sink = Sink {
            effect: &mut frame,
            mesh: &mut mesh,
        };

        self.submit_billboards(billboards, &cx, &mut sink);

        self.submit_shades(super::actors::Reach { eye, far: None }, &cx, &mut sink);

        debug_assert!(mesh.is_empty(), "neither class writes into the mesh pass");

        frame
    }

    fn submit_billboards(&self, billboards: &[Billboard], cx: &Submit<'_>, sink: &mut Sink<'_>) {
        for billboard in billboards {
            billboard.submit(cx, sink);
        }

        debug_assert_eq!(
            sink.effect.vertices.len(),
            sink.effect.objects.len() * QUAD_VERTICES as usize,
            "the renderer indexes the quads' vertex buffer by object",
        );
    }

    fn submit_shades(&self, reach: super::actors::Reach, cx: &Submit<'_>, sink: &mut Sink<'_>) {
        for shade in &self.shades {
            if !reach.admits(shade.centre) {
                sink.effect.culled += 1;

                continue;
            }

            shade.submit(cx, sink);
        }
    }

    fn submit_quads(
        &self,
        quads: &[super::quads::GroundQuad],
        reach: super::actors::Reach,
        cx: &Submit<'_>,
        sink: &mut Sink<'_>,
    ) {
        for quad in quads {
            if !reach.admits(quad.anchor) {
                sink.effect.culled += 1;

                continue;
            }

            quad.submit(cx, sink);
        }
    }

    pub fn append_quads_of(
        &self,
        frame: &mut Frame,
        quads: &[super::quads::GroundQuad],
        now_ms: u32,
    ) {
        let mut mesh = Vec::new();

        let cx = Submit {
            scene: self,
            eye: Vec3::ZERO,
            now_ms,
        };

        let open = super::actors::Reach {
            eye: Vec3::ZERO,
            far: None,
        };

        self.submit_quads(
            quads,
            open,
            &cx,
            &mut Sink {
                effect: frame,
                mesh: &mut mesh,
            },
        );

        debug_assert!(mesh.is_empty(), "EffectBillboard2 draws in the effect pass");
    }

    pub fn append_trails_of(&self, frame: &mut Frame, trails: &[super::swing::Ribbon]) {
        for ribbon in trails {
            let Some((array, layer)) = self.placement(ribbon.texture as usize) else {
                frame.unbound += 1;

                continue;
            };

            let object = frame.objects.len() as u32;

            frame.objects.push(
                BillboardUniforms::new(Matrix4::IDENTITY, BillboardPass::Additive)
                    .with_layer(layer)
                    .with_opaque(self.alpha_at(ribbon.texture as usize)),
            );

            let base_vertex = frame.vertices.len() as u32;

            let first = frame.indices.len() as u32;

            frame
                .vertices
                .extend_from_slice(&ribbon.billboard_vertices());

            frame
                .indices
                .extend_from_slice(&super::swing::TRAIL_INDICES);

            frame.draws.push(BillboardDraw {
                pass: BillboardPass::Additive,
                object,
                texture: array,
                base_vertex,
                indices: first..frame.indices.len() as u32,
            });

            frame.submitted(super::swing::TRAIL_CLASS);
        }
    }

    pub fn append_weather_of(&self, frame: &mut Frame, fall: &super::weather::Fall<'_>) {
        if !super::rain::DRAW_GATE.admits(self.detail) {
            return;
        }

        let (right, up) = super::weather::billboard_axes(fall.view);

        for (on, snow) in [
            (fall.emitters.snow_fast, fall.snow[1]),
            (fall.emitters.snow, fall.snow[0]),
        ] {
            if !on {
                continue;
            }

            let Some((array, layer)) = self.placement(super::snow::TEXTURE_SLOT) else {
                frame.unbound += 1;

                continue;
            };

            let opaque = self.alpha_at(super::snow::TEXTURE_SLOT);

            for flake in &snow.flakes {
                if super::snow::roofed(fall.map, flake.x, flake.z) {
                    continue;
                }

                let alpha = snow.alpha(flake, fall.anchor.floor);

                let world = Matrix4::translation(flake.x, flake.y, flake.z);

                frame.push_quad(
                    BillboardPass::Additive,
                    BillboardUniforms::new(world, BillboardPass::Additive)
                        .with_layer(layer)
                        .with_opaque(opaque),
                    &snow.quad(alpha, right, up),
                    array,
                );

                frame.submitted(super::snow::SNOW_CLASS);
            }
        }

        if !fall.emitters.rain {
            return;
        }

        let Some((array, layer)) = self.placement(super::rain::TEXTURE_SLOT) else {
            frame.unbound += 1;

            return;
        };

        let opaque = self.alpha_at(super::rain::TEXTURE_SLOT);

        let quad = fall.rain.quad(right, up);

        for drop in &fall.rain.drops {
            if !fall.rain.draws(drop, fall.anchor, fall.map) {
                continue;
            }

            let [x, y, z] = drop.world(fall.anchor);

            frame.push_quad(
                BillboardPass::Additive,
                BillboardUniforms::new(Matrix4::translation(x, y, z), BillboardPass::Additive)
                    .with_layer(layer)
                    .with_opaque(opaque),
                &quad,
                array,
            );

            frame.submitted(super::rain::RAIN_CLASS);
        }
    }

    pub fn append_drips(&self, frame: &mut Frame, drips: &[super::drip::Column]) {
        if drips.is_empty() {
            return;
        }

        let Some((array, layer)) = self.placement(super::drip::TEXTURE_SLOT) else {
            frame.unbound += 1;

            return;
        };

        let opaque = self.alpha_at(super::drip::TEXTURE_SLOT);

        let quad = super::drip::template();

        for column in drips {
            for at in column.translations() {
                frame.push_quad(
                    BillboardPass::Additive,
                    BillboardUniforms::new(
                        Matrix4::translation(at.x, at.y, at.z),
                        BillboardPass::Additive,
                    )
                    .with_layer(layer)
                    .with_opaque(opaque),
                    &quad,
                    array,
                );

                frame.submitted(super::drip::DROP_CLASS);
            }
        }
    }
}

pub fn bucket_bounds(billboards: &[Billboard], eye: Vec3) -> (usize, usize) {
    let inside: Vec<i32> = billboards
        .iter()
        .filter(|b| b.drawn(eye))
        .map(|b| b.sector)
        .collect();

    let (cx, cz) = (eye.x as i32 >> 5, eye.z as i32 >> 5);

    let mut counts = Vec::with_capacity(4);

    for sx in [-1, 1] {
        for sz in [-1, 1] {
            let cells = [
                (cz << 16) | cx,
                (cz << 16) | (cx + sx),
                ((cz + sz) << 16) | cx,
                ((cz + sz) << 16) | (cx + sx),
            ];

            counts.push(
                inside
                    .iter()
                    .filter(|c| **c == SECTOR_UNASSIGNED || cells.contains(c))
                    .count(),
            );
        }
    }

    (
        counts.iter().copied().min().unwrap_or(0),
        counts.iter().copied().max().unwrap_or(0),
    )
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Frame {
    pub objects: Vec<BillboardUniforms>,
    pub vertices: Vec<BillboardVertex>,
    pub indices: Vec<u16>,
    pub draws: Vec<BillboardDraw>,
    pub billboards: usize,
    pub shades: usize,
    pub quads: usize,
    pub culled: usize,
    pub unbound: usize,
    pub shade_triangles: usize,
    pub counts: Vec<(&'static str, usize)>,
}

impl Frame {
    pub fn clear(&mut self) {
        let Frame {
            objects,
            vertices,
            indices,
            draws,
            billboards,
            shades,
            quads,
            culled,
            unbound,
            shade_triangles,
            counts,
        } = self;

        objects.clear();

        vertices.clear();

        indices.clear();

        draws.clear();

        counts.clear();

        *billboards = 0;

        *shades = 0;

        *quads = 0;

        *culled = 0;

        *unbound = 0;

        *shade_triangles = 0;
    }

    pub fn push_quad(
        &mut self,
        pass: BillboardPass,
        uniforms: BillboardUniforms,
        vertices: &[BillboardVertex; 4],
        texture: usize,
    ) {
        let object = self.objects.len() as u32;

        self.objects.push(uniforms);

        let base_vertex = self.vertices.len() as u32;

        let first = self.indices.len() as u32;

        self.vertices.extend_from_slice(vertices);

        self.indices.extend_from_slice(&QUAD_INDICES);

        self.draws.push(BillboardDraw {
            pass,
            object,
            texture,
            base_vertex,
            indices: first..self.indices.len() as u32,
        });
    }

    pub fn submitted(&mut self, class: &super::effect_tree::Class) {
        if let Some((name, n)) = self.counts.last_mut()
            && *name == class.name
        {
            *n += 1;

            return;
        }

        match self.counts.iter_mut().find(|(name, _)| *name == class.name) {
            Some((_, n)) => *n += 1,
            None => self.counts.push((class.name, 1)),
        }
    }

    pub fn count_of(&self, name: &str) -> usize {
        self.counts
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(0, |(_, n)| *n)
    }
}
