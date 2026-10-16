use super::aura;
use super::butterflies;
use super::corpse;
use super::fish;
use super::floats;
use super::grass;
use super::objects::BoundTexture;
use super::registries::Registries;
use super::rng::Lcg;
use super::swing::{self, Swing};
use super::trees;
use super::walk;
use anyhow::{Context, Result};
use assets::GameDir;
use formats::audio::ani_sound::AniSound;
use formats::data::item_list;
use formats::mesh::appearance::{Look, PartRef};
use formats::mesh::costume;
use formats::mesh::glow;
use formats::mesh::model::{ModelAnimation, Slot};
use formats::mesh::msh::{self, SkinMesh};
use formats::mesh::pose::{ClipTable, FrameTree, PoseRequest, Smoothing};
use formats::mesh::registry::{ModelEntry, ValidIndex};
use formats::mesh::{ani, appearance, bon, registry};
use formats::net::account;
use formats::net::affect;
use formats::net::animation_probe;
use formats::scene::demo::{self, Roster};
use formats::texture::texture_list::{self, TextureList};
use formats::world::objects::ObjRecord;
use formats::world::path::{self, HeightField};
use formats::world::trn::{HEIGHT_GRID_DIM, Terrain};
use formats::world::{HEIGHT_SCALE, IMPASSABLE, ZONE_CELLS};
use formats::world::{attribute_map, collision, object_bin};
use glam::Vec3;
use render::camera::{self, Matrix4};
use render::device::Gpu;
use render::skin::{
    BonePalette, SkinArena, SkinDraw, SkinFog, SkinObject, SkinParams, SkinPass, SkinRenderer,
    SkinTexture,
};
use std::collections::BTreeMap;
use std::sync::Arc;

pub const ROSTER: &[u8] = br"UI\demo.bin";

pub const EFFECT_RNG_SEED: u32 = 1;

pub const RECENT_HIT_MS: u32 = 100;

pub const ROSTERS: [&[u8]; 4] = [
    br"UI\demo.bin",
    br"UI\demo2.bin",
    br"UI\demo3.bin",
    br"UI\demo4.bin",
];

pub fn roster_for(variant: u32) -> Option<&'static [u8]> {
    ROSTERS.get(variant as usize).copied()
}

pub const DEMO_ANIM_GROUP: usize = 0;

fn idle_anim(clips: &ActionClips, loaded: &Model) -> Anim {
    match clips.get(IDLE_ACTION).0 {
        Some(clip) => Anim::new(clip.slot, clip.sub_frame_ms),
        None => Anim::new(loaded.slot, loaded.sub_frame_ms),
    }
}

fn anim_group(look: &Look, main: i32, off: i32, items: &item_list::ItemList) -> usize {
    appearance::anim_group(
        look.class,
        look.model.unwrap_or(0),
        look.is_mounted(),
        look.parts[0].mesh,
        main,
        off,
        items,
    )
    .unwrap_or(DEMO_ANIM_GROUP)
}

pub const BONE_ANI: &[u8] = br"mesh\BoneAni4.txt";

pub const VALID_INDEX: &[u8] = br"mesh\ValidIndex.bin";

pub const ANI_SOUND: &[u8] = b"AniSound4.txt";

pub const OBJECT_BIN: &[u8] = b"object.bin";

pub const ATTRIBUTE_MAP: &[u8] = br"Env\AttributeMap.dat";

pub const IDLE_ACTION: usize = 1;

pub const AIR_MOVE_MOUNT_MODEL: u32 = 0x28;

pub const AIR_MOVE_ACTION: usize = appearance::ACTION_SEATING;

pub const DEMO_BODY_SCALE: f32 = 1.0;

pub const DEFAULT_SUB_FRAME_MS: u32 = 30;

pub const MATERIAL_DIFFUSE: [f32; 4] = [1.0, 1.0, 1.0, 0.0];

pub const MATERIAL_AMBIENT: [f32; 4] = [0.0; 4];

pub const MATERIAL_EMISSIVE: [f32; 4] = [0.3, 0.3, 0.3, 0.0];

pub const HIGHLIGHT_DIFFUSE: [f32; 4] = [0.5, 0.5, 0.5, MATERIAL_DIFFUSE[3]];

pub const RECENT_HIT_DIFFUSE: [f32; 4] = [0.7, 0.7, 0.9, MATERIAL_DIFFUSE[3]];

pub const HIGHLIGHT_EMISSIVE: [f32; 4] = [0.5, 0.5, 0.5, MATERIAL_EMISSIVE[3]];

pub const REACH_MARGIN: f32 = 1.5;

#[derive(Debug, Clone, Copy)]
pub struct Reach {
    pub eye: Vec3,
    pub far: Option<f32>,
}

impl Reach {
    pub fn at(eye: Vec3) -> Reach {
        Reach { eye, far: None }
    }

    pub fn with_projection(mut self, projection: &Matrix4) -> Self {
        let m = projection.as_d3dx();

        let diagonal = (1.0 + m[0].recip().powi(2) + m[5].recip().powi(2)).sqrt();

        if !diagonal.is_finite() {
            self.far = None;
        } else {
            self.far = self.far.map(|far| far * (diagonal / REACH_MARGIN).max(1.0));
        }

        self
    }

    pub fn admits(&self, at: Vec3) -> bool {
        self.admits_within(at, 0.0)
    }

    pub fn admits_within(&self, at: Vec3, radius: f32) -> bool {
        self.far.is_none_or(|far| {
            let (dx, dz) = (at.x - self.eye.x, at.z - self.eye.z);

            let reach = far * REACH_MARGIN + radius;

            dx * dx + dz * dz <= reach * reach
        })
    }
}

#[derive(Debug)]
pub struct Part {
    pub index: usize,
    pub file_name: String,
    pub mesh_path: String,
    pub mesh: Arc<SkinMesh>,
    pub alloc: usize,
    pub texture_name: String,
    pub texture: Option<usize>,
}

#[derive(Debug)]
pub struct Model {
    pub model_id: u32,
    pub base_name: String,
    pub animation: Arc<ModelAnimation>,
    pub parts: Vec<Part>,
    pub slot: usize,
    pub sub_frame_ms: u32,
    pub missing_parts: Vec<String>,
}

impl Model {
    pub fn from_action(
        sounds: &AniSound,
        slots: &registry::AnimSlotTable,
        model_id: u32,
        group: usize,
        action: usize,
    ) -> Option<(usize, u32)> {
        Model::from_actions(sounds, slots, model_id, group, action, action)
    }

    pub fn from_mounted_action(
        sounds: &AniSound,
        slots: &registry::AnimSlotTable,
        model_id: u32,
        group: usize,
        action: usize,
    ) -> Option<(usize, u32)> {
        let clip_action = appearance::mounted_action(action);

        Model::from_actions(sounds, slots, model_id, group, clip_action, action)
    }

    pub fn clip_for(
        sounds: &AniSound,
        slots: &registry::AnimSlotTable,
        model_id: u32,
        group: usize,
        action: usize,
        mounted: bool,
    ) -> Option<Clip> {
        let clip_action = if mounted {
            appearance::mounted_action(action)
        } else {
            action
        };

        let (slot, sub_frame_ms) =
            Model::from_actions(sounds, slots, model_id, group, clip_action, action)?;

        let code = *sounds.class(model_id as usize)?.frame.get(clip_action)?;

        Some(Clip {
            slot,
            sub_frame_ms,
            code,
        })
    }

    fn direct_clip_for(sounds: &AniSound, model_id: u32, action: usize) -> Option<Clip> {
        let table = sounds.class(model_id as usize)?;

        let code = *table.frame.get(action)?;

        let ms = *table.speed.get(action)?;

        Some(Clip {
            slot: usize::try_from(code).ok()?,
            sub_frame_ms: if ms > 0 {
                ms as u32
            } else {
                DEFAULT_SUB_FRAME_MS
            },
            code,
        })
    }

    fn from_actions(
        sounds: &AniSound,
        slots: &registry::AnimSlotTable,
        model_id: u32,
        group: usize,
        clip_action: usize,
        speed_action: usize,
    ) -> Option<(usize, u32)> {
        let table = sounds.class(model_id as usize)?;

        let code = *table.frame.get(clip_action)?;

        let ms = *table.speed.get(speed_action)?;

        let ms = if ms > 0 {
            ms as u32
        } else {
            DEFAULT_SUB_FRAME_MS
        };

        let code = usize::try_from(code).ok()?;

        let slot = if registry::ANIM_SLOT_MODELS.contains(&model_id) {
            usize::from(slots.get(group, code)?)
        } else {
            code
        };

        Some((slot, ms))
    }

    pub fn frames(&self) -> usize {
        self.animation.frames(self.slot).unwrap_or(0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Clip {
    pub slot: usize,
    pub sub_frame_ms: u32,
    pub code: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Anim {
    pub slot: usize,
    pub sub_frame_ms: u32,
    pub start_ms: u32,
    pub blend_from: u32,
    pub blend_matrix: usize,
    pub frame: usize,
}

impl Anim {
    pub fn new(slot: usize, sub_frame_ms: u32) -> Anim {
        Anim {
            slot,
            sub_frame_ms,
            start_ms: 0,
            blend_from: 0,
            blend_matrix: 0,
            frame: 0,
        }
    }

    pub fn set_clip(&mut self, slot: usize, now_ms: u32, model: &Model) -> bool {
        if slot == self.slot || model.animation.frames(slot).is_none() {
            return false;
        }

        if model.model_id < 100
            && self.slot < 186
            && let Some(frames) = model.animation.frames(self.slot)
        {
            self.frame = self.frame.min(frames.saturating_sub(1));
        }

        self.blend_from = self.slot as u32;

        self.blend_matrix = (self.frame + model.animation.anim_base(self.slot).unwrap_or(0))
            * model.animation.ani_num_bones;

        self.start_ms = now_ms;

        self.slot = slot;

        true
    }

    pub fn play(&mut self, clip: Clip, now_ms: u32, model: &Model, guarded: bool) -> bool {
        if !self.set_clip(clip.slot, now_ms, model) && guarded {
            return clip.code == 0;
        }

        self.sub_frame_ms = clip.sub_frame_ms;

        true
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AlternateHp {
    pub current: u32,
    pub max: u32,
    pub base_max: u32,
}

impl AlternateHp {
    pub fn active(&self) -> bool {
        self.max != 0
    }

    pub fn restore(&mut self) {
        self.current = self.base_max;

        self.max = self.base_max;
    }

    pub fn hurt(&mut self, damage: i32) -> i32 {
        self.current = self.current.wrapping_sub(damage as u32);

        i32::from(self.current as u16 as i16)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetHud {
    pub eligible: bool,
    pub hp_color: u32,
}

pub const TARGET_HP_DARK_RED: u32 = 0xFF80_0000;

impl Default for TargetHud {
    fn default() -> Self {
        Self {
            eligible: false,
            hp_color: TARGET_HP_DARK_RED,
        }
    }
}

#[derive(Debug)]
pub struct Actor {
    pub(crate) incarnation: u64,
    pub skill_anchor: Option<Vec3>,
    pub camera_visible: bool,
    pub model: usize,
    pub model_id: u32,
    pub record: usize,
    pub entity: u16,
    pub name: String,
    pub status: account::Status,
    pub alternate_hp: Option<AlternateHp>,
    pub mount_hp_bonus: i16,
    pub name_level: u8,
    pub level: u16,
    pub guild: u16,
    pub guild_rank: u16,
    pub charidx: Option<u32>,
    pub guild_name: String,
    pub body: i32,
    pub helmet_item: i32,
    pub kingdom: u8,
    pub costume_item: u16,
    pub costume_effect_item: i32,
    pub body_item_tier: u8,
    pub pet_item: u16,
    pub position: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
    pub anim: Anim,
    pub action: usize,
    pub emote_latch: Option<i16>,
    pub emote_hold: bool,
    pub(crate) spin: Option<super::attack::Spin>,
    pub(crate) completion_suspended: bool,
    pub group: usize,
    pub weapon_class: i32,
    pub off_weapon_class: i32,
    pub main_weapon_item: i32,
    pub off_weapon_item: i32,
    pub active_hands: [bool; 2],
    pub primary_tier: u8,
    pub skill_critical: bool,
    pub skill_child: Option<u16>,
    pub recent_hit_ms: Option<u32>,
    pub pending_hit: Option<super::hit_reaction::Pending>,
    pub haste_trail_ms: u32,
    pub motion_effects: super::actor_effects::MotionState,
    pub impact_grid_ms: u32,
    pub affect_pair_ms: u32,
    pub body_cloud_ms: u32,
    pub body_shadow: super::body_shadows::State,
    pub body_sprites: super::body_sprites::State,
    pub footprint_kind: u8,
    pub footprint_ms: u32,
    pub movement_dust: super::movement_dust::State,
    pub body_glow: super::body_glow::State,
    pub body_points: super::body_points::State,
    pub body_butterflies: Option<butterflies::OwnedFlight>,
    pub affect_emit_ms: [u32; 4],
    pub afterimage_ms: u32,
    pub skill_afterimage_window: Option<u32>,
    pub skill_afterimage_ms: u32,
    pub transformation: super::transformation::State,
    pub appearance_epoch: u32,
    pub projectile_muzzle: Vec3,
    pub clips: ActionClips,
    pub speed: f32,
    pub is_dead: bool,
    pub tracked: bool,
    pub party_tint: Option<u32>,
    pub green_name: bool,
    pub plate_outline: bool,
    pub plate_backdrop_color: u32,
    pub plate_name_color: u32,
    pub corpse_ms: Option<u32>,
    pub removal_mode: i32,
    pub shop_title: String,
    pub balloon: Option<super::balloon::Balloon>,
    pub highlight: bool,
    pub target_hud: TargetHud,
    pub destination: (i32, i32),
    pub mover: walk::Mover,
    pub scale: f32,
    pub evok_size: u8,
    pub mount_model: Option<u32>,
    pub mount_scale: f32,
    pub mount_kind: i16,
    pub mounted: bool,
    pub parts: [PartRef; appearance::PARTS],
    pub class: i32,
    pub forced_weather_ms: u32,
    pub glow: [Option<PartGlow>; appearance::PARTS],
    pub affects: [affect::Affect; affect::AFFECTS],
    pub details: [affect::Detail; affect::AFFECTS],
    pub flags: affect::Flags,
    pub tree: FrameTree,
    pub cape_anchor: &'static [i32],
    pub cape_flag: Option<appearance::Cape>,
    pub cape: Option<Cape>,
    pub mount: Option<Mount>,
    pub held: Vec<Held>,
    pub swings: Vec<Swing>,
    pub volley: super::skill_fx::Volley,
    pub pending: Option<super::skill_fx::Pending>,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct ActionClips {
    pub stand: Option<Clip>,
    pub walk: Option<Clip>,
    pub run: Option<Clip>,
    pub die: Option<Clip>,
    pub dead: Option<Clip>,
    pub attack1: Option<Clip>,
    pub attack2: Option<Clip>,
    pub attack3: Option<Clip>,
    pub mount_stand: Option<Clip>,
    pub mount_walk: Option<Clip>,
    pub mount_run: Option<Clip>,
    pub mount_die: Option<Clip>,
    pub mount_dead: Option<Clip>,
    pub mount_attack1: Option<Clip>,
    pub mount_attack2: Option<Clip>,
    pub mount_attack3: Option<Clip>,
    pub skill01: Option<Clip>,
    pub skill02: Option<Clip>,
    pub skill03: Option<Clip>,
    pub mount_skill01: Option<Clip>,
    pub mount_skill02: Option<Clip>,
    pub mount_skill03: Option<Clip>,
    pub strike: Option<Clip>,
    pub mount_strike: Option<Clip>,
    pub emotes: [Option<Clip>; appearance::EMOTE_ACTIONS],
    pub mount_emotes: [Option<Clip>; appearance::EMOTE_ACTIONS],
    pub extra: Option<Extra>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Extra {
    pub action: usize,
    pub body: Option<Clip>,
    pub mount: Option<Clip>,
}

impl ActionClips {
    pub fn get(&self, action: usize) -> (Option<Clip>, Option<Clip>) {
        if let Some(extra) = self.extra.filter(|e| e.action == action) {
            return (extra.body, extra.mount);
        }

        match action {
            appearance::ACTION_WALK => (self.walk, self.mount_walk),
            appearance::ACTION_RUN => (self.run, self.mount_run),
            appearance::ACTION_DIE => (self.die, self.mount_die),
            appearance::ACTION_DEAD => (self.dead, self.mount_dead),
            appearance::ACTION_ATTACK1 => (self.attack1, self.mount_attack1),
            appearance::ACTION_ATTACK2 => (self.attack2, self.mount_attack2),
            appearance::ACTION_ATTACK3 => (self.attack3, self.mount_attack3),
            appearance::ACTION_SKILL01 => (self.skill01, self.mount_skill01),
            appearance::ACTION_SKILL02 => (self.skill02, self.mount_skill02),
            appearance::ACTION_SKILL03 => (self.skill03, self.mount_skill03),
            appearance::ACTION_STRIKE => (self.strike, self.mount_strike),
            action if appearance::is_emote_action(action) => {
                let i = action - appearance::ACTION_SEAT;

                (self.emotes[i], self.mount_emotes[i])
            }
            _ => (self.stand, self.mount_stand),
        }
    }
}

fn body_sub_frame_ms(model: u32, group: usize, action: usize, table_ms: u32) -> u32 {
    if action == appearance::ACTION_LEVELUP && model == 2 && group == 4 {
        (table_ms as f32 * 1.6).round() as u32
    } else {
        table_ms
    }
}

pub fn gait(
    model: u32,
    scale: f32,
    mount_model: Option<u32>,
    mount_scale: f32,
    speed: f32,
) -> usize {
    let scaled_hs01 =
        (model == 0x1F && scale > 0.7) || (mount_model == Some(0x1F) && mount_scale > 0.7);

    let heavy_mount = matches!(mount_model, Some(0x14 | 0x27 | 0x28));

    let threshold = if scaled_hs01 || heavy_mount { 3 } else { 2 };

    if speed > threshold as f32 {
        appearance::ACTION_RUN
    } else {
        appearance::ACTION_WALK
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Held {
    pub hand: appearance::Hand,
    pub bone: i32,
    pub mesh_id: u32,
    pub grip: appearance::HandGrip,
    pub glow: Option<usize>,
    pub ordinary_glow: Option<super::weapon_glow::Ordinary>,
}

impl Held {
    pub fn world(&self, combined: &Matrix4) -> Matrix4 {
        grip_world(&self.grip, combined)
    }
}

pub fn grip_world(grip: &appearance::HandGrip, combined: &Matrix4) -> Matrix4 {
    let m = Matrix4::multiply(
        &Matrix4::translation(grip.offset[0], grip.offset[1], 0.0),
        &Matrix4::rotation_yaw_pitch_roll(grip.yaw, grip.pitch, grip.roll),
    );

    Matrix4::multiply(&m, combined)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActorMeshPlacement {
    Hand,
    Siege { selected: bool },
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeldDraw {
    pub motion_key: render::skin::MotionKey,
    pub mesh_id: u32,
    pub world: Matrix4,
    pub placement: ActorMeshPlacement,
    pub silhouette: bool,
    pub glow: Option<usize>,
    pub ordinary_glow: Option<super::weapon_glow::Ordinary>,
}

pub fn held_silhouette(model: u32, bone: i32, trail: bool) -> bool {
    trail
        && matches!(model, 0 | 1 | 2 | 4)
        && appearance::HAND_BONES
            .get(model as usize)
            .is_some_and(|hands| hands.contains(&bone))
}

#[derive(Debug)]
pub struct Mount {
    pub model: usize,
    pub parts: [PartRef; appearance::MOUNT_PARTS],
    pub hot_effect: bool,
    pub anchor: appearance::AnchorRule,
    pub seat: appearance::RiderSeat,
    pub scale: f32,
    pub tree: FrameTree,
    pub anim: Anim,
    pub glow: [Option<PartGlow>; appearance::PARTS],
}

#[derive(Debug)]
pub struct AirMoveRestore {
    entity: u16,
    action: usize,
    mounted: bool,
    mount_model: Option<u32>,
    mount_scale: f32,
    mount: Option<Mount>,
    cape_seat: Option<appearance::CapeSeat>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct AirMoveDisplay {
    entity: u16,
    ground_y: f32,
    draw_offset: [f32; 2],
    y: f32,
    yaw: f32,
    mount_frame: Option<i32>,
}

#[derive(Debug, Clone, Copy)]
struct AirMoveMountTemplate {
    model: usize,
    clip: Clip,
}

pub const UNBIASED_PITCH_MODELS: std::ops::RangeInclusive<u32> = 0x2D..=0x39;

pub fn skin_world(
    model: u32,
    rotation: [f32; 3],
    scale: f32,
    position: Vec3,
    chirality: Chirality,
) -> Matrix4 {
    let half = camera::PI / 2.0;

    let (yaw, pitch) = match UNBIASED_PITCH_MODELS.contains(&model) {
        true => (rotation[1] + half, rotation[0]),
        false => (rotation[1] - half, rotation[0] - half),
    };

    let r = Matrix4::rotation_yaw_pitch_roll(yaw, pitch, rotation[2]);

    let mut s = Matrix4::scaling(scale, scale, scale);

    if chirality == Chirality::Mirrored {
        s = Matrix4::multiply(&s, &Matrix4::scaling(1.0, 1.0, -1.0));
    }

    let m = Matrix4::multiply(&r, &s);

    Matrix4::multiply(
        &m,
        &Matrix4::translation(position.x, position.y, position.z),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Chirality {
    AsAuthored,
    Mirrored,
}

pub fn skin_yaw_human(yaw: f32) -> f32 {
    -yaw
}

fn initial_yaw(class: i32, yaw: f32) -> f32 {
    if class == 44 {
        std::f32::consts::PI
    } else {
        yaw
    }
}

pub fn facing(yaw: f32) -> f32 {
    camera::PI - yaw
}

pub const ORIGIN_YAW: f32 = -std::f32::consts::FRAC_PI_4;

pub fn attached_world(
    offset: [f32; 2],
    scale: f32,
    yaw: f32,
    pitch: f32,
    anchor: &Matrix4,
) -> Matrix4 {
    let m = Matrix4::multiply(
        &Matrix4::rotation_yaw_pitch_roll(yaw, pitch, 0.0),
        &Matrix4::translation(offset[0], offset[1], 0.0),
    );

    Matrix4::multiply(
        &Matrix4::multiply(&m, &Matrix4::scaling(scale, scale, scale)),
        anchor,
    )
}

impl Mount {
    pub fn rider_world(&self, anchor: &Matrix4) -> Matrix4 {
        let seat = &self.seat;

        attached_world(seat.offset, seat.scale, seat.yaw, seat.pitch, anchor)
    }

    pub fn anchor(&self) -> Option<Matrix4> {
        self.tree
            .last_combined(self.anchor.bones)
            .map(|m| Matrix4(self.anchor.adjust.apply(&m)))
    }
}

#[derive(Debug)]
pub struct Cape {
    pub model: usize,
    pub seat: appearance::CapeSeat,
    pub tree: FrameTree,
    pub anim: Anim,
    pub glow: [Option<PartGlow>; appearance::PARTS],
    pub refinement_glow: [Option<PartGlow>; appearance::PARTS],
}

fn cape_refinement_rank(refine: u8) -> u8 {
    if refine == 0 {
        0
    } else {
        appearance::cape_rank(200 + u16::from(refine.min(15) - 1) * 1000 / 14)
    }
}

impl Cape {
    pub fn world(&self, anchor: &Matrix4) -> Matrix4 {
        let seat = &self.seat;

        attached_world(seat.offset, seat.scale, seat.yaw, seat.pitch, anchor)
    }
}

impl Actor {
    pub fn is_evok(&self) -> bool {
        self.entity >= super::nameplate::PLAYER_ID_LIMIT
            && super::nameplate::tagged_name(&self.name)
    }

    pub fn presentation_scale(&self) -> f32 {
        self.scale * self.evok_factor()
    }

    pub(super) fn evok_factor(&self) -> f32 {
        if self.is_evok() {
            f32::from(self.evok_size.clamp(50, 100)) / 100.0
        } else {
            1.0
        }
    }

    pub fn set_rotation(&mut self, pitch: f32, yaw: f32, roll: f32) {
        self.pitch = pitch;

        self.yaw = yaw;

        self.roll = roll;
    }

    pub fn initialize_rotation(&mut self, pitch: f32, yaw: f32, roll: f32) {
        self.set_rotation(pitch, initial_yaw(self.class, yaw), roll);

        self.mover.yaw = self.yaw;

        self.mover.yaw_start = self.yaw;

        self.mover.yaw_target = self.yaw;
    }

    pub fn recently_hit(&self, now_ms: u32) -> bool {
        self.recent_hit_ms
            .is_some_and(|stamp| !(stamp.wrapping_add(RECENT_HIT_MS) < now_ms))
    }

    fn expire_recent_hit(&mut self, now_ms: u32) {
        if self
            .recent_hit_ms
            .is_some_and(|stamp| stamp.wrapping_add(RECENT_HIT_MS) < now_ms)
        {
            self.recent_hit_ms = None;
        }
    }

    pub fn world(&self) -> Matrix4 {
        let rot = [self.pitch, skin_yaw_human(self.yaw), self.roll];

        skin_world(
            self.model_id,
            rot,
            self.presentation_scale(),
            self.position,
            Chirality::Mirrored,
        )
    }

    pub fn mount_world(&self) -> Matrix4 {
        let Some(mount) = self.mount.as_ref() else {
            return self.world();
        };

        let model = self.mount_model.unwrap_or(self.model_id);

        let rot = [self.pitch, skin_yaw_human(self.yaw), self.roll];

        skin_world(
            model,
            rot,
            mount.scale * self.evok_factor(),
            self.position,
            Chirality::Mirrored,
        )
    }

    pub fn play(&mut self, action: usize, now_ms: u32, models: &[Model]) {
        self.play_mode(action, now_ms, models, false);
    }

    pub(crate) fn play_mode(
        &mut self,
        action: usize,
        now_ms: u32,
        models: &[Model],
        suspended: bool,
    ) {
        if matches!(action, 2 | 3) {
            self.clear_hand_actions();
        }

        if action == appearance::ACTION_STAND01 || action == appearance::ACTION_STAND02 {
            self.mover.server_driven = false;
        }

        let model = &models[self.model];

        let action = appearance::play_action(model.model_id, &self.parts, self.tracked, action);

        let (mut body, mut mount) = self.clips.get(action);

        if model.model_id <= 5
            && (4..=9).contains(&action)
            && let (Some(clip), Some(standing)) = (mount.as_mut(), self.clips.mount_stand)
        {
            clip.slot = standing.slot;

            clip.code = standing.code;
        }

        let previous_clip = (self.anim.slot, self.anim.start_ms);

        if let Some(clip) = body.as_mut() {
            clip.sub_frame_ms =
                body_sub_frame_ms(model.model_id, self.group, action, clip.sub_frame_ms);

            if model.model_id <= 5 {
                let duration = match action {
                    appearance::ACTION_RUN => Some((27.0 - f64::from(self.speed)) as i32 / 2),
                    appearance::ACTION_WALK => Some(
                        ((if model.model_id == 2 { 40.0 } else { 36.0 })
                            - 3.0 * f64::from(self.speed)) as i32
                            / 2,
                    ),
                    _ => None,
                };

                if let Some(duration) = duration {
                    clip.sub_frame_ms = (duration as f32 * self.scale) as u32;
                }
            }
        }

        if let Some(clip) = body
            && self.anim.play(clip, now_ms, model, !self.mounted)
        {
            self.action = action;

            if self.mounted || previous_clip != (self.anim.slot, self.anim.start_ms) {
                self.emote_hold = false;

                self.completion_suspended = suspended;
            }
        }

        if let (Some(seat), Some(mut clip)) = (self.mount.as_mut(), mount) {
            if model.model_id <= 5 {
                clip.sub_frame_ms = match action {
                    appearance::ACTION_RUN => match models[seat.model].model_id {
                        0x14 | 0x27 => 20,
                        0x30 => 3,
                        0x31 | 0x32 | 0x34 => 6,
                        0x3b => 4,
                        0x22 | 0x23 => 8,
                        0x28 => seat.anim.sub_frame_ms,
                        _ => ((27.0 - f64::from(self.speed)) as i32 / 2) as u32,
                    },
                    appearance::ACTION_WALK => {
                        let period = (32.0 - 3.0 * f64::from(self.speed)) as i32 / 2;

                        (period as f32 * self.mount_scale) as u32
                    }
                    _ => clip.sub_frame_ms,
                };
            }

            seat.anim.play(clip, now_ms, &models[seat.model], false);
        }

        if let Some(cape) = self.cape.as_mut()
            && let Some(slot) = appearance::cape_clip(self.mounted, action)
        {
            cape.anim.set_clip(slot, now_ms, &models[cape.model]);
        }

        if self.flags.get(0xB30) == 1
            && (self.mounted || previous_clip != (self.anim.slot, self.anim.start_ms))
        {
            self.skill_afterimage_ms = now_ms;
        }
    }

    pub fn clear_hand_actions(&mut self) {
        for hand in &mut self.swings {
            hand.emitters.skill = false;

            hand.emitters.panel = false;
        }
    }
}

pub const UNPUBLISHED_ANCHOR: Matrix4 = Matrix4([0.0; 16]);

#[derive(Debug, Clone, Copy)]
struct Draw<'a> {
    view: &'a Matrix4,
    eye: Vec3,
    field_scene: bool,
    fog: SkinFog,
    alpha_reference: f32,
    highlight: bool,
    recent_hit: bool,
    invisible: bool,
}

fn pose(tree: &mut FrameTree, model: &Model, now_ms: u32, world: &Matrix4, smoothing: Smoothing) {
    let mut anim = Anim::new(model.slot, model.sub_frame_ms);

    pose_anim(tree, model, &mut anim, now_ms, world, smoothing);
}

pub(crate) fn pose_anim(
    tree: &mut FrameTree,
    model: &Model,
    anim: &mut Anim,
    now_ms: u32,
    world: &Matrix4,
    smoothing: Smoothing,
) {
    sample_anim(tree, model, anim, now_ms, smoothing);

    tree.combine(&world.as_d3dx());
}

fn sample_anim(
    tree: &mut FrameTree,
    model: &Model,
    anim: &mut Anim,
    now_ms: u32,
    smoothing: Smoothing,
) {
    if model.model_id < 100 && anim.blend_from != 0 {
        let bones = model.animation.ani_num_bones as u64;

        let total: u64 = (0..model.animation.clip_count().min(186))
            .map(|slot| model.animation.frames(slot).unwrap_or(0) as u64)
            .sum();

        if total == 0
            || (anim.blend_matrix as u64).saturating_add(bones) > bones.saturating_mul(total)
        {
            anim.blend_from = 0;
        }
    }

    let request = PoseRequest {
        model: model.model_id,
        slot: anim.slot,
        anim_base: model.animation.anim_base(anim.slot).unwrap_or(0),
        frames: model.animation.frames(anim.slot).unwrap_or(0),
        sub_frame_ms: anim.sub_frame_ms,
        elapsed_ms: now_ms.wrapping_sub(anim.start_ms),
        blend_from: anim.blend_from,
        blend_matrix: anim.blend_matrix,
        smoothing,
    };

    let clips = ClipTable::new(model.animation.matrices(), model.animation.ani_num_bones);

    if let Some(result) = tree.sample(&request, &clips) {
        anim.frame = result.frame;

        if result.clears_blend_from {
            anim.blend_from = 0;
        }
    }
}

const NO_GLOW: [Option<PartGlow>; appearance::PARTS] = [None; appearance::PARTS];

#[derive(Debug, Clone, Copy)]
struct RigGlow {
    refine: [u8; appearance::PARTS],
    tier: [u8; appearance::PARTS],
    tint: [u8; appearance::PARTS],
    costume: bool,
    costume_part_one: bool,
    mmulti: bool,
    mount_kind: u8,
}

impl RigGlow {
    fn for_cape(worn: &appearance::Cape, costume_body: i32, rebuilt: bool) -> RigGlow {
        let mut refine = [0; appearance::PARTS];

        refine[0] = worn.rank;

        let mut tier = [0; appearance::PARTS];

        tier[0] = if rebuilt {
            if costume_body == 100 { 1 } else { 12 }
        } else {
            worn.tier
        };

        RigGlow {
            refine,
            tier,
            tint: [0; appearance::PARTS],
            costume: rebuilt && costume_body == 100,
            costume_part_one: false,
            mmulti: false,
            mount_kind: 0,
        }
    }

    fn for_fish(value: u8) -> RigGlow {
        let mut refine = [0; appearance::PARTS];

        refine[0] = value;

        RigGlow {
            refine,
            tier: [0; appearance::PARTS],
            tint: [0; appearance::PARTS],
            costume: false,
            costume_part_one: false,
            mmulti: false,
            mount_kind: 0,
        }
    }

    fn for_body(look: &Look) -> RigGlow {
        let mut tint = look.part_tint;

        if look.substitutes_part_names() {
            tint[..6].fill(0);
        }

        RigGlow {
            refine: look.part_color,
            tier: look.part_tier,
            tint,
            costume: look.costume_body != 0,
            costume_part_one: look.parts[1].mesh == COSTUME_FLAG_MESH,
            mmulti: appearance::MMULTI_CLASSES.contains(&look.class),
            mount_kind: 0,
        }
    }

    fn for_mount(worn: &appearance::Mount) -> RigGlow {
        let mut refine = [0u8; appearance::PARTS];

        refine[..appearance::MOUNT_PARTS].copy_from_slice(&worn.part_color);

        RigGlow {
            refine,
            tier: [0; appearance::PARTS],
            tint: [0; appearance::PARTS],
            costume: false,
            costume_part_one: false,
            mmulti: true,
            mount_kind: worn.visual_kind,
        }
    }
}

fn emit_with_glow(
    model: &Model,
    tree: &FrameTree,
    textures: &[BoundTexture],
    glow: &[Option<PartGlow>; appearance::PARTS],
    draw: Draw<'_>,
    (objects, palettes, draws): (
        &mut Vec<SkinObject>,
        &mut Vec<BonePalette>,
        &mut Vec<SkinDraw>,
    ),
) {
    for part in &model.parts {
        let base = part.texture;

        if base.is_some_and(|index| textures[index].pending) {
            continue;
        }

        let color_key =
            base.is_some_and(|index| textures[index].flag == texture_list::FormatFlag::ColorKey);

        let alpha_reference = if color_key || draw.invisible {
            0.0
        } else {
            draw.alpha_reference
        };

        let index = part.index;

        let lit = glow
            .get(index)
            .copied()
            .flatten()
            .filter(|part| !textures[part.sheet].pending);

        let texture = base.map(|base| match lit {
            Some(part) => textures.len() + part.pair,
            None => base,
        });

        let mut params = SkinParams {
            diffuse: if draw.highlight {
                HIGHLIGHT_DIFFUSE
            } else if draw.recent_hit {
                RECENT_HIT_DIFFUSE
            } else {
                MATERIAL_DIFFUSE
            },
            ambient: MATERIAL_AMBIENT,
            emissive: match draw.highlight {
                true => HIGHLIGHT_EMISSIVE,
                false => MATERIAL_EMISSIVE,
            },
            unlit: model.model_id == 0x3D,
            fog_start: draw.fog.start,
            fog_end: draw.fog.end,
            fog_enabled: draw.fog.enabled,
            force_opaque: base.is_some_and(|index| {
                textures[index].alpha_policy() == render::texture::AlphaPolicy::Opaque
            }),
            modulate_alpha: false,
            glow: lit.map(|part| part.glow),
        };

        if draw.field_scene || base.is_none() {
            params.diffuse[3] = 1.0;

            params.emissive[3] = 1.0;
        }

        let params = match lit.is_some_and(|part| part.glow.emissive) {
            true => render::skin::emissive_material(&params),
            false => params,
        };

        objects.push(SkinObject::new(
            &params,
            part.mesh.header.num_matrices,
            alpha_reference,
        ));

        {
            palettes.push(BonePalette::build(&part.mesh, draw.view, |bone| {
                Matrix4(tree.combined(bone as usize))
            }));
        }

        draws.push(SkinDraw {
            pass: if draw.invisible {
                SkinPass::Invisible
            } else {
                SkinPass::Opaque
            },
            part: part.alloc,
            object: objects.len() as u32 - 1,
            palette: palettes.len() as u32 - 1,
            texture,
        });

        if draw.highlight
            && let Some(base) = base
        {
            let mut outline =
                SkinObject::new(&params, part.mesh.header.num_matrices, alpha_reference);

            outline.material_diffuse = [1.0, 1.0, 1.0, 187.0 / 255.0];

            outline.glow = [0.0; 4];

            objects.push(outline);

            palettes.push(BonePalette::outline(
                &part.mesh,
                draw.view,
                draw.eye,
                |bone| Matrix4(tree.combined(bone as usize)),
            ));

            draws.push(SkinDraw {
                pass: SkinPass::Highlight,
                part: part.alloc,
                object: objects.len() as u32 - 1,
                palette: palettes.len() as u32 - 1,
                texture: Some(base),
            });
        }
    }
}

#[derive(Debug, Clone)]
pub(super) struct LoadedAnimation {
    animation: Arc<ModelAnimation>,
    anim_slots: Arc<registry::AnimSlotTable>,
}

#[derive(Debug, Clone)]
pub struct Doll {
    pub look: Look,
    pub pet_item: u16,
    pub position: Vec3,
    pub yaw: f32,
    pub scale: f32,
    pub hands: (i32, i32),
    pub record: usize,
    pub entity: u16,
    pub name: String,
    pub status: account::Status,
    pub name_level: u8,
    pub level: u16,
    pub guild: u16,
    pub guild_name: String,
    pub face: i32,
    pub affects: [affect::Affect; affect::AFFECTS],
    pub pick_action: Option<usize>,
    pub shop_title: String,
    pub green_name: bool,
}

impl Doll {
    fn body_scale(&self) -> f32 {
        if self.face == 25 && self.look.costume.is_none() {
            titan_scale(self.scale, self.status.mastery[3])
        } else {
            self.scale
        }
    }
}

pub(super) fn titan_scale(base: f32, mastery: u16) -> f32 {
    ((f64::from(mastery) * 0.0006000000000000001 + 1.2) * f64::from(base)) as f32
}

pub const COSTUME_FLAG_MESH: i16 = 0x28;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PartGlow {
    pub glow: glow::Glow,
    pub sheet: usize,
    pub pair: usize,
}

pub fn wearer(look: &Look) -> affect::Wearer {
    affect::Wearer {
        class: look.class,
        part0_mesh: look.parts[0].mesh as u16,
    }
}

fn pulse_item(items: &item_list::ItemList, id: i32) -> (i16, u32) {
    usize::try_from(id)
        .ok()
        .filter(|&id| id != 0)
        .and_then(|id| items.get(id))
        .map_or((0, 0), |entry| (entry.req_level, entry.equip_mask))
}

fn pulse_input(
    actor: &Actor,
    viewer: Option<u16>,
    viewer_position: Option<Vec3>,
    field_scene: bool,
    detail: aura::Detail,
    items: &item_list::ItemList,
    now_ms: u32,
) -> super::skill_anim::PulseInput {
    let (main_item_level, main_effect_mask) = pulse_item(items, actor.main_weapon_item);

    let (off_item_level, off_effect_mask) = pulse_item(items, actor.off_weapon_item);

    let local_distance = viewer_position.map(|position| {
        path::tile_distance(
            (actor.position.x as i32, actor.position.z as i32),
            (position.x as i32, position.z as i32),
        )
    });

    super::skill_anim::PulseInput {
        entity: actor.entity,
        parked: false,
        has_rig: true,
        body_position: actor.position,
        yaw: actor.yaw,
        level: actor.status.level as i16,
        class: actor.class,
        action: actor.action,
        frame: u32::try_from(actor.anim.frame).unwrap_or(u32::MAX),
        main_selector: actor.weapon_class,
        off_selector: actor.off_weapon_class,
        main_item_level,
        off_item_level,
        main_effect_mask,
        off_effect_mask,
        local_distance,
        is_local_player: viewer == Some(actor.entity),
        field_scene,
        colour_kind: actor.primary_tier,
        affect_29: actor.flags.get(affect::Flags::LAST) == 1,
        child_id: actor.skill_child.map_or(0, u32::from),
        effect_mode: i32::from(actor.skill_critical),
        subordinate_suppressed: !detail.effects,
        now_ms,
    }
}

fn apply_pulse_touches(actors: &mut [Actor], plans: &[super::skill_anim::PulsePlan]) {
    let touches: Vec<_> = plans
        .iter()
        .flat_map(|plan| plan.steps.iter())
        .filter_map(|step| match step {
            super::skill_anim::PulseStep::TouchChild { child_id, now_ms } if *child_id != 0 => {
                u16::try_from(*child_id)
                    .ok()
                    .map(|entity| (entity, *now_ms))
            }
            _ => None,
        })
        .collect();

    for (entity, stamp) in touches {
        if let Some(actor) = actors.iter_mut().find(|actor| actor.entity == entity) {
            actor.recent_hit_ms = Some(stamp);
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Unresolved {
    pub record: usize,
    pub face: i32,
    pub reason: String,
}

#[derive(Debug)]
pub struct ActorScene {
    effect_game: Arc<GameDir>,
    pub roster: Roster,
    pub models: Vec<Model>,
    pub actors: Vec<Actor>,
    pub(super) human_visibility_radius: Option<f32>,
    evok_size: u8,
    pub(crate) cape_refinement: bool,
    pub siege_meshes: [bool; 2],
    pub skill_anim: super::skill_anim::State,
    pulse_plans: Vec<super::skill_anim::PulsePlan>,
    viewer_death_landed: bool,
    rotor_refresh: Vec<u16>,
    pub textures: Vec<BoundTexture>,
    texture_loader: Option<texture_loading::Loader>,
    pub texture_updates: Vec<usize>,
    pub glow_pairs: Vec<(usize, usize)>,
    pub arena: SkinArena,
    pub skinfx: super::skinfx::SkinFxScene,
    pub unresolved: Vec<Unresolved>,
    pub viewer: Option<u16>,
    pub hovered: Option<u16>,
    pub(crate) hovered_doll: Option<u64>,
    pub target_affects: [affect::Affect; affect::AFFECTS],
    pub unplaced_capes: Vec<Unresolved>,
    pub unplaced_mounts: Vec<Unresolved>,
    animations: BTreeMap<u32, Option<LoadedAnimation>>,
    pub grass: Grass,
    pub grove: trees::Grove,
    pub ships: Vec<super::ships::Ship>,
    pub tree_kingdom: u8,
    pub tree_shadow_stamps: Vec<Vec3>,
    pub school: fish::School,
    pub flight: butterflies::Flight,
    pub raft: floats::Raft,
    pub light0: [f32; 4],
    pub light1: [f32; 4],
    pub effects_detail: aura::Detail,
    pub effect_options: Option<super::lod::EffectOptions>,
    pub render_lod: super::lod::Inputs,
    pub effect_rng: Lcg,
    pub heights: Vec<i8>,
    pub window: Vec<i8>,
    pub placement_window: Vec<i8>,
    pub relief: Vec<i8>,
    pub zone_heights: Vec<((u8, u8), Vec<i8>)>,
    attributes: Option<attribute_map::AttributeMap>,
    pub zone: (u8, u8),
    pub(crate) footprint_foliage: BTreeMap<(u8, u8), Vec<bool>>,
    pub(crate) death_particles: Vec<(u32, super::burst::Recipe)>,
    scene_id: u32,
    pub(crate) effect_environment: i32,
    pub(crate) body_light: super::body_effects::LightLatch,
    ticked_ms: Option<u32>,
    pub(crate) tick_sounds: Vec<i64>,
    pulsed_ms: Option<u32>,
    grass_tree: Option<FrameTree>,
    tree_scratch: BTreeMap<usize, FrameTree>,
    fish_scratch: BTreeMap<usize, FrameTree>,
    butterfly_scratch: BTreeMap<usize, FrameTree>,
    float_scratch: Option<FrameTree>,
    ship_scratch: BTreeMap<usize, FrameTree>,
    pub registries: Arc<Registries>,
    by_spec: BTreeMap<ModelSpec, Option<usize>>,
    part_cache: resources::PartCache,
    air_move_mount: Option<AirMoveMountTemplate>,
    air_move_display: Option<AirMoveDisplay>,
}

pub(crate) mod resources;
pub(crate) mod texture_loading;

#[cfg(test)]
mod tree_tests;

impl ActorScene {
    pub(crate) fn set_evok_size(&mut self, size: u8) {
        self.evok_size = size.clamp(50, 100);

        for actor in &mut self.actors {
            actor.evok_size = self.evok_size;
        }
    }

    pub fn set_effect_options(&mut self, options: super::lod::EffectOptions) {
        self.effects_detail = options.detail;

        self.effect_options = Some(options);
    }

    pub fn toggle_effects(&mut self, toggle_grass: bool) {
        let mut options = self.effect_options.unwrap_or_default();

        options.detail = self.effects_detail;

        options.toggle(toggle_grass);

        self.set_effect_options(options);
    }

    pub fn secondary_effects_enabled(&self) -> bool {
        self.effect_options.is_none_or(|options| options.secondary)
    }

    pub fn grass_effects_enabled(&self) -> bool {
        self.effect_options.is_none_or(|options| options.grass)
    }

    fn resource_loader(&mut self) -> resources::Loader<'_> {
        resources::Loader {
            models: &mut self.models,
            textures: &mut self.textures,
            arena: &mut self.arena,
            animations: &mut self.animations,
            by_spec: &mut self.by_spec,
            parts: &mut self.part_cache,
            background_textures: self.texture_loader.is_some(),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Grass {
    pub field: grass::Field,
    pub model: Option<usize>,
}

#[allow(clippy::too_many_arguments)]
pub fn build(
    game: &GameDir,
    roster: Option<&[u8]>,
    terrain: &Terrain,
    records: &[ObjRecord],
    light0: [f32; 4],
    light1: [f32; 4],
    grass_enabled: bool,
    scene_id: u32,
    registries: Arc<Registries>,
) -> Result<ActorScene> {
    let reg = Arc::clone(&registries);

    let Registries {
        entries,
        valid,
        sounds,
        textures,
        effect_textures,
        items,
        list: _,
        arena_visual: _,
        resource_names: _,
    } = &*reg;

    let roster = match roster {
        Some(file) => {
            let name = String::from_utf8_lossy(file).into_owned();

            demo::parse(
                &game.read(file).with_context(|| format!("reading {name}"))?,
                demo::Layout::Roster,
            )
            .with_context(|| format!("parsing {name}"))?
        }
        None => demo::Roster {
            actors: Vec::new(),
            layout: demo::Layout::Roster,
        },
    };

    let relief = terrain.build_height_grid();

    let mut heights = relief.clone();

    let masks = object_bin::parse(&game.read(OBJECT_BIN).context("reading object.bin")?)
        .context("parsing object.bin")?;

    collision::stamp_all(&mut heights, &masks, records);

    let attributes = match scene_id == super::world::SCENE_FIELD {
        false => None,
        true => game
            .read(ATTRIBUTE_MAP)
            .ok()
            .map(|bytes| attribute_map::parse(&bytes).context("parsing AttributeMap.dat"))
            .transpose()?,
    };

    let window = collision::window(
        &heights,
        (terrain.zone_x, terrain.zone_y),
        attributes.as_ref(),
    );

    let field = grass::Field::build(records, terrain, &window, light0, grass_enabled);

    let mut scene = ActorScene {
        effect_game: Arc::new(game.clone()),
        roster,
        models: Vec::new(),
        actors: Vec::new(),
        human_visibility_radius: super::human_visibility::load_radius(game, &reg.list),
        evok_size: 100,
        cape_refinement: false,
        siege_meshes: [false; 2],
        skill_anim: super::skill_anim::State::default(),
        pulse_plans: Vec::new(),
        viewer_death_landed: false,
        rotor_refresh: Vec::new(),
        textures: Vec::new(),
        arena: SkinArena::default(),
        skinfx: super::skinfx::SkinFxScene::default(),
        glow_pairs: Vec::new(),
        texture_loader: None,
        texture_updates: Vec::new(),
        unresolved: Vec::new(),
        viewer: None,
        hovered: None,
        hovered_doll: None,
        target_affects: affect::NO_AFFECTS,
        unplaced_capes: Vec::new(),
        unplaced_mounts: Vec::new(),
        animations: BTreeMap::new(),
        grass: Grass { field, model: None },
        grove: trees::Grove::build(records, terrain, &relief, light0, light1),
        ships: super::ships::build(records, (terrain.zone_x, terrain.zone_y)),
        tree_kingdom: 0xFF,
        tree_shadow_stamps: Vec::new(),
        school: fish::School::build(records, (terrain.zone_x, terrain.zone_y), 0),
        flight: butterflies::Flight::build(records, (terrain.zone_x, terrain.zone_y)),
        raft: floats::Raft::build(
            records,
            (terrain.zone_x, terrain.zone_y),
            |_, _| None,
            |x, z| super::lights::sample(&[terrain], x, z),
        ),
        light0,
        light1,
        effects_detail: aura::Detail::default(),
        effect_options: None,
        render_lod: Default::default(),
        effect_rng: Lcg::new(EFFECT_RNG_SEED),
        heights: heights.clone(),
        placement_window: window.clone(),
        window,
        relief,
        zone_heights: vec![((terrain.zone_x, terrain.zone_y), heights.clone())],
        attributes,
        ticked_ms: None,
        tick_sounds: Vec::new(),
        pulsed_ms: None,
        zone: (terrain.zone_x, terrain.zone_y),
        footprint_foliage: BTreeMap::from([(
            (terrain.zone_x, terrain.zone_y),
            super::footprints::foliage(records),
        )]),
        death_particles: Vec::new(),
        scene_id,
        effect_environment: 0,
        body_light: Default::default(),
        grass_tree: None,
        tree_scratch: BTreeMap::new(),
        fish_scratch: BTreeMap::new(),
        butterfly_scratch: BTreeMap::new(),
        float_scratch: None,
        ship_scratch: BTreeMap::new(),
        registries,
        by_spec: BTreeMap::new(),
        part_cache: resources::PartCache::default(),
        air_move_mount: None,
        air_move_display: None,
    };

    let records = scene.roster.actors.clone();

    let mut unresolved: Vec<Unresolved> = Vec::new();

    let mut unplaced_capes: Vec<Unresolved> = Vec::new();

    let mut unplaced_mounts: Vec<Unresolved> = Vec::new();

    for (record, actor) in records.iter().enumerate() {
        macro_rules! decline {
            ($($arg:tt)*) => {
                { unresolved.push(Unresolved { record, face : actor.face, reason :
                format!($($arg)*), }); continue; }
            };
        }

        let Some(look) = Look::resolve(actor, items) else {
            decline!("face {} is outside ItemList", actor.face);
        };

        let Some(model_id) = look.model else {
            decline!("appearance class {} has no model", look.class);
        };

        let spec = ModelSpec {
            model_id,
            clip: match look.is_mounted() {
                true => ClipChoice::Mounted(IDLE_ACTION),
                false => ClipChoice::Action(IDLE_ACTION),
            },
            parts: look.parts,
            variant: look.variant,
            costume_body: 0,
        };

        let resolved = scene
            .load_model(game, entries, valid, sounds, textures, &spec)
            .ok()
            .flatten();

        let Some(model) = resolved else {
            decline!("model {model_id} did not load");
        };

        let tree = FrameTree::build(&scene.models[model].animation.skeleton);

        let mount = look.mount.and_then(|worn| {
            let Some(mount_id) = worn.model else {
                unplaced_mounts.push(Unresolved {
                    record,
                    face: actor.face,
                    reason: format!("mount {} resolves to no model", worn.item),
                });

                return None;
            };

            let (Some(anchor), Some(seat)) =
                (look.rider_anchor(), look.rider_seat(DEMO_BODY_SCALE))
            else {
                unplaced_mounts.push(Unresolved {
                    record,
                    face: actor.face,
                    reason: format!("model {mount_id} publishes no rider anchor"),
                });

                return None;
            };

            let mut parts = [PartRef::EMPTY; appearance::PARTS];

            parts[..appearance::MOUNT_PARTS].copy_from_slice(&worn.parts);

            let spec = ModelSpec {
                model_id: mount_id,
                clip: ClipChoice::Action(IDLE_ACTION),
                parts,
                variant: 0,
                costume_body: 0,
            };

            let loaded = scene
                .load_model(game, entries, valid, sounds, textures, &spec)
                .ok()
                .flatten();

            let Some(model) = loaded else {
                unplaced_mounts.push(Unresolved {
                    record,
                    face: actor.face,
                    reason: format!("mount model {mount_id} did not load"),
                });

                return None;
            };

            let loaded = &scene.models[model];

            Some(Mount {
                model,
                parts: worn.parts,
                hot_effect: false,
                anchor,
                seat,
                scale: appearance::mount_world_scale(look.class, DEMO_BODY_SCALE, worn.scale),
                tree: FrameTree::build(&loaded.animation.skeleton),
                anim: Anim::new(loaded.slot, loaded.sub_frame_ms),
                glow: scene.resolve_glow(game, effect_textures, model, &RigGlow::for_mount(&worn)),
            })
        });

        let cape_anchor = appearance::anchor_rule(model_id, look.parts[0].mesh)
            .map_or(&[] as &[i32], |rule| rule.bones);

        let cape = look.cape.filter(|_| look.wears_cape()).and_then(|worn| {
            let Some(anchor) = look.cape_anchor() else {
                unplaced_capes.push(Unresolved {
                    record,
                    face: actor.face,
                    reason: format!("model {model_id} publishes no cape anchor"),
                });

                return None;
            };

            let (Some(slot), Some(seat)) = (
                look.cape_clip(IDLE_ACTION),
                look.cape_seat(appearance::DEMO_WCLASS),
            ) else {
                unplaced_capes.push(Unresolved {
                    record,
                    face: actor.face,
                    reason: format!("cape {} has no clip or no seat", worn.item),
                });

                return None;
            };

            if !anchor
                .iter()
                .any(|&bone| tree.slot(bone as usize).is_some())
            {
                unplaced_capes.push(Unresolved {
                    record,
                    face: actor.face,
                    reason: format!("model {model_id} has none of the bones {anchor:?}"),
                });

                return None;
            }

            let mut parts = [PartRef::EMPTY; appearance::PARTS];

            parts[0] = worn.part_for_body(look.costume_body);

            let spec = ModelSpec {
                model_id: appearance::CAPE_MODEL,
                clip: ClipChoice::Fixed {
                    slot,
                    sub_frame_ms: appearance::CAPE_SUB_FRAME_MS,
                },
                parts,
                variant: 0,
                costume_body: look.costume_body,
            };

            let loaded = scene
                .load_model(game, entries, valid, sounds, textures, &spec)
                .ok()
                .flatten();

            let Some(model) = loaded else {
                unplaced_capes.push(Unresolved {
                    record,
                    face: actor.face,
                    reason: format!("cape {} did not load", worn.item),
                });

                return None;
            };

            let glow = scene.resolve_glow(
                game,
                effect_textures,
                model,
                &RigGlow::for_cape(&worn, look.costume_body, false),
            );

            let loaded = &scene.models[model];

            Some(Cape {
                model,
                seat,
                tree: FrameTree::build(&loaded.animation.skeleton),
                anim: Anim::new(loaded.slot, loaded.sub_frame_ms),
                glow,
                refinement_glow: glow,
            })
        });

        let held: Vec<Held> = appearance::Hand::BOTH
            .into_iter()
            .filter_map(|hand| {
                let (bone, mesh_id, grip) =
                    look.hand(hand, actor.right_hand, actor.left_hand, items)?;

                tree.slot(bone as usize)?;

                Some(Held {
                    hand,
                    bone,
                    mesh_id,
                    grip,
                    glow: super::weapon_glow::special(
                        model_id,
                        mesh_id,
                        look.part_color[hand.part()],
                        glow::effect_class(
                            look.part_tier[hand.part()].min(15),
                            look.part_tint[hand.part()],
                        ),
                    ),
                    ordinary_glow: super::weapon_glow::Ordinary::for_hand(&look, hand, mesh_id),
                })
            })
            .collect();

        let parts = entries
            .iter()
            .find(|e| e.model_id == model_id)
            .map_or(0, |e| e.num_parts as usize);

        let swings = Swing::for_actor(record, actor, &look, parts, items, |bone| {
            tree.slot(bone as usize).is_some()
        });

        let mounted = look.is_mounted();

        let mount_model = look.mount.and_then(|m| m.model);

        let group = anim_group(&look, actor.right_hand, actor.left_hand, items);

        let weapon_class = appearance::weapon_class(actor.right_hand, items);

        let off_weapon_class = appearance::weapon_class(actor.left_hand, items);

        let body_slots = scene.anim_slots(model_id);

        let body_clip = |action: usize| {
            let slots = body_slots.as_ref()?;

            Model::clip_for(sounds, slots, model_id, group, action, mounted)
        };

        let mount_clip = |action: usize| Model::direct_clip_for(sounds, mount_model?, action);

        let clips = ActionClips {
            stand: body_clip(appearance::ACTION_STAND02),
            walk: body_clip(appearance::ACTION_WALK),
            run: body_clip(appearance::ACTION_RUN),
            die: body_clip(appearance::ACTION_DIE),
            dead: body_clip(appearance::ACTION_DEAD),
            attack1: body_clip(appearance::ACTION_ATTACK1),
            attack2: body_clip(appearance::ACTION_ATTACK2),
            attack3: body_clip(appearance::ACTION_ATTACK3),
            mount_stand: mount_clip(appearance::ACTION_STAND02),
            mount_walk: mount_clip(appearance::ACTION_WALK),
            mount_run: mount_clip(appearance::ACTION_RUN),
            mount_die: mount_clip(appearance::ACTION_DIE),
            mount_dead: mount_clip(appearance::ACTION_DEAD),
            mount_attack1: mount_clip(appearance::ACTION_ATTACK1),
            mount_attack2: mount_clip(appearance::ACTION_ATTACK2),
            mount_attack3: mount_clip(appearance::ACTION_ATTACK3),
            skill01: body_clip(appearance::ACTION_SKILL01),
            skill02: body_clip(appearance::ACTION_SKILL02),
            skill03: body_clip(appearance::ACTION_SKILL03),
            mount_skill01: mount_clip(appearance::ACTION_SKILL01),
            mount_skill02: mount_clip(appearance::ACTION_SKILL02),
            mount_skill03: mount_clip(appearance::ACTION_SKILL03),
            strike: body_clip(appearance::ACTION_STRIKE),
            mount_strike: mount_clip(appearance::ACTION_STRIKE),
            emotes: std::array::from_fn(|i| body_clip(appearance::ACTION_SEAT + i)),
            mount_emotes: std::array::from_fn(|i| mount_clip(appearance::ACTION_SEAT + i)),
            extra: None,
        };

        let (x, z) = actor.from;

        let loaded = &scene.models[model];

        let speed = actor.tail.first().copied().unwrap_or(0) as f32;

        let cape_flag = cape.as_ref().and(look.cape);

        scene.actors.push(Actor {
            incarnation: super::effect_context::next_incarnation(),
            skill_anchor: None,
            camera_visible: false,
            model,
            model_id,
            record,
            entity: 0,
            name: String::new(),
            status: account::Status::default(),
            alternate_hp: None,
            mount_hp_bonus: 0,
            name_level: 0,
            level: 0,
            guild: 0,
            guild_rank: 0,
            charidx: None,
            guild_name: String::new(),
            body: record as i32,
            helmet_item: look.helmet_item,
            kingdom: look.kingdom,
            costume_item: 0,
            costume_effect_item: look.costume.unwrap_or(0),
            body_item_tier: 0,
            pet_item: 0,
            position: Vec3::new(x, ground_height(&heights, terrain, x, z), z),
            yaw: initial_yaw(look.class, (actor.yaw_deg as f32).to_radians()),
            pitch: 0.0,
            roll: 0.0,
            scale: 1.0,
            evok_size: 100,
            anim: idle_anim(&clips, loaded),
            action: IDLE_ACTION,
            emote_latch: None,
            emote_hold: false,
            spin: None,
            completion_suspended: false,
            group,
            weapon_class,
            off_weapon_class,
            main_weapon_item: actor.right_hand,
            active_hands: super::swing::hand_activity(
                &look,
                (actor.left_hand, actor.right_hand),
                items,
            ),
            off_weapon_item: actor.left_hand,
            primary_tier: look.part_tier[7],
            skill_critical: false,
            skill_child: None,
            recent_hit_ms: None,
            pending_hit: None,
            haste_trail_ms: 0,
            motion_effects: Default::default(),
            impact_grid_ms: 0,
            affect_pair_ms: 0,
            body_cloud_ms: 0,
            body_shadow: Default::default(),
            body_sprites: Default::default(),
            footprint_kind: 1,
            footprint_ms: 0,
            movement_dust: Default::default(),
            body_glow: Default::default(),
            body_points: Default::default(),
            body_butterflies: butterflies::owned(look.class, look.parts[0], &mut scene.effect_rng),
            affect_emit_ms: [0; 4],
            afterimage_ms: 0,
            appearance_epoch: 0,
            skill_afterimage_window: None,
            skill_afterimage_ms: 0,
            transformation: Default::default(),
            projectile_muzzle: Vec3::ZERO,
            clips,
            speed,
            is_dead: false,
            tracked: true,
            party_tint: None,
            green_name: false,
            plate_outline: false,
            plate_backdrop_color: 0x55AA_0000,
            plate_name_color: 0xFFFF_FFAA,
            corpse_ms: None,
            removal_mode: super::corpse::MODE_NONE,
            highlight: false,
            target_hud: TargetHud::default(),
            shop_title: String::new(),
            balloon: None,
            destination: (actor.to.0 as i32, actor.to.1 as i32),
            mover: walk::Mover::standing(
                [x, z],
                initial_yaw(look.class, (actor.yaw_deg as f32).to_radians()),
            ),
            mount_model,
            mount_scale: look.mount.map_or(1.0, |m| m.scale),
            mount_kind: look.mount.map_or(0, |m| m.kind),
            mounted,
            parts: look.parts,
            glow: NO_GLOW,
            class: look.class,
            forced_weather_ms: 0,
            affects: affect::NO_AFFECTS,
            details: affect::NO_DETAILS,
            flags: affect::Flags::build(&affect::NO_AFFECTS, &affect::NO_DETAILS, &wearer(&look)),
            tree,
            cape_anchor,
            cape_flag,
            cape,
            mount,
            held,
            swings,
            pending: None,
            volley: super::skill_fx::Volley::default(),
        });
    }

    for actor in &mut scene.actors {
        super::footprints::refresh_type(actor);
    }

    scene.unresolved = unresolved;

    scene.unplaced_capes = unplaced_capes;

    scene.unplaced_mounts = unplaced_mounts;

    scene.load_grass(game, entries, valid, sounds, textures)?;

    let mut by_tree: BTreeMap<(u32, PartRef), Option<usize>> = BTreeMap::new();

    for spec in scene.grove.specs() {
        let mut parts = [PartRef::EMPTY; appearance::PARTS];

        parts[0] = spec.1;

        let model = ModelSpec {
            model_id: spec.0,
            clip: ClipChoice::Fixed {
                slot: trees::TREE_SLOT,
                sub_frame_ms: trees::TREE_SUB_FRAME_MS,
            },
            parts,
            variant: 0,
            costume_body: 0,
        };

        let loaded = scene.load_model(game, entries, valid, sounds, textures, &model)?;

        by_tree.insert(spec, loaded);
    }

    let mut grove = std::mem::take(&mut scene.grove);

    for tree in &mut grove.trees {
        tree.model = by_tree
            .get(&(tree.model_id, trees::part_zero(tree.kind)))
            .copied()
            .flatten();
    }

    for tree in &grove.trees {
        let Some(model) = tree.model else {
            continue;
        };

        scene
            .tree_scratch
            .entry(model)
            .or_insert_with(|| FrameTree::build(&scene.models[model].animation.skeleton));
    }

    scene.grove = grove;

    scene.load_school(game, entries, valid, sounds, textures)?;

    scene.load_flight(game, entries, valid, sounds, textures)?;

    scene.load_raft(game, entries, valid, sounds, textures)?;

    scene.load_ships(game, entries, valid, sounds, textures)?;

    if scene_id == super::world::SCENE_FIELD {
        scene.load_air_move_mount(game, entries, valid, sounds, textures)?;
    }

    Ok(scene)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ClipChoice {
    Action(usize),
    Mounted(usize),
    Fixed { slot: usize, sub_frame_ms: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ModelSpec {
    pub model_id: u32,
    pub clip: ClipChoice,
    pub parts: [PartRef; appearance::PARTS],
    pub variant: u32,
    pub costume_body: i32,
}

impl ModelSpec {
    pub fn bare(model_id: u32, clip: ClipChoice) -> ModelSpec {
        ModelSpec {
            model_id,
            clip,
            parts: [PartRef::EMPTY; appearance::PARTS],
            variant: 0,
            costume_body: 0,
        }
    }
}

fn skin_part_admitted(face_mesh: i16, part: u32, mesh: i16) -> bool {
    (face_mesh as u16) < 90 || part == 0 || mesh != 0
}

pub fn ground_height(heights: &[i8], terrain: &Terrain, x: f32, z: f32) -> f32 {
    let (ox, oz) = (
        terrain.zone_x as f32 * ZONE_CELLS as f32,
        terrain.zone_y as f32 * ZONE_CELLS as f32,
    );

    let (cx, cz) = ((x - ox).floor(), (z - oz).floor());

    if cx < 0.0 || cz < 0.0 || cx >= HEIGHT_GRID_DIM as f32 || cz >= HEIGHT_GRID_DIM as f32 {
        return 0.0;
    }

    let index = cz as usize * HEIGHT_GRID_DIM + cx as usize;

    f32::from(heights.get(index).copied().unwrap_or(0)) * HEIGHT_SCALE
}

pub fn scene_ground_height(scene: u32, z: f32, terrain: f32) -> f32 {
    if scene == super::world::SCENE_SELECT_CHAR {
        if z < 2060.0 { 0.1 } else { 4.1 }
    } else {
        terrain
    }
}

pub fn snap_height(window: &[i8], zone: (u8, u8), x: f32, z: f32, own: f32) -> f32 {
    let field = HeightField::window(window, zone);

    let byte = field.byte(x as i32, z as i32);

    match byte < i32::from(IMPASSABLE) {
        true => byte as f32 * HEIGHT_SCALE,
        false => own,
    }
}

pub fn special_height(
    class: i32,
    main_mesh: i16,
    at: Vec3,
    zone: (u8, u8),
    saved: &[i8],
    ground: f32,
) -> f32 {
    if class == 0x2D && main_mesh == 0 {
        return 0.0;
    }

    if class != 0x32 || !at.x.is_finite() || !at.z.is_finite() {
        return ground;
    }

    let x = (at.x - f32::from(zone.0) * 128.0) as i32;

    let z = (at.z - f32::from(zone.1) * 128.0) as i32;

    if !(0..256).contains(&x) || !(0..256).contains(&z) {
        return ground;
    }

    saved
        .get(z as usize * 256 + x as usize)
        .map_or(ground, |&height| f32::from(height) * 0.1)
}

fn apply_air_move_step(actor: &mut Actor, step: &walk::Step, display: &mut AirMoveDisplay) {
    display.ground_y = step.ground;

    actor.position = Vec3::new(
        step.position[0] + display.draw_offset[0],
        display.y,
        step.position[1] + display.draw_offset[1],
    );

    actor.yaw = display.yaw;

    if let (Some(mount), Some(frame)) = (actor.mount.as_mut(), display.mount_frame)
        && let Ok(frame) = u32::try_from(frame)
    {
        mount.anim.sub_frame_ms = frame;
    }
}

impl ActorScene {
    pub fn teleport_afterimage(
        &self,
        index: usize,
        now_ms: u32,
    ) -> Option<super::skinfx::SkinMeshEffect> {
        let actor = self.actors.get(index)?;

        let mut effect = super::skinfx::afterimage(super::skinfx::BodySnapshot::of(actor));

        let slots = self.anim_slots(actor.model_id);

        let primary_model = effect.mesh_id as usize;

        let clip = |model, action| {
            self.registries
                .sounds
                .class(model)
                .and_then(|table| table.frame.get(action))
                .and_then(|code| usize::try_from(*code).ok())
                .and_then(|code| slots.as_ref()?.get(actor.group, code))
                .map(usize::from)
        };

        let primary_clip = clip(primary_model, actor.action);

        let secondary_clip = actor
            .action
            .checked_add(28)
            .and_then(|action| clip(actor.model_id as usize, action));

        for (rig, slot) in [
            (effect.rig.as_mut(), primary_clip),
            (effect.rig2.as_mut(), secondary_clip),
        ] {
            if let Some(rig) = rig {
                rig.animation.start_ms = now_ms;

                rig.clock_started = true;

                if let Some(slot) = slot
                    && let Some(model) = self.models.get(rig.model)
                {
                    rig.animation.set_clip(slot, now_ms, model);
                }
            }
        }

        if let Some(rider) = effect.rig2.as_mut()
            && let Some(mount) = actor.mount.as_ref()
        {
            let model = effect.mesh_id as u32;

            if let Some(rule) = appearance::anchor_rule(model, mount.parts[0].mesh) {
                let variant = if model == 20 {
                    if matches!(actor.parts[0].mesh, 7 | 47 | 48) {
                        4
                    } else {
                        3
                    }
                } else {
                    2
                };

                if let Some(rotation) = appearance::seat_rotation(variant, model) {
                    rider.parent_anchor = Some((rule, rotation));
                }
            }
        }

        Some(effect)
    }

    pub fn skill_afterimage(
        &self,
        index: usize,
        now: u32,
    ) -> Option<super::skinfx::SkinMeshEffect> {
        let actor = self.actors.get(index)?;

        let mut effect = self.teleport_afterimage(index, now)?;

        effect.scale = 1.0;

        effect.start_ms = Some(now);

        effect.lifetime_ms = 300;

        effect.colour_a = [0.5; 3];

        effect.spin = actor.yaw;

        effect.yaw = actor.yaw;

        if let Some(primary) = effect.rig.as_mut() {
            primary.anim_speed_ms = actor.anim.sub_frame_ms;
        }

        if let Some(rider) = effect.rig2.as_mut() {
            let slot = actor
                .action
                .checked_add(24)
                .and_then(|action| {
                    self.registries
                        .sounds
                        .class(actor.model_id as usize)?
                        .frame
                        .get(action)
                })
                .and_then(|code| usize::try_from(*code).ok())
                .and_then(|code| self.anim_slots(actor.model_id)?.get(actor.group, code))
                .map(usize::from);

            if let Some(slot) = slot {
                rider
                    .animation
                    .set_clip(slot, now, &self.models[rider.model]);
            }
        }

        Some(effect)
    }

    pub fn hydrate_skin(
        &mut self,
        effect: &mut super::skinfx::SkinMeshEffect,
        now_ms: u32,
    ) -> Result<()> {
        let request = effect
            .construction
            .as_ref()
            .context("skin effect has no construction request")?;

        let dual = request.secondary.is_some();

        anyhow::ensure!(
            !dual || (effect.scale2.is_finite() && effect.scale2 != 0.0),
            "effect rider scale is invalid"
        );

        let mut primary = self.load_effect_rig(
            &request.primary,
            request.cape && !dual,
            effect.scale,
            now_ms,
        )?;

        if effect.motion == super::skinfx::Motion::WeaponTrack {
            primary.animation.start_ms = effect.start_ms.unwrap_or(now_ms);
        }

        let mut secondary = request
            .secondary
            .as_ref()
            .map(|input| self.load_effect_rig(input, request.cape, effect.scale, now_ms))
            .transpose()?;

        if let Some(rider) = secondary.as_mut() {
            let model = u32::try_from(request.primary.model_id)?;

            let rule = appearance::anchor_rule(model, request.primary.parts[0].mesh)
                .context("effect mount has no rider anchor")?;

            anyhow::ensure!(
                primary.tree.last_combined(rule.bones).is_some(),
                "effect mount anchor bone absent"
            );

            let variant = if model == 20 {
                if matches!(
                    request.secondary.as_ref().unwrap().parts[0].mesh,
                    7 | 47 | 48
                ) {
                    4
                } else {
                    3
                }
            } else {
                2
            };

            let rotation = appearance::seat_rotation(variant, model)
                .context("effect rider rotation absent")?;

            rider.parent_anchor = Some((rule, rotation));
        }

        effect.build_skins(primary, secondary);

        Ok(())
    }

    fn load_effect_rig(
        &mut self,
        input: &super::skinfx::SkinInput,
        cape: bool,
        outer_scale: f32,
        now_ms: u32,
    ) -> Result<super::skinfx::Rig> {
        let model_id = u32::try_from(input.model_id).context("negative effect model")?;

        anyhow::ensure!(
            input.scale.is_finite() && outer_scale.is_finite(),
            "nonfinite effect scale"
        );

        let mut animation = input
            .animation
            .unwrap_or(Anim::new(0, super::skinfx::ANIM_SPEED_MS));

        if let Some(action) = input.clip_action {
            animation.slot = self
                .registries
                .sounds
                .class(model_id as usize)
                .and_then(|table| table.frame.get(action))
                .and_then(|slot| usize::try_from(*slot).ok())
                .context("effect animation table slot absent")?;
        }

        animation.start_ms = now_ms;

        let spec = ModelSpec {
            model_id,
            parts: input.parts,
            variant: u32::from(cape),
            costume_body: 0,
            clip: ClipChoice::Fixed {
                slot: animation.slot,
                sub_frame_ms: animation.sub_frame_ms,
            },
        };

        let game = Arc::clone(&self.effect_game);

        let tables = Arc::clone(&self.registries);

        let index = self
            .load_model(
                &game,
                &tables.entries,
                &tables.valid,
                &tables.sounds,
                &tables.textures,
                &spec,
            )?
            .with_context(|| {
                format!(
                    "effect model {model_id} / clip {} unavailable",
                    animation.slot
                )
            })?;

        let model = &self.models[index];

        anyhow::ensure!(
            !model.parts.is_empty(),
            "effect model {model_id} has no drawable parts"
        );

        let mut tree = match input.pose.as_ref() {
            Some(pose) => {
                anyhow::ensure!(pose.mesh_id == input.model_id, "effect pose/model mismatch");

                pose.tree.clone()
            }
            None => {
                let mut tree = FrameTree::build(&model.animation.skeleton);

                pose_anim(
                    &mut tree,
                    model,
                    &mut animation,
                    now_ms,
                    &Matrix4::IDENTITY,
                    self.render_lod.smoothing,
                );

                tree
            }
        };

        tree.combine(&Matrix4::IDENTITY.as_d3dx());

        let mut rig = super::skinfx::Rig::new(index, input.model_id, tree);

        rig.animation = animation;

        rig.clock_started = true;

        rig.anim_speed_ms = animation.sub_frame_ms;

        rig.scale_factor = if outer_scale == 0.0 {
            1.0
        } else {
            input.scale / outer_scale
        };

        anyhow::ensure!(
            rig.scale_factor.is_finite(),
            "effect rig scale ratio is nonfinite"
        );

        Ok(rig)
    }

    pub fn take_viewer_death_landed(&mut self) -> bool {
        std::mem::take(&mut self.viewer_death_landed)
    }

    fn load_model(
        &mut self,
        game: &GameDir,
        entries: &[ModelEntry],
        valid: &ValidIndex,
        sounds: &AniSound,
        textures: &TextureList,
        spec: &ModelSpec,
    ) -> Result<Option<usize>> {
        self.resource_loader()
            .load_model(game, entries, valid, sounds, textures, spec)
    }

    pub fn order(&mut self, record: usize, now_ms: u32) -> bool {
        self.plan(record, now_ms, false)
    }

    pub fn redirect(&mut self, record: usize, now_ms: u32) -> bool {
        self.plan(record, now_ms, true)
    }

    fn plan(&mut self, record: usize, now_ms: u32, mid_step: bool) -> bool {
        let Some(index) = self.actors.iter().position(|a| a.record == record) else {
            return false;
        };

        let occupied: Vec<(i32, i32)> = self
            .actors
            .iter()
            .map(|a| (a.position.x as i32, a.position.z as i32))
            .collect();

        let field = HeightField::window(&self.window, self.zone);

        let actor = &mut self.actors[index];

        let (to, budget) = (actor.destination, path::STEP_BUDGET);

        match mid_step {
            false => actor.mover.order(&field, to, budget, now_ms, &occupied),
            true => actor
                .mover
                .redirect(&field, to, budget, now_ms, actor.speed, &occupied),
        }
    }

    pub fn destroy(&mut self, records: &[std::ops::Range<usize>]) {
        if self.actors.iter().any(|a| {
            Some(a.incarnation) == self.hovered_doll
                && records.iter().any(|r| r.contains(&a.record))
        }) {
            self.hovered_doll = None;
        }

        self.actors
            .retain(|a| !records.iter().any(|r| r.contains(&a.record)));
    }

    pub fn play_at(&mut self, index: usize, action: usize, now_ms: u32) {
        if index < self.actors.len() {
            self.actors[index].play(action, now_ms, &self.models);
        }
    }

    pub fn play(&mut self, record: usize, action: usize, now_ms: u32) {
        let Some(index) = self.actors.iter().position(|a| a.record == record) else {
            return;
        };

        self.actors[index].play(action, now_ms, &self.models);
    }

    pub fn anim_slots(&self, model_id: u32) -> Option<Arc<registry::AnimSlotTable>> {
        self.animations
            .get(&model_id)?
            .as_ref()
            .map(|l| l.anim_slots.clone())
    }

    fn load_animation(
        &mut self,
        game: &GameDir,
        entry: &ModelEntry,
        valid: &ValidIndex,
    ) -> Result<Option<LoadedAnimation>> {
        self.resource_loader().load_animation(game, entry, valid)
    }

    pub fn answer_animation_probe(
        &mut self,
        game: &GameDir,
        request: animation_probe::Request,
    ) -> Option<animation_probe::Response> {
        let client_id = self.viewer?;

        let model_id = request.effective_model() as u32;

        let registries = Arc::clone(&self.registries);

        let Some(entry) = registries
            .entries
            .iter()
            .find(|entry| entry.model_id == model_id)
        else {
            return None;
        };

        let loaded = match self.load_animation(game, entry, &registries.valid) {
            Ok(Some(loaded)) => loaded,
            Ok(None) => return None,
            Err(_) => {
                self.animations.insert(model_id, None);

                return None;
            }
        };

        Some(request.answer(&loaded.animation, client_id))
    }

    fn bind_effect_slot(
        &mut self,
        game: &GameDir,
        textures: &TextureList,
        slot: usize,
    ) -> Option<usize> {
        self.bind_table_slot(game, textures, slot, true)
    }

    fn bind_table_slot(
        &mut self,
        game: &GameDir,
        textures: &TextureList,
        slot: usize,
        effect_table: bool,
    ) -> Option<usize> {
        self.resource_loader()
            .bind_table_slot(game, textures, slot, effect_table)
    }

    fn resolve_glow(
        &mut self,
        game: &GameDir,
        effects: &TextureList,
        model: usize,
        rig: &RigGlow,
    ) -> [Option<PartGlow>; appearance::PARTS] {
        let mut out = [None; appearance::PARTS];

        for loaded in 0..self.models[model].parts.len() {
            let part = self.models[model].parts[loaded].index;

            let Some(base) = self.models[model].parts[loaded].texture else {
                continue;
            };

            if part >= appearance::PARTS {
                continue;
            }

            let class = glow::effect_class(rig.tier[part], rig.tint[part]);

            let context = glow::Part {
                texture: self.textures[base].slot as i32,
                color_key: self.textures[base].flag == texture_list::FormatFlag::ColorKey,
                costume: rig.costume || (part == 1 && rig.costume_part_one),
                mmulti: rig.mmulti,
                mount_kind: match part < 2 {
                    true => rig.mount_kind,
                    false => 0,
                },
            };

            let Some(glow) = glow::for_part(rig.refine[part].min(glow::PART_CLAMP), class, context)
            else {
                continue;
            };

            let Some(sheet) = self.bind_effect_slot(game, effects, glow.texture as usize) else {
                continue;
            };

            out[part] = Some(PartGlow {
                glow,
                sheet,
                pair: self.glow_pair(base, sheet),
            });
        }

        out
    }

    fn glow_pair(&mut self, base: usize, sheet: usize) -> usize {
        match self.glow_pairs.iter().position(|&p| p == (base, sheet)) {
            Some(index) => index,
            None => {
                self.glow_pairs.push((base, sheet));

                self.glow_pairs.len() - 1
            }
        }
    }

    fn load_grass(
        &mut self,
        game: &GameDir,
        entries: &[ModelEntry],
        valid: &ValidIndex,
        sounds: &AniSound,
        textures: &TextureList,
    ) -> Result<()> {
        let specs = self
            .grass
            .field
            .blades
            .iter()
            .map(|b| (grass::Field::mesh_index(b.kind) as i16, b.colour as i16))
            .collect::<std::collections::BTreeSet<_>>();

        for (mesh, tex) in specs {
            let mut parts = [PartRef::EMPTY; appearance::PARTS];

            parts[0] = PartRef { mesh, tex };

            let spec = ModelSpec {
                model_id: grass::LEAF_MODEL,
                clip: ClipChoice::Fixed {
                    slot: grass::LEAF_SLOT,
                    sub_frame_ms: grass::LEAF_SUB_FRAME_MS,
                },
                parts,
                variant: 0,
                costume_body: 0,
            };

            let loaded = self.load_model(game, entries, valid, sounds, textures, &spec)?;

            for blade in self.grass.field.blades.iter_mut().filter(|b| {
                (grass::Field::mesh_index(b.kind) as i16, b.colour as i16) == (mesh, tex)
            }) {
                blade.model = loaded;
            }

            if self.grass_tree.is_none() {
                if let Some(model) = loaded {
                    self.grass_tree =
                        Some(FrameTree::build(&self.models[model].animation.skeleton));
                }
            }
        }

        self.grass.model = self.grass.field.blades.first().and_then(|b| b.model);

        Ok(())
    }

    fn load_school(
        &mut self,
        game: &GameDir,
        entries: &[ModelEntry],
        valid: &ValidIndex,
        sounds: &AniSound,
        textures: &TextureList,
    ) -> Result<()> {
        let mut school = std::mem::take(&mut self.school);

        let registries = Arc::clone(&self.registries);

        for mesh in school.meshes() {
            let mut parts = [PartRef::EMPTY; appearance::PARTS];

            parts[0] = PartRef { mesh, tex: 0 };

            let spec = ModelSpec {
                model_id: fish::FISH_MODEL,
                clip: ClipChoice::Fixed {
                    slot: fish::FISH_SLOT,
                    sub_frame_ms: fish::FISH_SUB_FRAME_MS,
                },
                parts,
                variant: 0,
                costume_body: 0,
            };

            let loaded = self.load_model(game, entries, valid, sounds, textures, &spec)?;

            for fish in school.fish.iter_mut().filter(|f| f.mesh == mesh) {
                fish.model = loaded;

                fish.glow = loaded.and_then(|model| {
                    self.resolve_glow(
                        game,
                        &registries.effect_textures,
                        model,
                        &RigGlow::for_fish(fish.refine),
                    )[0]
                });
            }

            if let Some(model) = loaded {
                self.fish_scratch
                    .entry(model)
                    .or_insert_with(|| FrameTree::build(&self.models[model].animation.skeleton));
            }
        }

        self.school = school;

        Ok(())
    }

    fn load_flight(
        &mut self,
        game: &GameDir,
        entries: &[ModelEntry],
        valid: &ValidIndex,
        sounds: &AniSound,
        textures: &TextureList,
    ) -> Result<()> {
        let mut flight = std::mem::take(&mut self.flight);

        let mut specs = flight.specs();

        specs.extend(
            self.actors
                .iter()
                .filter_map(|a| a.body_butterflies.as_ref())
                .flat_map(|f| {
                    f.butterflies
                        .iter()
                        .map(|b| (b.model_id, b.texture, b.sub_frame_ms))
                }),
        );

        specs.sort_unstable();

        specs.dedup();

        for spec in specs {
            let (model_id, texture, sub_frame_ms) = spec;

            let mut parts = [PartRef::EMPTY; appearance::PARTS];

            parts[0] = PartRef {
                mesh: 0,
                tex: texture,
            };

            let loaded = self.load_model(
                game,
                entries,
                valid,
                sounds,
                textures,
                &ModelSpec {
                    model_id,
                    clip: ClipChoice::Fixed {
                        slot: butterflies::BUTTERFLY_SLOT,
                        sub_frame_ms,
                    },
                    parts,
                    variant: 0,
                    costume_body: 0,
                },
            )?;

            for butterfly in flight
                .butterflies
                .iter_mut()
                .filter(|b| (b.model_id, b.texture, b.sub_frame_ms) == spec)
            {
                butterfly.model = loaded;
            }

            for butterfly in self
                .actors
                .iter_mut()
                .filter_map(|a| a.body_butterflies.as_mut())
                .flat_map(|f| &mut f.butterflies)
                .filter(|b| (b.model_id, b.texture, b.sub_frame_ms) == spec)
            {
                butterfly.model = loaded;
            }

            if let Some(model) = loaded {
                self.butterfly_scratch
                    .entry(model)
                    .or_insert_with(|| FrameTree::build(&self.models[model].animation.skeleton));
            }
        }

        self.flight = flight;

        Ok(())
    }

    fn load_raft(
        &mut self,
        game: &GameDir,
        entries: &[ModelEntry],
        valid: &ValidIndex,
        sounds: &AniSound,
        textures: &TextureList,
    ) -> Result<()> {
        if self.raft.meshed().next().is_none() {
            return Ok(());
        }

        let loaded = self.load_model(
            game,
            entries,
            valid,
            sounds,
            textures,
            &ModelSpec {
                model_id: floats::FLOAT_MODEL,
                clip: ClipChoice::Fixed {
                    slot: floats::FLOAT_SLOT,
                    sub_frame_ms: floats::FLOAT_SUB_FRAME_MS,
                },
                parts: [PartRef::EMPTY; appearance::PARTS],
                variant: 0,
                costume_body: 0,
            },
        )?;

        for float in self.raft.floats.iter_mut().filter(|f| f.has_mesh()) {
            float.model = loaded;
        }

        if let Some(model) = loaded {
            self.float_scratch
                .get_or_insert_with(|| FrameTree::build(&self.models[model].animation.skeleton));
        }

        Ok(())
    }

    fn load_ships(
        &mut self,
        game: &GameDir,
        entries: &[ModelEntry],
        valid: &ValidIndex,
        sounds: &AniSound,
        textures: &TextureList,
    ) -> Result<()> {
        let mut ships = std::mem::take(&mut self.ships);

        let result = (|| {
            for ship in &mut ships {
                if ship.model.is_some() {
                    continue;
                }

                let mut parts = [PartRef::EMPTY; appearance::PARTS];

                parts[0] = ship.part_zero();

                ship.model = self.load_model(
                    game,
                    entries,
                    valid,
                    sounds,
                    textures,
                    &ModelSpec {
                        model_id: 60,
                        clip: ClipChoice::Fixed {
                            slot: 0,
                            sub_frame_ms: 30,
                        },
                        parts,
                        variant: 0,
                        costume_body: 0,
                    },
                )?;

                if let Some(model) = ship.model {
                    self.ship_scratch.entry(model).or_insert_with(|| {
                        FrameTree::build(&self.models[model].animation.skeleton)
                    });
                }
            }

            Ok(())
        })();

        self.ships = ships;

        result
    }

    fn load_air_move_mount(
        &mut self,
        game: &GameDir,
        entries: &[ModelEntry],
        valid: &ValidIndex,
        sounds: &AniSound,
        textures: &TextureList,
    ) -> Result<()> {
        let clip = Model::direct_clip_for(sounds, AIR_MOVE_MOUNT_MODEL, AIR_MOVE_ACTION)
            .context("model 0x28 has no AirMove action")?;

        let model = self
            .load_model(
                game,
                entries,
                valid,
                sounds,
                textures,
                &ModelSpec::bare(
                    AIR_MOVE_MOUNT_MODEL,
                    ClipChoice::Fixed {
                        slot: clip.slot,
                        sub_frame_ms: clip.sub_frame_ms,
                    },
                ),
            )?
            .context("AirMove model 0x28 did not load")?;

        self.air_move_mount = Some(AirMoveMountTemplate { model, clip });

        Ok(())
    }

    pub fn place_dolls(&mut self, game: &GameDir, dolls: &[Doll]) -> Result<Vec<Option<usize>>> {
        let mut placed = Vec::with_capacity(dolls.len());

        let reg = Arc::clone(&self.registries);

        let Registries {
            entries,
            valid,
            sounds,
            textures,
            effect_textures,
            items,
            list: _,
            arena_visual: _,
            resource_names: _,
        } = &*reg;

        for doll in dolls {
            let look = &doll.look;

            macro_rules! decline {
                ($($arg:tt)*) => {
                    { self.unresolved.push(Unresolved { record : doll.record, face : doll
                    .face, reason : format!($($arg)*), }); placed.push(None); continue; }
                };
            }

            let Some(model_id) = look.model else {
                decline!("appearance class {} has no model", look.class);
            };

            let mount = self.load_mount(
                game,
                entries,
                valid,
                sounds,
                textures,
                effect_textures,
                look,
                doll,
            )?;

            let mounted = mount.is_some();

            let spec = ModelSpec {
                model_id,
                clip: match mounted {
                    true => ClipChoice::Mounted(IDLE_ACTION),
                    false => ClipChoice::Action(IDLE_ACTION),
                },
                parts: look.parts,
                variant: look.variant,
                costume_body: look.costume_body,
            };

            let Some(model) = self.load_model(game, entries, valid, sounds, textures, &spec)?
            else {
                decline!("model {model_id} did not load");
            };

            let tree = FrameTree::build(&self.models[model].animation.skeleton);

            let cape_anchor = look.cape_anchor().unwrap_or(&[]);

            let cape = self.load_cape(
                game,
                entries,
                valid,
                sounds,
                textures,
                effect_textures,
                look,
                doll.face as i16,
                IDLE_ACTION,
            );

            if look.cape.is_some() && cape.is_none() {
                self.unplaced_capes.push(Unresolved {
                    record: doll.record,
                    face: doll.face,
                    reason: "the cape resolved to no seat or no model".to_string(),
                });
            }

            let held: Vec<Held> = appearance::Hand::BOTH
                .into_iter()
                .filter_map(|hand| {
                    let (bone, mesh_id, grip) =
                        look.hand(hand, doll.hands.1, doll.hands.0, items)?;

                    tree.slot(bone as usize)?;

                    Some(Held {
                        hand,
                        bone,
                        mesh_id,
                        grip,
                        glow: super::weapon_glow::special(
                            model_id,
                            mesh_id,
                            look.part_color[hand.part()],
                            glow::effect_class(
                                look.part_tier[hand.part()].min(15),
                                look.part_tint[hand.part()],
                            ),
                        ),
                        ordinary_glow: super::weapon_glow::Ordinary::for_hand(look, hand, mesh_id),
                    })
                })
                .collect();

            let parts = entries
                .iter()
                .find(|e| e.model_id == model_id)
                .map_or(0, |e| e.num_parts as usize);

            let mut swings =
                Swing::for_hands(doll.record, doll.hands, look, parts, items, |bone| {
                    tree.slot(bone as usize).is_some()
                });

            let flags = affect::Flags::build(&doll.affects, &affect::NO_DETAILS, &wearer(look));

            for swing in &mut swings {
                swing.apply_affects(&flags);
            }

            let ridden = mount.as_ref().and(look.mount);

            let mount_model = ridden.and_then(|m| m.model);

            let group = anim_group(look, doll.hands.1, doll.hands.0, items);

            let weapon_class = appearance::weapon_class(doll.hands.1, items);

            let off_weapon_class = appearance::weapon_class(doll.hands.0, items);

            let body_slots = self.anim_slots(model_id);

            let clip = |action: usize| {
                let slots = body_slots.as_ref()?;

                Model::clip_for(sounds, slots, model_id, group, action, mounted)
            };

            let mount_clip = |action: usize| Model::direct_clip_for(sounds, mount_model?, action);

            let clips = ActionClips {
                stand: clip(appearance::ACTION_STAND02),
                walk: clip(appearance::ACTION_WALK),
                run: clip(appearance::ACTION_RUN),
                die: clip(appearance::ACTION_DIE),
                dead: clip(appearance::ACTION_DEAD),
                attack1: clip(appearance::ACTION_ATTACK1),
                attack2: clip(appearance::ACTION_ATTACK2),
                attack3: clip(appearance::ACTION_ATTACK3),
                mount_stand: mount_clip(appearance::ACTION_STAND02),
                mount_walk: mount_clip(appearance::ACTION_WALK),
                mount_run: mount_clip(appearance::ACTION_RUN),
                mount_die: mount_clip(appearance::ACTION_DIE),
                mount_dead: mount_clip(appearance::ACTION_DEAD),
                mount_attack1: mount_clip(appearance::ACTION_ATTACK1),
                mount_attack2: mount_clip(appearance::ACTION_ATTACK2),
                mount_attack3: mount_clip(appearance::ACTION_ATTACK3),
                skill01: clip(appearance::ACTION_SKILL01),
                skill02: clip(appearance::ACTION_SKILL02),
                skill03: clip(appearance::ACTION_SKILL03),
                mount_skill01: mount_clip(appearance::ACTION_SKILL01),
                mount_skill02: mount_clip(appearance::ACTION_SKILL02),
                mount_skill03: mount_clip(appearance::ACTION_SKILL03),
                strike: clip(appearance::ACTION_STRIKE),
                mount_strike: mount_clip(appearance::ACTION_STRIKE),
                emotes: std::array::from_fn(|i| clip(appearance::ACTION_SEAT + i)),
                mount_emotes: std::array::from_fn(|i| mount_clip(appearance::ACTION_SEAT + i)),
                extra: doll.pick_action.map(|action| Extra {
                    action,
                    body: clip(action),
                    mount: mount_clip(action),
                }),
            };

            let glow = self.resolve_glow(game, effect_textures, model, &RigGlow::for_body(look));

            let loaded = &self.models[model];

            placed.push(Some(self.actors.len()));

            let cape_flag = cape.as_ref().and(look.cape);

            self.actors.push(Actor {
                incarnation: super::effect_context::next_incarnation(),
                skill_anchor: None,
                camera_visible: false,
                model,
                model_id,
                record: doll.record,
                entity: doll.entity,
                name: doll.name.clone(),
                status: doll.status,
                alternate_hp: None,
                mount_hp_bonus: 0,
                name_level: doll.name_level,
                level: doll.level,
                guild: doll.guild,
                guild_rank: 0,
                charidx: None,
                guild_name: doll.guild_name.clone(),
                body: doll.face,
                helmet_item: look.helmet_item,
                kingdom: look.kingdom,
                costume_item: 0,
                costume_effect_item: look.costume.unwrap_or(0),
                body_item_tier: usize::try_from(doll.face)
                    .ok()
                    .and_then(|id| self.registries.items.get(id))
                    .map_or(0, |item| item.tier as u8),
                pet_item: doll.pet_item,
                position: doll.position,
                yaw: initial_yaw(look.class, doll.yaw),
                pitch: 0.0,
                roll: 0.0,
                scale: doll.body_scale(),
                evok_size: self.evok_size,
                anim: idle_anim(&clips, loaded),
                action: IDLE_ACTION,
                emote_latch: None,
                emote_hold: false,
                spin: None,
                completion_suspended: false,
                group,
                weapon_class,
                off_weapon_class,
                main_weapon_item: doll.hands.1,
                active_hands: super::swing::hand_activity(look, doll.hands, items),
                off_weapon_item: doll.hands.0,
                primary_tier: look.part_tier[7],
                skill_critical: false,
                skill_child: None,
                recent_hit_ms: None,
                pending_hit: None,
                haste_trail_ms: 0,
                motion_effects: Default::default(),
                impact_grid_ms: 0,
                affect_pair_ms: 0,
                body_cloud_ms: 0,
                body_shadow: Default::default(),
                body_sprites: Default::default(),
                footprint_kind: 1,
                footprint_ms: 0,
                movement_dust: Default::default(),
                body_glow: Default::default(),
                body_points: Default::default(),
                body_butterflies: butterflies::owned(
                    look.class,
                    look.parts[0],
                    &mut self.effect_rng,
                ),
                affect_emit_ms: [0; 4],
                afterimage_ms: 0,
                appearance_epoch: 0,
                skill_afterimage_window: None,
                skill_afterimage_ms: 0,
                transformation: Default::default(),
                projectile_muzzle: Vec3::ZERO,
                clips,
                speed: 0.0,
                is_dead: false,
                tracked: false,
                party_tint: None,
                green_name: doll.green_name || super::nameplate::tagged_name(&doll.name),
                plate_outline: !super::nameplate::tagged_name(&doll.name) && doll.name_level < 10,
                plate_backdrop_color: 0x55AA_0000,
                plate_name_color: if super::nameplate::tagged_name(&doll.name) {
                    0xFFFF_FFAA
                } else {
                    super::nameplate::name_colour(doll.name_level)
                },
                corpse_ms: None,
                removal_mode: super::corpse::MODE_NONE,
                highlight: false,
                target_hud: TargetHud::default(),
                shop_title: doll.shop_title.clone(),
                balloon: None,
                destination: (doll.position.x as i32, doll.position.z as i32),
                mover: walk::Mover::standing(
                    [doll.position.x, doll.position.z],
                    initial_yaw(look.class, doll.yaw),
                ),
                mount_model,
                mount_scale: ridden.map_or(1.0, |m| m.scale),
                mount_kind: ridden.map_or(0, |m| m.kind),
                mounted,
                parts: look.parts,
                glow,
                class: look.class,
                forced_weather_ms: 0,
                affects: doll.affects,
                details: affect::NO_DETAILS,
                flags,
                tree,
                cape_anchor,
                cape_flag,
                cape,
                mount,
                held,
                swings,
                pending: None,
                volley: super::skill_fx::Volley::default(),
            });

            if let Some(actor) = self.actors.last_mut() {
                super::footprints::refresh_type(actor);
            }

            self.mark_rotor_refresh(doll.entity);
        }

        self.load_flight(game, entries, valid, sounds, textures)?;

        Ok(placed)
    }

    pub fn place_one(&mut self, game: &GameDir, doll: &Doll) -> Result<Option<usize>> {
        Ok(self
            .place_dolls(game, std::slice::from_ref(doll))?
            .into_iter()
            .next()
            .flatten())
    }

    pub fn redress(&mut self, game: &GameDir, index: usize, doll: &Doll) -> Result<bool> {
        if index >= self.actors.len() {
            return Ok(false);
        }

        let Some(new) = self.place_one(game, doll)? else {
            return Ok(false);
        };

        let standing = &self.actors[index];

        let incarnation = standing.incarnation;

        let rotation = (standing.pitch, standing.yaw, standing.roll);

        let costume_item = standing.costume_item;

        let guild_rank = standing.guild_rank;

        let name_levels = (standing.level, standing.name_level);

        let name_backdrop = (
            standing.plate_outline,
            standing.plate_backdrop_color,
            standing.green_name,
            standing.plate_name_color,
        );

        let pending_hit = standing.pending_hit;

        let completion = (
            standing.emote_hold,
            standing.completion_suspended,
            standing.spin,
            standing.volley,
        );

        let haste_trail_ms = standing.haste_trail_ms;

        let motion_effects = standing.motion_effects;

        let live_pulse = (
            standing.skill_critical,
            standing.skill_child,
            standing.recent_hit_ms,
            standing.impact_grid_ms,
            standing.projectile_muzzle,
            standing.affect_pair_ms,
            standing.affect_emit_ms,
            standing.appearance_epoch.wrapping_add(1),
            standing.afterimage_ms,
            standing.skill_afterimage_window,
            standing.skill_afterimage_ms,
            standing.transformation,
            standing.body_cloud_ms,
            (standing.footprint_kind, standing.footprint_ms),
            standing.movement_dust,
            standing.body_glow,
            standing.body_points,
        );

        let live = (
            standing.record,
            standing.action,
            standing.speed,
            standing.is_dead,
            standing.highlight,
            standing.target_hud,
            standing.alternate_hp,
            standing.mount_hp_bonus,
            standing.destination,
            standing.mover.clone(),
            standing.details,
            standing.flags,
            standing.mount_kind,
            standing.tracked,
            standing.balloon.clone(),
            standing.party_tint,
        );

        self.actors.swap(index, new);

        self.actors.truncate(new);

        let actor = &mut self.actors[index];

        actor.incarnation = incarnation;

        actor.set_rotation(rotation.0, rotation.1, rotation.2);

        actor.record = live.0;

        actor.action = live.1;

        actor.speed = live.2;

        actor.is_dead = live.3;

        actor.highlight = live.4;

        actor.target_hud = live.5;

        actor.alternate_hp = live.6;

        actor.mount_hp_bonus = live.7;

        actor.costume_item = costume_item;

        actor.guild_rank = guild_rank;

        (actor.level, actor.name_level) = name_levels;

        (
            actor.plate_outline,
            actor.plate_backdrop_color,
            actor.green_name,
            actor.plate_name_color,
        ) = name_backdrop;

        super::nameplate::refresh_backdrop(actor);

        actor.pending_hit = pending_hit;

        (
            actor.emote_hold,
            actor.completion_suspended,
            actor.spin,
            actor.volley,
        ) = completion;

        actor.haste_trail_ms = haste_trail_ms;

        actor.motion_effects = motion_effects;

        actor.skill_critical = live_pulse.0;

        actor.skill_child = live_pulse.1;

        actor.recent_hit_ms = live_pulse.2;

        actor.impact_grid_ms = live_pulse.3;

        actor.projectile_muzzle = live_pulse.4;

        actor.affect_pair_ms = live_pulse.5;

        actor.affect_emit_ms = live_pulse.6;

        actor.appearance_epoch = live_pulse.7;

        actor.afterimage_ms = live_pulse.8;

        actor.skill_afterimage_window = live_pulse.9;

        actor.skill_afterimage_ms = live_pulse.10;

        actor.transformation = live_pulse.11;

        actor.body_cloud_ms = live_pulse.12;

        (actor.footprint_kind, actor.footprint_ms) = live_pulse.13;

        actor.movement_dust = live_pulse.14;

        actor.body_glow = live_pulse.15;

        actor.body_points = live_pulse.16;

        super::footprints::refresh_type(actor);

        actor.destination = live.8;

        actor.mover = live.9;

        actor.details = live.10;

        actor.flags = live.11;

        for swing in &mut actor.swings {
            swing.apply_affects(&actor.flags);
        }

        actor.mount_kind = live.12;

        actor.tracked = live.13;

        actor.balloon = live.14;

        actor.party_tint = live.15;

        Ok(true)
    }

    pub fn hidden(actor: &Actor, viewer: Option<u16>) -> bool {
        let gm_hidden =
            actor.entity < formats::net::spawn::PLAYER_ID_LIMIT && actor.status.flags & 1 != 0;

        gm_hidden
            || actor.flags.get(0xB55) == 1
            || actor.flags.invisible() && Some(actor.entity) != viewer
    }

    pub fn entity(&self, id: u16) -> Option<usize> {
        self.actors.iter().position(|actor| actor.entity == id)
    }

    pub fn take_pulse_plans(&mut self) -> Vec<super::skill_anim::PulsePlan> {
        std::mem::take(&mut self.pulse_plans)
    }

    pub fn take_damage_sounds(&mut self) -> Vec<super::skill_anim::DamageSoundRequest> {
        self.skill_anim.take_damage_sounds()
    }

    pub fn finish_damage_sound(
        &mut self,
        request: super::skill_anim::DamageSoundRequest,
        played: bool,
    ) -> bool {
        self.skill_anim.finish_damage_sound(request, played)
    }

    fn mark_rotor_refresh(&mut self, entity: u16) {
        if entity != 0 && !self.rotor_refresh.contains(&entity) {
            self.rotor_refresh.push(entity);
        }
    }

    fn has_human_rotors(actor: &Actor) -> bool {
        matches!(actor.class, 0x24 | 0x25)
            && actor.parts[1].mesh == 10
            && actor.parts[7].mesh != 0x3A2
    }

    pub fn take_rotor_refreshes(&mut self) -> Vec<(u16, Vec<super::mesh_fx::MeshRotate>)> {
        let dirty = std::mem::take(&mut self.rotor_refresh);

        dirty
            .into_iter()
            .map(|entity| {
                let rotors = self
                    .entity(entity)
                    .and_then(|index| self.actors.get(index))
                    .filter(|actor| Self::has_human_rotors(actor))
                    .map(|actor| {
                        super::mesh_fx::human_rotors(entity, actor.position)
                            .into_iter()
                            .map(|mut rotor| {
                                rotor.owner_key = Some(super::effect_context::ActorKey::of(actor));

                                rotor
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                (entity, rotors)
            })
            .collect()
    }

    pub fn begin_air_move(&mut self, index: usize, now_ms: u32) -> Option<AirMoveRestore> {
        if self.air_move_display.is_some() {
            return None;
        }

        let template = self.air_move_mount?;

        let actor = self.actors.get(index)?;

        let slots = self.anim_slots(actor.model_id)?;

        let body_clip = Model::clip_for(
            &self.registries.sounds,
            &slots,
            actor.model_id,
            actor.group,
            AIR_MOVE_ACTION,
            true,
        )?;

        self.models[actor.model].animation.frames(body_clip.slot)?;

        let worn = appearance::Mount {
            item: 0,
            appearance_item: 0,
            kind: actor.mount_kind,
            model: Some(AIR_MOVE_MOUNT_MODEL),
            parts: [PartRef::EMPTY; appearance::MOUNT_PARTS],
            part_color: [0; appearance::MOUNT_PARTS],
            visual_kind: 0,
            scale: actor.mount_scale,
        };

        let seat = appearance::rider_seat(worn, actor.class, actor.scale)?;

        let tree = FrameTree::build(&self.models[template.model].animation.skeleton);

        let old_glow = actor.mount.as_ref().map_or(NO_GLOW, |mount| mount.glow);

        let body_model = actor.model;

        let actor = &mut self.actors[index];

        let restore = AirMoveRestore {
            entity: actor.entity,
            action: actor.action,
            mounted: actor.mounted,
            mount_model: actor.mount_model,
            mount_scale: actor.mount_scale,
            mount: actor.mount.take(),
            cape_seat: actor.cape.as_ref().map(|cape| cape.seat),
        };

        let mut anim = Anim::new(template.clip.slot, template.clip.sub_frame_ms);

        anim.start_ms = now_ms;

        actor.mount = Some(Mount {
            model: template.model,
            parts: worn.parts,
            hot_effect: false,
            anchor: appearance::AnchorRule {
                bones: &[15],
                adjust: appearance::AnchorAdjust::Copy,
            },
            seat,
            scale: appearance::mount_world_scale(actor.class, actor.scale, actor.mount_scale),
            tree,
            anim,
            glow: old_glow,
        });

        actor.mount_model = Some(AIR_MOVE_MOUNT_MODEL);

        actor.mounted = true;

        actor
            .anim
            .play(body_clip, now_ms, &self.models[body_model], false);

        actor.action = AIR_MOVE_ACTION;

        if let Some(cape) = actor.cape.as_mut() {
            (cape.seat.yaw, cape.seat.pitch) = appearance::cape_rotation(AIR_MOVE_MOUNT_MODEL);

            if let Some(slot) = appearance::cape_clip(true, AIR_MOVE_ACTION) {
                cape.anim.set_clip(slot, now_ms, &self.models[cape.model]);
            }
        }

        self.air_move_display = Some(AirMoveDisplay {
            entity: actor.entity,
            ground_y: actor.position.y,
            draw_offset: [0.0; 2],
            y: actor.position.y,
            yaw: actor.yaw,
            mount_frame: Some(template.clip.sub_frame_ms as i32),
        });

        Some(restore)
    }

    pub fn air_move_active(&self) -> bool {
        self.air_move_display.is_some()
    }

    pub(super) fn refresh_air_move_ground(&mut self, index: usize) {
        if let (Some(actor), Some(display)) =
            (self.actors.get(index), self.air_move_display.as_mut())
            && actor.entity == display.entity
        {
            display.ground_y = actor.position.y;
        }
    }

    pub fn air_move_pose(&self) -> Option<super::portal::AirMovePose> {
        let display = self.air_move_display?;

        let actor = self
            .actors
            .iter()
            .find(|actor| actor.entity == display.entity)?;

        Some(super::portal::AirMovePose {
            position: [actor.position.x, display.y, actor.position.z],
            ground_y: display.ground_y,
            draw_offset: display.draw_offset,
            yaw: display.yaw,
            mount_frame: display.mount_frame,
        })
    }

    pub fn apply_air_move_pose(&mut self, pose: super::portal::AirMovePose) -> bool {
        let Some(display) = self.air_move_display.as_mut() else {
            return false;
        };

        let Some(actor) = self
            .actors
            .iter_mut()
            .find(|actor| actor.entity == display.entity)
        else {
            return false;
        };

        display.ground_y = pose.ground_y;

        display.draw_offset = pose.draw_offset;

        display.y = pose.position[1];

        display.yaw = pose.yaw;

        display.mount_frame = pose.mount_frame;

        actor.position = Vec3::new(pose.position[0], pose.position[1], pose.position[2]);

        actor.yaw = pose.yaw;

        if let (Some(mount), Some(frame)) = (actor.mount.as_mut(), pose.mount_frame)
            && let Ok(frame) = u32::try_from(frame)
        {
            mount.anim.sub_frame_ms = frame;
        }

        true
    }

    pub fn finish_air_move(&mut self, restore: AirMoveRestore, now_ms: u32) -> bool {
        let Some(display) = self.air_move_display else {
            return false;
        };

        if display.entity != restore.entity {
            return false;
        }

        let Some(index) = self.entity(display.entity) else {
            self.air_move_display = None;

            return false;
        };

        self.air_move_display = None;

        let actor = &mut self.actors[index];

        actor.position.y = display.ground_y;

        actor.yaw = display.yaw;

        actor
            .mover
            .reset_position([actor.position.x, actor.position.z]);

        actor.mover.yaw = actor.yaw;

        actor.destination = (actor.position.x as i32, actor.position.z as i32);

        actor.mount = restore.mount;

        actor.mount_model = restore.mount_model;

        actor.mount_scale = restore.mount_scale;

        actor.mounted = restore.mounted;

        if let (Some(cape), Some(seat)) = (actor.cape.as_mut(), restore.cape_seat) {
            cape.seat = seat;
        }

        let action = match restore.action {
            appearance::ACTION_WALK | appearance::ACTION_RUN => appearance::ACTION_STAND02,
            action => action,
        };

        actor.play(action, now_ms, &self.models);

        true
    }

    pub fn remove(&mut self, id: u16) -> Option<usize> {
        let index = self.entity(id)?;

        self.actors.remove(index);

        for actor in &mut self.actors {
            if let Some(pending) = actor.pending.as_mut()
                && pending.target == Some(id)
            {
                pending.target = None;
            }
        }

        self.mark_rotor_refresh(id);

        self.skill_anim.forget(id);

        if self
            .air_move_display
            .is_some_and(|display| display.entity == id)
        {
            self.air_move_display = None;
        }

        Some(index)
    }

    #[allow(clippy::too_many_arguments)]
    fn load_mount(
        &mut self,
        game: &GameDir,
        entries: &[ModelEntry],
        valid: &ValidIndex,
        sounds: &AniSound,
        textures: &TextureList,
        effects: &TextureList,
        look: &Look,
        doll: &Doll,
    ) -> Result<Option<Mount>> {
        let Some(worn) = look.mount else {
            return Ok(None);
        };

        macro_rules! decline {
            ($($arg:tt)*) => {
                { self.unplaced_mounts.push(Unresolved { record : doll.record, face :
                doll.face, reason : format!($($arg)*), }); return Ok(None); }
            };
        }

        let Some(mount_id) = worn.model else {
            decline!("mount {} resolves to no model", worn.item);
        };

        let (Some(anchor), Some(seat)) = (look.rider_anchor(), look.rider_seat(doll.body_scale()))
        else {
            decline!("model {mount_id} publishes no rider anchor");
        };

        let mut parts = [PartRef::EMPTY; appearance::PARTS];

        parts[..appearance::MOUNT_PARTS].copy_from_slice(&worn.parts);

        let spec = ModelSpec {
            model_id: mount_id,
            clip: ClipChoice::Action(IDLE_ACTION),
            parts,
            variant: 0,
            costume_body: 0,
        };

        let Some(model) = self.load_model(game, entries, valid, sounds, textures, &spec)? else {
            decline!("mount model {mount_id} did not load");
        };

        let loaded = &self.models[model];

        Ok(Some(Mount {
            model,
            parts: worn.parts,
            hot_effect: false,
            anchor,
            seat,
            scale: appearance::mount_world_scale(look.class, doll.body_scale(), worn.scale),
            tree: FrameTree::build(&loaded.animation.skeleton),
            anim: Anim::new(loaded.slot, loaded.sub_frame_ms),
            glow: self.resolve_glow(game, effects, model, &RigGlow::for_mount(&worn)),
        }))
    }

    #[allow(clippy::too_many_arguments)]
    fn load_cape(
        &mut self,
        game: &GameDir,
        entries: &[ModelEntry],
        valid: &ValidIndex,
        sounds: &AniSound,
        textures: &TextureList,
        effects: &TextureList,
        look: &Look,
        body: i16,
        action: usize,
    ) -> Option<Cape> {
        let worn = look.cape?;

        let seat = look.cape_seat(body)?;

        look.cape_anchor()?;

        let slot = look.cape_clip(action)?;

        let mut parts = [PartRef::EMPTY; appearance::PARTS];

        parts[0] = worn.part_for_body(look.costume_body);

        let spec = ModelSpec {
            model_id: appearance::CAPE_MODEL,
            clip: ClipChoice::Fixed {
                slot,
                sub_frame_ms: appearance::CAPE_SUB_FRAME_MS,
            },
            parts,
            variant: 0,
            costume_body: look.costume_body,
        };

        let model = self
            .load_model(game, entries, valid, sounds, textures, &spec)
            .ok()
            .flatten()?;

        let glow = self.resolve_glow(
            game,
            effects,
            model,
            &RigGlow::for_cape(&worn, look.costume_body, true),
        );

        let refined = appearance::Cape {
            rank: cape_refinement_rank(worn.refine),
            ..worn
        };

        let refinement_glow = self.resolve_glow(
            game,
            effects,
            model,
            &RigGlow::for_cape(&refined, look.costume_body, true),
        );

        let loaded = &self.models[model];

        Some(Cape {
            model,
            seat,
            tree: FrameTree::build(&loaded.animation.skeleton),
            anim: Anim::new(loaded.slot, loaded.sub_frame_ms),
            glow,
            refinement_glow,
        })
    }

    pub fn attributes(&self) -> Option<&attribute_map::AttributeMap> {
        self.attributes.as_ref()
    }

    pub fn rewindow(&mut self, anchor: (u8, u8)) -> bool {
        let moved = self.zone != anchor;

        let grids: Vec<((u8, u8), &[i8])> = self
            .zone_heights
            .iter()
            .map(|(zone, grid)| (*zone, grid.as_slice()))
            .collect();

        self.window = collision::window_over(&grids, anchor, self.attributes.as_ref());

        self.placement_window.clone_from(&self.window);

        self.zone = anchor;

        moved
    }

    pub fn add_neighbour(
        &mut self,
        game: &GameDir,
        terrain: &Terrain,
        records: &[ObjRecord],
        light0: [f32; 4],
        light1: [f32; 4],
    ) -> Result<()> {
        self.add_prepared_neighbour(game, terrain, records, light0, light1, None)
    }

    pub(crate) fn add_prepared_neighbour(
        &mut self,
        game: &GameDir,
        terrain: &Terrain,
        records: &[ObjRecord],
        light0: [f32; 4],
        light1: [f32; 4],
        prepared: Option<(Vec<i8>, Vec<i8>)>,
    ) -> Result<()> {
        let reg = Arc::clone(&self.registries);

        let Registries {
            entries,
            valid,
            sounds,
            textures,
            ..
        } = &*reg;

        let (heights, relief) = match prepared {
            Some(grids) => grids,
            None => {
                let masks =
                    object_bin::parse(&game.read(OBJECT_BIN).context("reading object.bin")?)
                        .context("parsing object.bin")?;

                let relief = terrain.build_height_grid();

                let mut heights = relief.clone();

                collision::stamp_all(&mut heights, &masks, records);

                (heights, relief)
            }
        };

        let zone = (terrain.zone_x, terrain.zone_y);

        self.footprint_foliage
            .insert(zone, super::footprints::foliage(records));

        if !self.zone_heights.iter().any(|(at, _)| *at == zone) {
            self.zone_heights.push((zone, heights.clone()));
        }

        let window = collision::window(&heights, zone, self.attributes.as_ref());

        let more = grass::Field::build(records, terrain, &window, light0, self.grass.field.enabled);

        let mut placed = self
            .grass
            .field
            .blades
            .iter()
            .map(|b| (b.zone, b.record))
            .collect::<std::collections::BTreeSet<_>>();

        for blade in more.blades {
            if placed.insert((blade.zone, blade.record)) {
                self.grass.field.blades.push(blade);
            }
        }

        self.load_grass(game, entries, valid, sounds, textures)?;

        let mut grove = trees::Grove::build(records, terrain, &relief, light0, light1);

        for spec in grove.specs() {
            let mut parts = [PartRef::EMPTY; appearance::PARTS];

            parts[0] = spec.1;

            let model = ModelSpec {
                model_id: spec.0,
                clip: ClipChoice::Fixed {
                    slot: trees::TREE_SLOT,
                    sub_frame_ms: trees::TREE_SUB_FRAME_MS,
                },
                parts,
                variant: 0,
                costume_body: 0,
            };

            let loaded = self.load_model(game, entries, valid, sounds, textures, &model)?;

            for tree in grove.trees.iter_mut() {
                if (tree.model_id, trees::part_zero(tree.kind)) == spec {
                    tree.model = loaded;
                }
            }

            if let Some(model) = loaded {
                self.tree_scratch
                    .entry(model)
                    .or_insert_with(|| FrameTree::build(&self.models[model].animation.skeleton));
            }
        }

        self.grove.trees.append(&mut grove.trees);

        self.grove.unresolved.append(&mut grove.unresolved);

        let ordinal = self.school.fish.len();

        let mut more = fish::School::build(records, (terrain.zone_x, terrain.zone_y), ordinal);

        self.school.fish.append(&mut more.fish);

        self.load_school(game, entries, valid, sounds, textures)?;

        let mut flight = butterflies::Flight::build(records, (terrain.zone_x, terrain.zone_y));

        self.flight.butterflies.append(&mut flight.butterflies);

        self.load_flight(game, entries, valid, sounds, textures)?;

        let mut raft = floats::Raft::build(
            records,
            (terrain.zone_x, terrain.zone_y),
            |_, _| None,
            |x, z| super::lights::sample(&[terrain], x, z),
        );

        for float in raft.floats.drain(..) {
            if !self
                .raft
                .floats
                .iter()
                .any(|f| f.zone == float.zone && f.record == float.record)
            {
                self.raft.floats.push(float);
            }
        }

        self.load_raft(game, entries, valid, sounds, textures)?;

        for ship in super::ships::build(records, (terrain.zone_x, terrain.zone_y)) {
            if !self
                .ships
                .iter()
                .any(|s| s.zone == ship.zone && s.record == ship.record)
            {
                self.ships.push(ship);
            }
        }

        self.load_ships(game, entries, valid, sounds, textures)?;

        Ok(())
    }

    pub fn held_mesh_ids(&self) -> Vec<u32> {
        let mut ids: Vec<u32> = self
            .actors
            .iter()
            .flat_map(|a| {
                a.held
                    .iter()
                    .map(|h| h.mesh_id)
                    .chain(super::siege_mesh::mesh(a.parts[7].mesh))
            })
            .collect();

        ids.sort_unstable();

        ids.dedup();

        ids
    }

    pub fn upload_textures(&self, gpu: &Gpu, renderer: &SkinRenderer) -> Vec<SkinTexture> {
        let uploaded: Vec<_> = self
            .textures
            .iter()
            .map(|t| {
                render::texture::upload(
                    gpu,
                    &t.texture,
                    t.alpha_policy(),
                    &format!("skin texture {}", t.path),
                )
            })
            .collect();

        let plain = uploaded.iter().map(|t| renderer.bind_texture(gpu, &t.view));

        let paired = self.glow_pairs.iter().map(|&(base, sheet)| {
            renderer.bind_glow(gpu, &uploaded[base].view, &uploaded[sheet].view)
        });

        plain.chain(paired).collect()
    }

    pub fn tick(&mut self, now_ms: u32) -> bool {
        self.tick_with_reach(
            now_ms,
            Reach {
                eye: Vec3::ZERO,
                far: None,
            },
        )
    }

    pub(crate) fn ticked_at(&self, now_ms: u32) -> bool {
        self.ticked_ms == Some(now_ms)
    }

    pub(crate) fn tick_with_reach(&mut self, now_ms: u32, reach: Reach) -> bool {
        if self.ticked_ms == Some(now_ms) {
            return false;
        }

        self.ticked_ms = Some(now_ms);

        if now_ms % 5000 < 100
            && let Some(table) = self.registries.sounds.class(20)
        {
            let sound = table.sound[1];

            if (1..512).contains(&sound) {
                self.tick_sounds.extend(
                    self.actors
                        .iter()
                        .filter(|a| a.model_id == 20 && a.parts[1].mesh == 0)
                        .map(|_| i64::from(sound)),
                );
            }
        }

        let field = HeightField::window(&self.window, self.zone);

        let viewer = self.viewer;

        let mut viewer_arrived = false;

        for actor in &mut self.actors {
            actor.expire_recent_hit(now_ms);

            if let Some(flight) = actor.body_butterflies.as_mut() {
                flight.birth_ms.get_or_insert(now_ms);
            }
        }

        for actor in &mut self.actors {
            let step = actor.mover.advance(now_ms, actor.speed, &field);

            super::attack::tick_spin(actor, &self.models, now_ms);

            if let Some(display) = self
                .air_move_display
                .as_mut()
                .filter(|display| display.entity == actor.entity)
            {
                apply_air_move_step(actor, &step, display);
            } else {
                let ground = scene_ground_height(self.scene_id, step.position[1], step.ground);

                let at = Vec3::new(step.position[0], ground, step.position[1]);

                let height = special_height(
                    actor.class,
                    actor.parts[7].mesh,
                    at,
                    self.zone,
                    &self.placement_window,
                    ground,
                );

                actor.position = Vec3::new(at.x, height, at.z);

                if actor.yaw != actor.mover.yaw {
                    actor.set_rotation(0.0, actor.mover.yaw, 0.0);
                }
            }

            if step.arrived {
                if actor.action == appearance::ACTION_WALK || actor.action == appearance::ACTION_RUN
                {
                    viewer_arrived |= viewer == Some(actor.entity);

                    actor.play(appearance::ACTION_STAND02, now_ms, &self.models);

                    actor.mover.flatten(step.index, actor.speed);

                    actor.mover.arrive();
                } else if actor.action == appearance::ACTION_STAND01
                    || actor.action == appearance::ACTION_STAND02
                {
                    viewer_arrived |= viewer == Some(actor.entity);

                    actor.mover.arrive();
                }
            } else if !actor.mover.server_driven {
                let action = gait(
                    self.models[actor.model].model_id,
                    DEMO_BODY_SCALE,
                    actor.mount_model,
                    actor.mount_scale,
                    actor.speed,
                );

                actor.play(action, now_ms, &self.models);
            }

            if actor.completion_suspended
                || super::attack::advance_volley(
                    actor,
                    &self.models,
                    now_ms,
                    reach.admits(actor.position),
                )
            {
                actor.position.y += corpse::sink(actor, now_ms);

                continue;
            }

            if corpse::settle(actor, &self.models, now_ms, viewer) {
                actor.volley.armed = false;

                if viewer == Some(actor.entity) {
                    self.viewer_death_landed = true;
                }

                if let Some(recipe) = corpse::completion_burst(actor) {
                    self.death_particles.push((now_ms, recipe));
                }
            }

            if matches!(actor.action, 11 | 12) {
            } else if matches!(actor.action, 13 | 15) || actor.emote_hold {
                super::attack::settle(actor, &self.models, now_ms);
            } else if self.skill_anim.active_slot(actor.entity).is_some() {
                let complete = self
                    .models
                    .get(actor.model)
                    .is_some_and(|model| corpse::clip_over(actor, model, now_ms));

                if complete {
                    match self.skill_anim.clip_completed(actor.entity) {
                        super::skill_anim::Advance::Play(action) => {
                            actor.play(action, now_ms, &self.models);
                        }
                        super::skill_anim::Advance::Stand02 => {
                            actor.clear_hand_actions();

                            actor.flags.set(0xB30, 0);

                            actor.skill_afterimage_window = None;

                            actor.play(super::skill_anim::STAND02, now_ms, &self.models);
                        }
                        super::skill_anim::Advance::Inactive => {}
                    }
                }
            } else {
                if super::attack::settle(actor, &self.models, now_ms) {
                    actor.flags.set(0xB30, 0);

                    actor.skill_afterimage_window = None;
                }
            }

            actor.position.y += corpse::sink(actor, now_ms);
        }

        viewer_arrived
    }

    #[allow(clippy::too_many_arguments)]
    pub fn frame(
        &mut self,
        now_ms: u32,
        view: &Matrix4,
        eye: Vec3,
        pose_reach: Option<f32>,
        fog: SkinFog,
        alpha: SkinAlpha,
        ground: &[&Terrain],
    ) -> Frame {
        self.frame_reusing(
            now_ms,
            view,
            eye,
            pose_reach,
            fog,
            alpha,
            ground,
            Frame::default(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn frame_reusing(
        &mut self,
        now_ms: u32,
        view: &Matrix4,
        eye: Vec3,
        pose_reach: Option<f32>,
        fog: SkinFog,
        alpha: SkinAlpha,
        ground: &[&Terrain],
        mut scratch: Frame,
    ) -> Frame {
        scratch.clear();

        let Frame {
            mut objects,
            mut palettes,
            mut motion_keys,
            mut draws,
            mut weapons,
            mut spawned,
            mut ground_marks,
            mut sparks,
            mut trails,
            mut skin_fires,
            mut poison,
            afterimage_sound: _,
        } = scratch;

        let mut fired = Vec::new();

        let alpha_reference = alpha.opaque;

        self.tick_with_reach(
            now_ms,
            Reach {
                eye,
                far: pose_reach,
            },
        );

        let reach = Reach {
            eye,
            far: pose_reach,
        };

        let viewer = self.viewer;

        let claimed = self.hovered;

        let claimed_doll = self.hovered_doll;

        let detail = self.effects_detail;

        let field_scene = self.scene_id == super::world::SCENE_FIELD;

        let viewer_position = viewer
            .and_then(|entity| self.entity(entity))
            .and_then(|index| self.actors.get(index))
            .map(|actor| actor.position);

        let mut pulse_plans = Vec::new();

        let mut body_objects = Vec::new();

        let pulses_enabled = self.pulsed_ms != Some(now_ms);

        self.pulsed_ms = Some(now_ms);

        for actor in &mut self.actors {
            let hidden = ActorScene::hidden(actor, viewer);

            let posed = if field_scene {
                actor.camera_visible
            } else {
                reach.admits(actor.position)
            };

            let siege = super::siege_mesh::mesh(actor.parts[7].mesh)
                .filter(|id| self.siege_meshes[(*id - 2888) as usize]);

            let draw_rigs = !hidden && posed && actor.class != 45 && siege.is_none();

            super::body_sprites::refresh(actor, self.effect_environment);

            let owns_hover = if field_scene {
                claimed.is_some_and(|id| id == actor.entity)
            } else {
                claimed_doll == Some(actor.incarnation)
            };

            let highlight = owns_hover && !actor.flags.invisible();

            let recent_hit = field_scene && actor.recently_hit(now_ms);

            let placement = actor.mount_world();

            let mut mount_emission = None;

            let body_world = match actor.mount.as_mut() {
                None => placement,
                Some(mount) => {
                    let model = &self.models[mount.model];

                    if actor.class == 0x28 {
                        (mount.seat.offset, mount.seat.scale) = appearance::rider_offsets(
                            model.model_id,
                            mount.parts,
                            actor.mount_scale,
                            actor.class,
                            actor.scale,
                        );
                    }

                    if posed {
                        let pose_ms = if model.model_id == 49
                            && mount.parts[0].mesh == 24
                            && matches!(
                                actor.action,
                                appearance::ACTION_WALK | appearance::ACTION_RUN
                            ) {
                            mount.anim.blend_from = 0;

                            mount.anim.start_ms
                        } else {
                            now_ms
                        };

                        pose_anim(
                            &mut mount.tree,
                            model,
                            &mut mount.anim,
                            pose_ms,
                            &placement,
                            self.render_lod.smoothing,
                        );
                    }

                    if draw_rigs {
                        if model.model_id != 38 || actor.model_id == 38 {
                            actor.body_points.publish(model.model_id, |id| {
                                mount
                                    .tree
                                    .slot(id)
                                    .map(|_| Matrix4(mount.tree.combined(id)))
                            });
                        }

                        mount_emission =
                            Some((model.model_id, mount.parts[0].mesh, mount.hot_effect));

                        let before = draws.len();

                        emit_with_glow(
                            model,
                            &mount.tree,
                            &self.textures,
                            &mount.glow,
                            Draw {
                                view,
                                eye,
                                field_scene,
                                fog,
                                alpha_reference,
                                highlight,
                                recent_hit: false,
                                invisible: actor.flags.invisible(),
                            },
                            (&mut objects, &mut palettes, &mut draws),
                        );

                        tag_skin_motion(
                            &mut motion_keys,
                            palettes.len(),
                            &draws[before..],
                            actor.incarnation,
                            1,
                        );

                        if draws.len() != before {
                            if highlight {
                                self.target_affects = actor.affects;
                            } else if claimed.is_none() {
                                self.target_affects = affect::NO_AFFECTS;
                            }
                        }
                    }

                    mount
                        .anchor()
                        .map_or(UNPUBLISHED_ANCHOR, |a| mount.rider_world(&a))
                }
            };

            if pulses_enabled
                && actor.class != 45
                && let Some((model, mesh, hot)) = mount_emission
            {
                spawned.extend(super::body_effects::skin_emit(
                    actor,
                    model,
                    mesh,
                    hot,
                    now_ms,
                    &mut self.effect_rng,
                ));
            }

            let model = &self.models[actor.model];

            if draw_rigs {
                actor.skill_anchor = super::effect_context::skill_anchor(actor, &self.models);
            }

            if posed {
                pose_anim(
                    &mut actor.tree,
                    model,
                    &mut actor.anim,
                    now_ms,
                    &body_world,
                    self.render_lod.smoothing,
                );

                super::body_glow::publish(actor);

                super::body_points::publish(actor);

                if let Some(point) = super::effect_context::projectile_muzzle(actor, &self.models) {
                    actor.projectile_muzzle = point;
                }

                if pulses_enabled {
                    spawned.extend(super::skill_anim::build_sand_dust(
                        actor.class,
                        actor.action,
                        u32::try_from(actor.anim.frame).unwrap_or(u32::MAX),
                        actor.position,
                    ));
                }

                let mut pulse_exits = false;

                if pulses_enabled
                    && let Some(mut plan) = self.skill_anim.pulse(pulse_input(
                        actor,
                        viewer,
                        viewer_position,
                        field_scene,
                        detail,
                        &self.registries.items,
                        now_ms,
                    ))
                {
                    self.skill_anim.queue_damage_sound(
                        actor.entity,
                        actor.model_id,
                        actor.class,
                        &plan,
                        now_ms,
                    );

                    pulse_exits = plan.exits_function;

                    if actor.flags.get(0xB55) != 0 || actor.flags.invisible() {
                        plan.steps.retain(|step| {
                            matches!(step, super::skill_anim::PulseStep::TouchChild { .. })
                        });
                    }

                    pulse_plans.push(plan);
                }

                if pulses_enabled
                    && !pulse_exits
                    && let Some(trail) = super::actor_effects::attack_body_trail(actor)
                {
                    spawned.push(trail);
                }
            }

            if draw_rigs {
                if pulses_enabled && actor.class != 45 {
                    spawned.extend(super::body_effects::skin_emit(
                        actor,
                        actor.model_id,
                        actor.parts[0].mesh,
                        false,
                        now_ms,
                        &mut self.effect_rng,
                    ));
                }

                if let Some((particles, mark)) =
                    super::actor_effects::body_cloud(actor, now_ms, &mut self.effect_rng)
                {
                    spawned.extend(particles);

                    ground_marks.push(mark);
                }

                if pulses_enabled {
                    self.body_light
                        .raise(actor, self.effect_environment, self.scene_id);

                    spawned.extend(super::body_effects::emit(
                        actor,
                        now_ms,
                        self.effect_environment,
                        &mut self.effect_rng,
                    ));
                }

                super::body_effects::arm_mount(actor, self.effect_environment);

                let before = draws.len();

                let objects_before = objects.len();

                emit_with_glow(
                    model,
                    &actor.tree,
                    &self.textures,
                    &actor.glow,
                    Draw {
                        view,
                        eye,
                        field_scene,
                        fog,
                        alpha_reference,
                        highlight,
                        recent_hit,
                        invisible: actor.flags.invisible(),
                    },
                    (&mut objects, &mut palettes, &mut draws),
                );

                body_objects.push((actor.entity, objects_before..objects.len(), highlight));

                tag_skin_motion(
                    &mut motion_keys,
                    palettes.len(),
                    &draws[before..],
                    actor.incarnation,
                    0,
                );

                if draws.len() != before {
                    if highlight {
                        self.target_affects = actor.affects;
                    } else if claimed.is_none() {
                        self.target_affects = affect::NO_AFFECTS;
                    }
                }

                for weapon in &mut actor.held {
                    let silhouette = held_silhouette(
                        actor.model_id,
                        weapon.bone,
                        actor.flags.get(affect::Flags::TRAIL) == 1,
                    );

                    let world = weapon.world(&Matrix4(actor.tree.combined(weapon.bone as usize)));

                    weapons.push(HeldDraw {
                        motion_key: (
                            actor.incarnation,
                            ((weapon.bone as u64) << 2) | u64::from(silhouette),
                        ),
                        mesh_id: weapon.mesh_id,
                        world,
                        placement: ActorMeshPlacement::Hand,
                        silhouette,
                        glow: weapon.glow,
                        ordinary_glow: weapon.ordinary_glow,
                    });

                    if silhouette {
                        let scale = actor.scale + 0.05;

                        weapons.push(HeldDraw {
                            mesh_id: weapon.mesh_id,
                            silhouette: true,
                            glow: weapon.glow,
                            motion_key: (actor.incarnation, ((weapon.bone as u64) << 2) | 2),
                            placement: ActorMeshPlacement::Hand,
                            ordinary_glow: weapon.ordinary_glow,
                            world: Matrix4::multiply(
                                &Matrix4::scaling(scale, scale, scale),
                                &world,
                            ),
                        });
                    }

                    if !silhouette {
                        weapon.glow = weapon
                            .glow
                            .map(|slot| super::weapon_glow::after_draw(weapon.mesh_id, slot));
                    }
                }
            }

            if !hidden
                && posed
                && actor.class != 45
                && let Some(mesh_id) = siege
            {
                weapons.push(HeldDraw {
                    mesh_id,
                    motion_key: (actor.incarnation, u64::MAX),
                    world: super::siege_mesh::world(mesh_id, actor.position),
                    placement: ActorMeshPlacement::Siege {
                        selected: actor.target_hud.eligible,
                    },
                    silhouette: false,
                    glow: None,
                    ordinary_glow: None,
                });
            }

            let clip_frames = model.animation.frames(actor.anim.slot).unwrap_or(0) as i32;

            let time_base = actor.anim.sub_frame_ms;

            let swing_owner = super::effect_context::ActorKey::of(actor);

            let concealed = actor.flags.get(0xB55) != 0 || actor.flags.invisible();

            for swing in &mut actor.swings {
                if concealed {
                    swing.emitters = super::aura::Emitters::default();
                }

                swing.owner_visible = posed;

                if !posed {
                    swing.end_swing(now_ms);

                    continue;
                }

                let combined = Matrix4(actor.tree.combined(swing.bone as usize));

                swing.follow(grip_world(&swing.grip, &combined));

                let first_particle = spawned.len();

                let mut sink = super::aura::Sink {
                    spawned: &mut spawned,
                    fired: &mut fired,
                };

                swing.emit(detail, now_ms, &mut self.effect_rng, &mut sink);

                for particle in &mut spawned[first_particle..] {
                    particle.conceal_with = Some(swing_owner);
                }

                let bone = usize::try_from(swing.bone)
                    .ok()
                    .filter(|bone| actor.tree.slot(*bone).is_some())
                    .map(|_| combined);

                if let Some(spark) =
                    swing.emit_trail_spark(now_ms, time_base, detail, swing_owner, bone, now_ms)
                {
                    sparks.push(spark);
                }

                if let Some(ribbon) =
                    swing.trail_frame(now_ms, time_base, clip_frames, body_world, combined)
                {
                    trails.push(ribbon);
                }
            }

            let Some(cape) = actor.cape.as_mut() else {
                continue;
            };

            let anchor = actor
                .tree
                .last_combined(actor.cape_anchor)
                .map_or(UNPUBLISHED_ANCHOR, Matrix4);

            let cape_model = &self.models[cape.model];

            let cape_world = cape.world(&anchor);

            if posed {
                pose_anim(
                    &mut cape.tree,
                    cape_model,
                    &mut cape.anim,
                    now_ms,
                    &cape_world,
                    self.render_lod.smoothing,
                );
            }

            if draw_rigs {
                let before = draws.len();

                emit_with_glow(
                    cape_model,
                    &cape.tree,
                    &self.textures,
                    if field_scene && self.cape_refinement {
                        &cape.refinement_glow
                    } else {
                        &cape.glow
                    },
                    Draw {
                        view,
                        eye,
                        field_scene,
                        fog,
                        alpha_reference,
                        highlight,
                        recent_hit: false,
                        invisible: actor.flags.invisible(),
                    },
                    (&mut objects, &mut palettes, &mut draws),
                );

                tag_skin_motion(
                    &mut motion_keys,
                    palettes.len(),
                    &draws[before..],
                    actor.incarnation,
                    2,
                );

                if draws.len() != before {
                    if highlight {
                        self.target_affects = actor.affects;
                    } else if claimed.is_none() {
                        self.target_affects = affect::NO_AFFECTS;
                    }
                }
            }
        }

        apply_pulse_touches(&mut self.actors, &pulse_plans);

        for (entity, range, highlighted) in body_objects {
            let hit = field_scene
                && self
                    .entity(entity)
                    .and_then(|index| self.actors.get(index))
                    .is_some_and(|actor| actor.recently_hit(now_ms));

            if hit && !highlighted {
                for object in &mut objects[range] {
                    object.material_diffuse = RECENT_HIT_DIFFUSE;
                }
            }
        }

        self.pulse_plans.extend(pulse_plans);

        self.tree_frame(
            now_ms,
            view,
            reach,
            ground,
            fog,
            &mut objects,
            &mut palettes,
            &mut draws,
            &mut spawned,
        );

        self.ship_frame(
            now_ms,
            view,
            reach,
            fog,
            alpha_reference,
            &mut objects,
            &mut palettes,
            &mut draws,
        );

        self.fish_frame(
            now_ms,
            view,
            reach,
            ground,
            fog,
            alpha_reference,
            &mut objects,
            &mut palettes,
            &mut draws,
        );

        self.butterfly_frame(
            now_ms,
            view,
            reach,
            fog,
            alpha_reference,
            &mut objects,
            &mut palettes,
            &mut draws,
        );

        self.float_frame(
            now_ms,
            view,
            reach,
            fog,
            alpha_reference,
            &mut objects,
            &mut palettes,
            &mut draws,
        );

        self.grass_frame(
            now_ms,
            view,
            eye,
            fog,
            alpha.blended,
            &mut objects,
            &mut palettes,
            &mut draws,
        );

        super::actor_effects::movement_afterimages(self, now_ms);

        let afterimage_sound = super::actor_effects::skill_afterimages(self, now_ms);

        super::actor_effects::held_skins(self, now_ms);

        let detail = i32::from(!self.effects_detail.effects);

        self.skinfx
            .update_with(now_ms, detail, &mut self.effect_rng, |effect| {
                let owner = super::effect_context::actor(&self.actors, effect.owner?)?;

                Some(super::skinfx::Owner {
                    position: owner.position,
                    yaw: owner.yaw,
                    scale: owner.scale,
                    weapon: super::effect_context::skin_owner_points(owner, &self.models),
                })
            });

        let emitted = self.skinfx.take_emissions();

        for expired in self.skinfx.take_removed() {
            match expired.removal() {
                Some(super::skinfx::Removal::Fire(at)) => skin_fires.push(at),
                Some(super::skinfx::Removal::Orbit(mut child)) => {
                    match self.hydrate_skin(&mut child, now_ms) {
                        Ok(()) => self.skinfx.spawn(*child),
                        Err(error) => {
                            tracing::warn!(
                                model = child.mesh_id,% error,
                                "expired projectile follow-up rig hydration failed"
                            )
                        }
                    }
                }
                None => {}
            }
        }

        for effect in &mut self.skinfx.objects {
            effect.animate_with_smoothing(
                now_ms,
                &self.models,
                &self.registries.sounds,
                self.render_lod.smoothing,
            );
        }

        self.skinfx.submit(
            &super::skinfx::Emit {
                models: &self.models,
                textures: &self.textures,
                view,
                fog,
            },
            detail,
            (&mut objects, &mut palettes, &mut draws),
        );

        let super::skinfx::Emissions {
            mut motes,
            mut beads,
        } = emitted;

        spawned.append(&mut motes);

        poison.append(&mut beads);

        self.maintain_texture_cache(&draws);

        Frame {
            afterimage_sound,
            motion_keys,
            skin_fires,
            ground_marks,
            objects,
            palettes,
            draws,
            weapons,
            spawned,
            sparks,
            trails,
            poison,
        }
    }

    pub fn place_swing_radii(&mut self, mesh_max_z: impl Fn(u32) -> Option<f32>) {
        Self::place_swing_radii_slice(&mut self.actors, mesh_max_z);
    }

    pub(crate) fn place_actor_swing_radii(
        &mut self,
        index: usize,
        mesh_max_z: impl Fn(u32) -> Option<f32>,
    ) {
        Self::place_swing_radii_slice(&mut self.actors[index..=index], mesh_max_z);
    }

    fn place_swing_radii_slice(actors: &mut [Actor], mesh_max_z: impl Fn(u32) -> Option<f32>) {
        for actor in actors {
            for swing in &mut actor.swings {
                if swing.size == swing::DUAL_WIELD_SIZE {
                    continue;
                }

                let mesh = actor
                    .held
                    .iter()
                    .find(|hand| hand.hand == swing.hand)
                    .map(|hand| hand.mesh_id);

                swing.place_blade(mesh.and_then(&mesh_max_z).unwrap_or(0.0));
            }
        }
    }

    pub fn apply_affects(&mut self, index: usize, flags: &formats::net::affect::Flags) {
        let Some(actor) = self.actors.get_mut(index) else {
            return;
        };

        for swing in &mut actor.swings {
            swing.apply_affects(flags);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn tree_frame(
        &mut self,
        now_ms: u32,
        view: &Matrix4,
        reach: Reach,
        ground: &[&Terrain],
        fog: SkinFog,
        objects: &mut Vec<SkinObject>,
        palettes: &mut Vec<BonePalette>,
        draws: &mut Vec<SkinDraw>,
        spawned: &mut Vec<super::effects::Billboard>,
    ) {
        self.refresh_tree_variants(reach);

        for tree in &mut self.grove.trees {
            if !reach.admits(tree.position) {
                continue;
            }

            let (Some(index), Some(mut scratch)) = (
                tree.model,
                tree.model.and_then(|m| self.tree_scratch.remove(&m)),
            ) else {
                continue;
            };

            let model = &self.models[index];

            let rot = [0.0, tree.yaw, 0.0];

            let world = skin_world(
                model.model_id,
                rot,
                1.0,
                tree.position,
                Chirality::AsAuthored,
            );

            let mut anim = Anim::new(model.slot, model.sub_frame_ms);

            pose_anim(
                &mut scratch,
                model,
                &mut anim,
                now_ms.wrapping_add(tree.phase_ms),
                &world,
                self.render_lod.smoothing,
            );

            let (diffuse, emissive) = trees::material(
                super::lights::sample(ground, tree.position.x, tree.position.z),
                self.light0,
                self.light1,
            );

            tree.diffuse = diffuse;

            tree.emissive = if tree.model_id == trees::PALE_MODEL {
                trees::PALE_EMISSIVE
            } else {
                emissive
            };

            let params = tree.params(fog);

            for part in &model.parts {
                let texture = part.texture;

                let params = SkinParams {
                    force_opaque: texture
                        .is_some_and(|index| !self.textures[index].flag.has_alpha()),
                    ..params
                };

                objects.push(SkinObject::new(
                    &params,
                    part.mesh.header.num_matrices,
                    trees::ALPHA_REFERENCE,
                ));

                palettes.push(BonePalette::build(&part.mesh, view, |bone| {
                    Matrix4(scratch.combined(bone as usize))
                }));

                draws.push(SkinDraw {
                    pass: SkinPass::Opaque,
                    part: part.alloc,
                    object: objects.len() as u32 - 1,
                    palette: palettes.len() as u32 - 1,
                    texture,
                });
            }

            self.tree_scratch.insert(index, scratch);

            if let Some(firefly) = tree.firefly(
                now_ms,
                anim.frame,
                self.effects_detail.effects,
                &mut self.effect_rng,
            ) {
                spawned.push(firefly);
            }
        }
    }

    pub fn refresh_tree_variants(&mut self, reach: Reach) -> usize {
        if self.scene_id != super::world::SCENE_FIELD {
            return 0;
        }

        let game = Arc::clone(&self.effect_game);

        let tables = Arc::clone(&self.registries);

        let mut changed = 0;

        for i in 0..self.grove.trees.len() {
            let tree = &self.grove.trees[i];

            if !reach.admits(tree.position) {
                continue;
            }

            let kind = trees::kingdom_kind(tree.kind, self.tree_kingdom);

            if kind == tree.kind {
                continue;
            }

            let model_id = trees::model_index(kind).expect("kingdom tree kinds have models");

            let mut parts = [PartRef::EMPTY; appearance::PARTS];

            parts[0] = trees::part_zero(kind);

            let loaded = match self.load_model(
                &game,
                &tables.entries,
                &tables.valid,
                &tables.sounds,
                &tables.textures,
                &ModelSpec {
                    model_id,
                    parts,
                    clip: ClipChoice::Fixed {
                        slot: 0,
                        sub_frame_ms: 80,
                    },
                    variant: 0,
                    costume_body: 0,
                },
            ) {
                Ok(model) => model,
                Err(error) => {
                    tracing::warn!(
                        kind, model_id,% error, "kingdom tree model unavailable"
                    );

                    None
                }
            };

            let tree = &mut self.grove.trees[i];

            tree.kind = kind;

            tree.model_id = model_id;

            tree.model = loaded;

            self.tree_shadow_stamps.push(tree.position);

            if let Some(model) = loaded {
                self.tree_scratch
                    .entry(model)
                    .or_insert_with(|| FrameTree::build(&self.models[model].animation.skeleton));
            }

            changed += 1;
        }

        changed
    }

    #[allow(clippy::too_many_arguments)]
    fn ship_frame(
        &mut self,
        now_ms: u32,
        view: &Matrix4,
        reach: Reach,
        fog: SkinFog,
        alpha_reference: f32,
        objects: &mut Vec<SkinObject>,
        palettes: &mut Vec<BonePalette>,
        draws: &mut Vec<SkinDraw>,
    ) {
        for ship in &self.ships {
            if !reach.admits(ship.position) {
                continue;
            }

            let (Some(index), Some(mut scratch)) = (
                ship.model,
                ship.model.and_then(|m| self.ship_scratch.remove(&m)),
            ) else {
                continue;
            };

            let model = &self.models[index];

            let world = skin_world(
                60,
                [0.0, ship.yaw, 0.0],
                1.0,
                ship.position,
                Chirality::AsAuthored,
            );

            pose(
                &mut scratch,
                model,
                now_ms,
                &world,
                self.render_lod.smoothing,
            );

            for part in &model.parts {
                let texture = part.texture;

                let params = SkinParams {
                    force_opaque: texture
                        .is_some_and(|index| !self.textures[index].flag.has_alpha()),
                    ..ship.params(fog)
                };

                objects.push(SkinObject::new(
                    &params,
                    part.mesh.header.num_matrices,
                    alpha_reference,
                ));

                palettes.push(BonePalette::build(&part.mesh, view, |bone| {
                    Matrix4(scratch.combined(bone as usize))
                }));

                draws.push(SkinDraw {
                    pass: SkinPass::Opaque,
                    part: part.alloc,
                    object: objects.len() as u32 - 1,
                    palette: palettes.len() as u32 - 1,
                    texture,
                });
            }

            self.ship_scratch.insert(index, scratch);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn fish_frame(
        &mut self,
        now_ms: u32,
        view: &Matrix4,
        reach: Reach,
        ground: &[&Terrain],
        fog: SkinFog,
        alpha_reference: f32,
        objects: &mut Vec<SkinObject>,
        palettes: &mut Vec<BonePalette>,
        draws: &mut Vec<SkinDraw>,
    ) {
        for fish in &self.school.fish {
            let at = fish.position(now_ms);

            if !reach.admits(at) {
                continue;
            }

            let (Some(index), Some(mut scratch)) = (
                fish.model,
                fish.model.and_then(|m| self.fish_scratch.remove(&m)),
            ) else {
                continue;
            };

            let model = &self.models[index];

            let rot = [0.0, fish.yaw(now_ms), 0.0];

            let world = skin_world(model.model_id, rot, fish.speed, at, Chirality::AsAuthored);

            pose(
                &mut scratch,
                model,
                fish.pose_ms(now_ms),
                &world,
                self.render_lod.smoothing,
            );

            let params = fish.params(
                super::lights::sample(ground, at.x, at.z),
                self.light0,
                self.light1,
                fog,
            );

            for part in &model.parts {
                let texture = part.texture;

                let params = SkinParams {
                    force_opaque: texture
                        .is_some_and(|index| !self.textures[index].flag.has_alpha()),
                    glow: fish.glow.map(|p| p.glow),
                    ..params
                };

                objects.push(SkinObject::new(
                    &params,
                    part.mesh.header.num_matrices,
                    alpha_reference,
                ));

                palettes.push(BonePalette::build(&part.mesh, view, |bone| {
                    Matrix4(scratch.combined(bone as usize))
                }));

                draws.push(SkinDraw {
                    pass: SkinPass::Blended,
                    part: part.alloc,
                    object: objects.len() as u32 - 1,
                    palette: palettes.len() as u32 - 1,
                    texture: texture
                        .map(|base| fish.glow.map_or(base, |p| self.textures.len() + p.pair)),
                });
            }

            self.fish_scratch.insert(index, scratch);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn butterfly_frame(
        &mut self,
        now_ms: u32,
        view: &Matrix4,
        reach: Reach,
        fog: SkinFog,
        alpha_reference: f32,
        objects: &mut Vec<SkinObject>,
        palettes: &mut Vec<BonePalette>,
        draws: &mut Vec<SkinDraw>,
    ) {
        let viewer = self.viewer;

        let owned = self
            .actors
            .iter_mut()
            .filter(|a| !Self::hidden(a, viewer) && reach.admits(a.position))
            .filter_map(|a| a.body_butterflies.as_mut().map(|f| (a.position, f)))
            .flat_map(|(owner, f)| {
                let birth = f.birth_ms;

                f.butterflies.iter_mut().filter_map(move |b| {
                    let at = b.position(now_ms.wrapping_sub(birth?))
                        + owner
                        + Vec3::Y * if b.arm == 2 { 2.0 } else { 1.5 };

                    Some((b, at))
                })
            });

        let placed = self.flight.butterflies.iter_mut().map(|b| {
            let at = b.position(now_ms);

            (b, at)
        });

        for (butterfly, at) in placed.chain(owned) {
            if !reach.admits(at) {
                continue;
            }

            let (Some(index), Some(mut scratch)) = (
                butterfly.model,
                butterfly
                    .model
                    .and_then(|m| self.butterfly_scratch.remove(&m)),
            ) else {
                continue;
            };

            let model = &self.models[index];

            let rot = [0.0, butterfly.yaw, 0.0];

            let world = skin_world(
                model.model_id,
                rot,
                butterfly.scale,
                at,
                Chirality::AsAuthored,
            );

            let pose_ms = butterfly.pose_ms(now_ms);

            if let Some(anim) = butterfly.anim.as_mut() {
                pose_anim(
                    &mut scratch,
                    model,
                    anim,
                    pose_ms,
                    &world,
                    self.render_lod.smoothing,
                );
            } else {
                pose(
                    &mut scratch,
                    model,
                    pose_ms,
                    &world,
                    self.render_lod.smoothing,
                );
            }

            let distance = ((at.x - reach.eye.x).powi(2) + (at.z - reach.eye.z).powi(2)).sqrt();

            let params = butterfly.params(distance, self.light0, self.light1, fog);

            for part in &model.parts {
                let texture = part.texture;

                let params = SkinParams {
                    force_opaque: texture
                        .is_some_and(|index| !self.textures[index].flag.has_alpha()),
                    ..params
                };

                objects.push(SkinObject::new(
                    &params,
                    part.mesh.header.num_matrices,
                    alpha_reference,
                ));

                palettes.push(BonePalette::build(&part.mesh, view, |bone| {
                    Matrix4(scratch.combined(bone as usize))
                }));

                draws.push(SkinDraw {
                    pass: SkinPass::Blended,
                    part: part.alloc,
                    object: objects.len() as u32 - 1,
                    palette: palettes.len() as u32 - 1,
                    texture,
                });
            }

            self.butterfly_scratch.insert(index, scratch);
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn float_frame(
        &mut self,
        now_ms: u32,
        view: &Matrix4,
        reach: Reach,
        fog: SkinFog,
        alpha_reference: f32,
        objects: &mut Vec<SkinObject>,
        palettes: &mut Vec<BonePalette>,
        draws: &mut Vec<SkinDraw>,
    ) {
        let Some(mut scratch) = self.float_scratch.take() else {
            return;
        };

        for float in self.raft.floats.iter().filter(|f| f.has_mesh()) {
            let Some(index) = float.model else {
                continue;
            };

            if !reach.admits(float.position) {
                continue;
            }

            let model = &self.models[index];

            let rot = [0.0, float.yaw, 0.0];

            let scale = floats::FLOAT_SCALE;

            let at = float.position;

            let world = skin_world(model.model_id, rot, scale, at, Chirality::AsAuthored);

            pose(
                &mut scratch,
                model,
                now_ms.wrapping_add(float.phase_ms),
                &world,
                self.render_lod.smoothing,
            );

            let params = float.params(fog);

            for part in &model.parts {
                let texture = part.texture;

                let params = SkinParams {
                    force_opaque: texture
                        .is_some_and(|index| !self.textures[index].flag.has_alpha()),
                    ..params
                };

                objects.push(SkinObject::new(
                    &params,
                    part.mesh.header.num_matrices,
                    alpha_reference,
                ));

                palettes.push(BonePalette::build(&part.mesh, view, |bone| {
                    Matrix4(scratch.combined(bone as usize))
                }));

                draws.push(SkinDraw {
                    pass: SkinPass::Blended,
                    part: part.alloc,
                    object: objects.len() as u32 - 1,
                    palette: palettes.len() as u32 - 1,
                    texture,
                });
            }
        }

        self.float_scratch = Some(scratch);
    }

    #[allow(clippy::too_many_arguments)]
    fn grass_frame(
        &mut self,
        now_ms: u32,
        view: &Matrix4,
        eye: Vec3,
        fog: SkinFog,
        alpha_reference: f32,
        objects: &mut Vec<SkinObject>,
        palettes: &mut Vec<BonePalette>,
        draws: &mut Vec<SkinDraw>,
    ) {
        let Some(tree) = self.grass_tree.as_mut() else {
            return;
        };

        for (_, blade, distance_squared) in self.grass.field.visible(eye) {
            let Some(model) = blade.model else {
                continue;
            };

            let model = &self.models[model];

            let clips = ClipTable::new(model.animation.matrices(), model.animation.ani_num_bones);

            let anim_base = model.animation.anim_base(model.slot).unwrap_or(0);

            let frames = model.animation.frames(model.slot).unwrap_or(0);

            let request = PoseRequest {
                model: model.model_id,
                slot: model.slot,
                anim_base,
                frames,
                sub_frame_ms: model.sub_frame_ms,
                elapsed_ms: now_ms.wrapping_add(blade.phase_ms),
                blend_from: 0,
                blend_matrix: 0,
                smoothing: self.render_lod.smoothing,
            };

            tree.sample(&request, &clips);

            let rot = [0.0, blade.yaw, 0.0];

            let world = skin_world(
                model.model_id,
                rot,
                1.0,
                blade.position,
                Chirality::AsAuthored,
            );

            tree.combine(&world.as_d3dx());

            let params = self.grass.field.params(blade, distance_squared, fog);

            for part in &model.parts {
                let texture = part.texture;

                objects.push(SkinObject::new(
                    &params,
                    part.mesh.header.num_matrices,
                    alpha_reference,
                ));

                palettes.push(BonePalette::build(&part.mesh, view, |bone| {
                    Matrix4(tree.combined(bone as usize))
                }));

                draws.push(SkinDraw {
                    pass: SkinPass::Blended,
                    part: part.alloc,
                    object: objects.len() as u32 - 1,
                    palette: palettes.len() as u32 - 1,
                    texture,
                });
            }
        }
    }

    pub fn triangle_count(&self) -> u32 {
        self.arena.triangle_count()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SkinAlpha {
    pub opaque: f32,
    pub blended: f32,
}

fn tag_skin_motion(
    keys: &mut Vec<Option<render::skin::MotionKey>>,
    palettes: usize,
    draws: &[SkinDraw],
    incarnation: u64,
    rig: u64,
) {
    keys.resize(palettes, None);

    for draw in draws {
        let variant = if draw.pass == SkinPass::Highlight {
            1
        } else {
            0
        };

        if let Some(key) = keys.get_mut(draw.palette as usize) {
            *key = Some((
                incarnation,
                ((draw.part as u64) << 4) | (rig << 1) | variant,
            ));
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Frame {
    pub ground_marks: Vec<super::shade::Shade>,
    pub skin_fires: Vec<Vec3>,
    pub afterimage_sound: bool,
    pub objects: Vec<SkinObject>,
    pub palettes: Vec<BonePalette>,
    pub motion_keys: Vec<Option<render::skin::MotionKey>>,
    pub draws: Vec<SkinDraw>,
    pub weapons: Vec<HeldDraw>,
    pub spawned: Vec<super::effects::Billboard>,
    pub sparks: Vec<super::spark::BoneSpark>,
    pub poison: Vec<super::skill::poison::Built>,
    pub trails: Vec<super::swing::Ribbon>,
}

impl Frame {
    pub(crate) fn clear(&mut self) {
        self.afterimage_sound = false;

        self.objects.clear();

        self.palettes.clear();

        self.motion_keys.clear();

        self.draws.clear();

        self.weapons.clear();

        self.spawned.clear();

        self.ground_marks.clear();

        self.skin_fires.clear();

        self.sparks.clear();

        self.poison.clear();

        self.trails.clear();
    }
}
