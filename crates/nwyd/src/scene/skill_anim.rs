use super::actors::Actor;
use formats::data::skill_data::{Skill, SkillData};
use glam::{Vec2, Vec3};
use rustc_hash::FxHashMap;
use std::ops::RangeInclusive;

pub const SLOTS: usize = 4;

pub const PLAYED_SLOT: usize = 0;

pub const NO_CLIP: i32 = -1;

pub const CHAIN_LOW_MAX: u16 = 0x67;

pub const CHAIN_HIGH: RangeInclusive<u16> = 200..=247;

pub const CHAIN_ROWS: usize = 152;

pub const GROUND_MAX: i8 = i8::MAX;

pub const SELF_ONLY: RangeInclusive<usize> = 0x38..=0x3F;

pub const NO_INTERRUPT_CLASS: i32 = 0x42;

pub const NO_INTERRUPT_ACTIONS: [usize; 3] = [7, 8, 9];

pub const SCALED_ACTIONS: RangeInclusive<usize> = 4..=9;

pub const COMBO_DIVISOR: f32 = 1.2;

pub const COMBO_THRESHOLD: i32 = 2;

pub const STAND02: usize = 1;

pub const TARGET_WEIGHT: f32 = 0.8;

pub const CASTER_WEIGHT: f32 = 0.2;

pub const SPECIAL_TARGET_WEIGHT: f32 = 0.4;

pub const SPECIAL_CASTER_WEIGHT: f32 = 0.6;

pub const SPECIAL_TARGET_CLASS: i32 = 0x38;

pub const SPECIAL_TARGET_PART0_MESH: i16 = 0;

pub const TARGET_SCALE_FACTOR: f64 = 0.3;

pub const TARGET_SCALE_MAX: f32 = 2.0;

pub const GROUND_SCALE: f32 = 0.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnchorTarget {
    pub position: Vec2,
    pub model: u32,
    pub scale: f32,
    pub class: i32,
    pub part0_mesh: i16,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchor {
    pub point: Vec2,
    pub scale: f32,
}

pub const PULSE_INTERVAL_MS: u32 = 100;

pub const DAMAGE_SOUND_INTERVAL_MS: u32 = 400;

pub const SAND_CLASS: i32 = 0x44;

pub const SAND_ACTIONS: [usize; 2] = [5, 8];

pub const SAND_FRAMES: RangeInclusive<u32> = 8..=10;

pub const SAND_TEXTURE: usize = 0xC1;

pub const SAND_LIFETIME_MS: u32 = 700;

pub const SAND_SCALE: Vec3 = Vec3::new(0.5, 1.0, 0.5);

pub const SAND_GROWTH: f32 = f32::from_bits(0x3BA3_D70A);

pub const SAND_FLIPBOOK_FRAMES: u32 = 1;

pub const SAND_FRAME_MS: u32 = 0x50;

pub const SAND_COLUMNS: RangeInclusive<i32> = -3..=2;

pub fn build_sand_dust(
    class: i32,
    action: usize,
    frame: u32,
    position: Vec3,
) -> Vec<super::effects::Billboard> {
    if class != SAND_CLASS || !SAND_ACTIONS.contains(&action) || !SAND_FRAMES.contains(&frame) {
        return Vec::new();
    }

    let mut out = Vec::with_capacity(12);

    for column in SAND_COLUMNS {
        let column = column as f32;

        let mut positive = super::effects::Billboard::new(
            SAND_TEXTURE,
            SAND_LIFETIME_MS,
            SAND_SCALE,
            SAND_GROWTH,
            SAND_FLIPBOOK_FRAMES,
            SAND_FRAME_MS,
        );

        positive.lift = column as i32 % 2 == 1;

        positive.position = Vec3::new(
            column * 0.5 + position.x,
            position.y,
            column * 0.02 + position.z,
        );

        out.push(positive);

        let mut negative = super::effects::Billboard::new(
            SAND_TEXTURE,
            SAND_LIFETIME_MS,
            SAND_SCALE,
            SAND_GROWTH,
            SAND_FLIPBOOK_FRAMES,
            SAND_FRAME_MS,
        );

        negative.lift = column as i32 % 2 == 1;

        negative.position = Vec3::new(
            position.x - column * 0.5,
            position.y,
            position.z - column * 0.02,
        );

        out.push(negative);
    }

    out
}

pub const DAMAGE_SOUND_DEFAULT: i64 = 0x1B;

pub const DAMAGE_SOUND_SUPPRESSED: i64 = 0x18;

pub const MAIN_PULSE_FRAMES: RangeInclusive<u32> = 8..=10;

pub const OFF_PULSE_FRAMES: RangeInclusive<u32> = 15..=16;

pub const MAIN_PULSE_EXCLUDED: RangeInclusive<i32> = 0x65..=0x67;

pub const OFF_PULSE_REFUSED_MASK: u32 = 0x80;

pub const PULSE_MESH: i32 = 0x213;

pub const PULSE_MESH_TEXTURE: i32 = 0xE5;

pub const PULSE_SHADE_GRID: i32 = 7;

pub const PULSE_SHADE_TEXTURE: i32 = 0x76;

pub const PULSE_PARTICLE_TEXTURE: i32 = 0xE7;

pub const PULSE_BLADE_TEXTURE: i32 = 0xE6;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulseSide {
    Main,
    Off,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulseInput {
    pub entity: u16,
    pub parked: bool,
    pub has_rig: bool,
    pub body_position: Vec3,
    pub yaw: f32,
    pub level: i16,
    pub class: i32,
    pub action: usize,
    pub frame: u32,
    pub main_selector: i32,
    pub off_selector: i32,
    pub main_item_level: i16,
    pub off_item_level: i16,
    pub main_effect_mask: u32,
    pub off_effect_mask: u32,
    pub local_distance: Option<i32>,
    pub is_local_player: bool,
    pub field_scene: bool,
    pub colour_kind: u8,
    pub affect_29: bool,
    pub child_id: u32,
    pub effect_mode: i32,
    pub subordinate_suppressed: bool,
    pub now_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulseMesh {
    pub mesh: i32,
    pub argb: u32,
    pub yaw: f32,
    pub mode: i32,
    pub at: Vec3,
    pub scale_xz: f32,
    pub scale_y: f32,
    pub texture: i32,
    pub lifetime_ms: u32,
    pub period_ms: u32,
    pub pulse: bool,
    pub additive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulseShade {
    pub grid: i32,
    pub texture: i32,
    pub radius: f32,
    pub at: Vec2,
    pub argb: u32,
    pub lifetime_ms: u32,
    pub additive: bool,
    pub requires_mesh: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulseParticle {
    pub at: Vec3,
    pub axis: Vec3,
    pub kind: i32,
    pub count: i32,
    pub size_step: f32,
    pub argb: u32,
    pub random_colour: bool,
    pub texture: i32,
    pub spread: f32,
    pub additive: bool,
    pub lifetime_ms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PulseBlade {
    pub texture: i32,
    pub lifetime_ms: u32,
    pub scale: Vec3,
    pub constructor_growth: f32,
    pub frame_count: i32,
    pub frame_ms: i32,
    pub at: Vec3,
    pub axis: Vec3,
    pub yaw: f32,
    pub growth: Vec3,
    pub motion: i32,
    pub argb: u32,
    pub billboard: bool,
    pub additive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum PulseStep {
    Mesh(PulseMesh),
    Shade(PulseShade),
    TouchChild { child_id: u32, now_ms: u32 },
    Particle(PulseParticle),
    Blade(PulseBlade),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PulsePlan {
    pub side: PulseSide,
    pub selector: i32,
    pub exits_function: bool,
    pub clock_write: Option<u32>,
    pub steps: Vec<PulseStep>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DamageSoundRequest {
    pub entity: u16,
    pub token: u64,
    pub slot: i64,
    pub kind: i16,
    pub previous_kind: i16,
    pub last_play_ms: u32,
    pub now_ms: u32,
}

impl DamageSoundRequest {
    pub fn should_play(self, already_playing: bool) -> bool {
        !already_playing
            || self.previous_kind != self.kind
            || self.last_play_ms.wrapping_add(DAMAGE_SOUND_INTERVAL_MS) < self.now_ms
    }
}

pub fn damage_sound_id(model_id: u32, class: i32, selector: i32) -> i64 {
    if model_id == 3 || class == 0x28 {
        return DAMAGE_SOUND_DEFAULT;
    }

    match selector {
        1 => 0x15,
        2 => 0x16,
        3 => 0x17,
        0x0B | 0x1F | 0x3D => 0x19,
        0x0C | 0x20 | 0x3E => 0x1A,
        0x15 | 0x16 | 0x17 | 0x29 => 0x1C,
        0x65..=0x68 => DAMAGE_SOUND_SUPPRESSED,
        _ => DAMAGE_SOUND_DEFAULT,
    }
}

fn corrected_selector(selector: i32, item_level: i16) -> i32 {
    if selector == 1 && item_level > 0x5A {
        2
    } else {
        selector
    }
}

fn pulse_due(last_ms: u32, now_ms: u32) -> bool {
    last_ms != 0 && last_ms.wrapping_add(PULSE_INTERVAL_MS) < now_ms
}

fn pulse_colours(kind: u8) -> (u32, u32) {
    match kind {
        8 => (0x80FF_CCCC, 0x0088_3333),
        7 => (0x80FF_CCFF, 0x0088_4388),
        6 => (0x80CC_FFCC, 0x0033_8843),
        5 => (0x80CC_CCFF, 0x0022_2288),
        _ => (0x80FF_FFFF, 0x0033_4388),
    }
}

fn multiply_wide(value: f32, factor: f64) -> f32 {
    (f64::from(value) * factor) as f32
}

fn multiply_add_wide(base: f32, value: f32, factor: f64) -> f32 {
    (f64::from(base) + f64::from(value) * factor) as f32
}

fn pulse_geometry(anchor: Anchor, input: PulseInput) -> (Vec3, Vec3) {
    let mut level = f32::from(input.level) / 300.0;

    if level > 1.0 {
        level = 1.0;
    }

    let y = (f64::from(input.body_position.y) + 0.9 + f64::from(level) * 0.3) as f32;

    let at = Vec3::new(anchor.point.x, y, anchor.point.y);

    let delta = Vec3::new(
        anchor.point.x - input.body_position.x,
        0.0,
        anchor.point.y - input.body_position.z,
    );

    let direction = if delta.length_squared() == 0.0 {
        Vec3::ZERO
    } else {
        delta.normalize()
    };

    (at, direction)
}

fn pulse_mesh(
    at: Vec3,
    input: PulseInput,
    argb: u32,
    scale: f32,
    lifetime_ms: u32,
    period_ms: u32,
) -> PulseMesh {
    PulseMesh {
        mesh: PULSE_MESH,
        argb,
        yaw: input.yaw,
        mode: 4,
        at,
        scale_xz: scale,
        scale_y: scale,
        texture: PULSE_MESH_TEXTURE,
        lifetime_ms,
        period_ms,
        pulse: false,
        additive: true,
    }
}

fn pulse_shade(
    at: Vec3,
    direction: Vec3,
    argb: u32,
    lifetime_ms: u32,
    requires_mesh: bool,
) -> PulseShade {
    PulseShade {
        grid: PULSE_SHADE_GRID,
        texture: PULSE_SHADE_TEXTURE,
        radius: 1.0,
        at: Vec2::new(
            multiply_add_wide(at.x, direction.x, 0.5),
            multiply_add_wide(at.z, direction.z, 0.5),
        ),
        argb,
        lifetime_ms,
        additive: true,
        requires_mesh,
    }
}

fn pulse_particle(
    at: Vec3,
    direction: Vec3,
    count: i32,
    size_step: f32,
    argb: u32,
    lifetime_ms: u32,
) -> PulseParticle {
    PulseParticle {
        at: Vec3::new(
            multiply_add_wide(at.x, direction.x, 0.3),
            multiply_add_wide(at.y, direction.y, 0.3),
            multiply_add_wide(at.z, direction.z, 0.3),
        ),
        axis: direction,
        kind: 5,
        count,
        size_step,
        argb,
        random_colour: false,
        texture: PULSE_PARTICLE_TEXTURE,
        spread: 1.0,
        additive: true,
        lifetime_ms,
    }
}

fn pulse_blade(
    at: Vec3,
    direction: Vec3,
    input: PulseInput,
    scale: Vec3,
    positive: bool,
    motion: i32,
    growth: f32,
    argb: u32,
) -> PulseBlade {
    let side = if positive { 0.05_f64 } else { -0.05_f64 };

    PulseBlade {
        texture: PULSE_BLADE_TEXTURE,
        lifetime_ms: 300,
        scale,
        constructor_growth: f32::from_bits(0x3A03_126F),
        frame_count: 1,
        frame_ms: 0x50,
        at: Vec3::new(
            multiply_add_wide(at.x, direction.x, 0.5),
            multiply_add_wide(at.y, direction.y, 0.5),
            multiply_add_wide(at.z, direction.z, 0.5),
        ),
        axis: direction,
        yaw: (f64::from(input.yaw) + std::f64::consts::FRAC_PI_2 + side) as f32,
        growth: Vec3::splat(growth),
        motion,
        argb,
        billboard: false,
        additive: true,
    }
}

fn main_pulse(anchor: Anchor, input: PulseInput) -> Vec<PulseStep> {
    let (translucent, opaque) = pulse_colours(input.colour_kind);

    let (at, direction) = pulse_geometry(anchor, input);

    let remote_large = anchor.scale > 1.5 && input.field_scene && !input.is_local_player;

    let trail_scale = multiply_wide(anchor.scale, if remote_large { 1.8 } else { 2.0 });

    let mut steps = vec![PulseStep::Mesh(pulse_mesh(
        at,
        input,
        translucent,
        trail_scale,
        200,
        200,
    ))];

    let special_large = anchor.scale > 1.5 && input.class == SPECIAL_TARGET_CLASS;

    if !special_large {
        let shade_argb = if input.effect_mode == 0 {
            opaque
        } else {
            0x0088_3333
        };

        steps.push(PulseStep::Shade(pulse_shade(
            at, direction, shade_argb, 250, false,
        )));
    }

    if input.subordinate_suppressed {
        return steps;
    }

    steps.push(PulseStep::TouchChild {
        child_id: input.child_id,
        now_ms: input.now_ms,
    });

    if input.effect_mode == 0 {
        return steps;
    }

    if !special_large {
        let (count, size_step) = if input.affect_29 {
            (20, 1.0)
        } else {
            (10, 0.8)
        };

        steps.push(PulseStep::Particle(pulse_particle(
            at,
            direction,
            count,
            size_step,
            0x0088_3333,
            300,
        )));
    }

    let blade_scale = multiply_wide(anchor.scale, if anchor.scale <= 1.5 { 4.0 } else { 1.3 });

    steps.push(PulseStep::Blade(pulse_blade(
        at,
        direction,
        input,
        Vec3::splat(blade_scale),
        true,
        0x0F,
        f32::from_bits(0x3B44_9BA6),
        0x0088_3333,
    )));

    steps.push(PulseStep::Blade(pulse_blade(
        at,
        direction,
        input,
        Vec3::splat(blade_scale),
        false,
        0x0F,
        f32::from_bits(0x3BA3_D70A),
        0x0088_3333,
    )));

    steps
}

fn off_pulse(anchor: Anchor, input: PulseInput) -> Vec<PulseStep> {
    let (translucent, opaque) = pulse_colours(input.colour_kind);

    let (at, direction) = pulse_geometry(anchor, input);

    let mut steps = vec![
        PulseStep::Mesh(pulse_mesh(
            at,
            input,
            translucent,
            multiply_wide(anchor.scale, 1.3),
            150,
            500,
        )),
        PulseStep::Shade(pulse_shade(
            at,
            direction,
            if input.effect_mode == 0 {
                opaque
            } else {
                0x0088_3333
            },
            500,
            true,
        )),
    ];

    if input.subordinate_suppressed {
        return steps;
    }

    let (count, size_step, particle_argb, particle_lifetime) = if input.effect_mode == 0 {
        (5, 0.1, 0x00FF_EEAA, 800)
    } else {
        (10, 1.3, 0x0088_3333, 300)
    };

    steps.push(PulseStep::Particle(pulse_particle(
        at,
        direction,
        count,
        size_step,
        particle_argb,
        particle_lifetime,
    )));

    let blade_argb = if input.effect_mode == 0 {
        opaque
    } else {
        0x0088_3333
    };

    let blade_scale = Vec3::new(
        multiply_wide(anchor.scale, 4.0),
        multiply_wide(anchor.scale, 4.0),
        multiply_wide(anchor.scale, 4.5),
    );

    for (positive, motion) in [(true, 0x0E), (false, 0x0F)] {
        steps.push(PulseStep::Blade(pulse_blade(
            at,
            direction,
            input,
            blade_scale,
            positive,
            motion,
            f32::from_bits(0x3BA3_D70A),
            blade_argb,
        )));
    }

    steps
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Caster {
    pub entity: u16,
    pub model: u32,
    pub mounted: bool,
    pub class: i32,
    pub action: usize,
    pub is_local_player: bool,
}

impl Caster {
    pub fn of(actor: &Actor, is_local_player: bool) -> Caster {
        Caster {
            entity: actor.entity,
            model: actor.model_id,
            mounted: actor.mounted,
            class: actor.class,
            action: actor.action,
            is_local_player,
        }
    }
}

pub fn clips(skill: &Skill, model: u32, mounted: bool) -> [i32; SLOTS] {
    let window = skill.anim_window(model, mounted);

    [
        i32::from(window[0]) - 1,
        i32::from(window[1]) - 1,
        i32::from(window[2]) - 1,
        i32::from(window[3]) - 1,
    ]
}

pub fn combo_count(clips: &[i32; SLOTS]) -> i32 {
    clips.iter().filter(|&&clip| clip > 0).count() as i32
}

pub fn duration_scale(action: usize, count: i32) -> f32 {
    match SCALED_ACTIONS.contains(&action) && count >= COMBO_THRESHOLD {
        true => 1.0 / COMBO_DIVISOR,
        false => 1.0,
    }
}

pub fn scaled_duration(action: usize, count: i32, sub_frame_ms: u32) -> u32 {
    (sub_frame_ms as f32 * duration_scale(action, count)).trunc() as u32
}

pub fn admits_targeted(row: u16) -> Option<usize> {
    let low = row <= CHAIN_LOW_MAX;

    let high = row.wrapping_sub(200) <= 0x2F;

    (low || high).then(|| usize::from(row))
}

pub fn admits_ground(row: i8) -> Option<usize> {
    (row >= 0).then_some(row as usize)
}

#[derive(Debug, Clone, PartialEq)]
pub enum Cast {
    Fallback(Option<usize>),
    Silent(&'static str),
    Chain(Chain),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Chain {
    pub row: usize,
    pub clips: [i32; SLOTS],
    pub count: i32,
    pub stand02_first: bool,
    pub play: Option<usize>,
}

impl Chain {
    pub fn duration_scale(&self) -> f32 {
        self.play
            .map_or(1.0, |action| duration_scale(action, self.count))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Advance {
    Inactive,
    Play(usize),
    Stand02,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Sequence {
    cursor: usize,
    clips: [i32; SLOTS],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct DamageSoundState {
    last_kind: i16,
    last_play_ms: u32,
    pending: Option<u64>,
}

impl Default for DamageSoundState {
    fn default() -> Self {
        Self {
            last_kind: -1,
            last_play_ms: 0,
            pending: None,
        }
    }
}

#[derive(Debug, Default)]
pub struct State {
    sequences: FxHashMap<u16, Sequence>,
    anchors: FxHashMap<u16, Anchor>,
    pulse_ms: FxHashMap<u16, u32>,
    damage_sound: FxHashMap<u16, DamageSoundState>,
    next_damage_sound_token: u64,
    damage_sound_requests: Vec<DamageSoundRequest>,
}

impl State {
    pub fn forget(&mut self, entity: u16) {
        self.sequences.remove(&entity);

        self.anchors.remove(&entity);

        self.pulse_ms.remove(&entity);

        self.damage_sound.remove(&entity);

        self.damage_sound_requests
            .retain(|request| request.entity != entity);
    }

    pub fn active_slot(&self, entity: u16) -> Option<usize> {
        self.sequences.get(&entity).map(|sequence| sequence.cursor)
    }

    pub fn anchor(&self, entity: u16) -> Option<Anchor> {
        self.anchors.get(&entity).copied()
    }

    pub fn pulse_clock(&self, entity: u16) -> u32 {
        self.pulse_ms.get(&entity).copied().unwrap_or(0)
    }

    pub fn queue_damage_sound(
        &mut self,
        entity: u16,
        model_id: u32,
        class: i32,
        plan: &PulsePlan,
        now_ms: u32,
    ) {
        let kind = match plan.side {
            PulseSide::Main => 0,
            PulseSide::Off => 1,
        };

        let state = self.damage_sound.entry(entity).or_default();

        let previous_kind = state.last_kind;

        state.last_kind = kind;

        let slot = damage_sound_id(model_id, class, plan.selector);

        self.next_damage_sound_token = self.next_damage_sound_token.wrapping_add(1);

        if self.next_damage_sound_token == 0 {
            self.next_damage_sound_token = 1;
        }

        let token = self.next_damage_sound_token;

        state.pending = Some(token);

        self.damage_sound_requests.push(DamageSoundRequest {
            entity,
            token,
            slot,
            kind,
            previous_kind,
            last_play_ms: state.last_play_ms,
            now_ms,
        });
    }

    pub fn take_damage_sounds(&mut self) -> Vec<DamageSoundRequest> {
        std::mem::take(&mut self.damage_sound_requests)
    }

    pub fn finish_damage_sound(&mut self, request: DamageSoundRequest, played: bool) -> bool {
        let Some(state) = self.damage_sound.get_mut(&request.entity) else {
            return false;
        };

        if state.pending != Some(request.token) {
            return false;
        }

        state.pending = None;

        if played {
            state.last_play_ms = request.now_ms;
        }

        true
    }

    pub fn damage_sound_state(&self, entity: u16) -> (i16, u32) {
        self.damage_sound
            .get(&entity)
            .map_or((-1, 0), |state| (state.last_kind, state.last_play_ms))
    }

    pub fn pulse(&mut self, input: PulseInput) -> Option<PulsePlan> {
        if input.parked || !input.has_rig {
            return None;
        }

        let anchor = self.anchor(input.entity);

        if !(4..=9).contains(&input.action) || (input.class != 2 && input.action >= 7) {
            return None;
        }

        if MAIN_PULSE_FRAMES.contains(&input.frame) {
            let selector = corrected_selector(input.main_selector, input.main_item_level);

            if input.local_distance.is_some_and(|distance| distance > 20) {
                return Some(PulsePlan {
                    side: PulseSide::Main,
                    selector,
                    exits_function: true,
                    clock_write: None,
                    steps: Vec::new(),
                });
            }

            if MAIN_PULSE_EXCLUDED.contains(&selector) {
                return Some(PulsePlan {
                    side: PulseSide::Main,
                    selector,
                    exits_function: false,
                    clock_write: None,
                    steps: Vec::new(),
                });
            }

            let due = pulse_due(self.pulse_clock(input.entity), input.now_ms);

            self.pulse_ms.insert(input.entity, input.now_ms);

            return Some(PulsePlan {
                side: PulseSide::Main,
                selector,
                exits_function: false,
                clock_write: Some(input.now_ms),
                steps: anchor
                    .filter(|_| due)
                    .map(|anchor| main_pulse(anchor, input))
                    .unwrap_or_default(),
            });
        }

        if OFF_PULSE_FRAMES.contains(&input.frame) {
            let masks_admit = if input.off_effect_mask == 0 {
                input.main_effect_mask != 0
            } else {
                input.off_effect_mask != OFF_PULSE_REFUSED_MASK
            };

            if !masks_admit {
                return None;
            }

            let selector = corrected_selector(input.off_selector, input.off_item_level);

            let due = pulse_due(self.pulse_clock(input.entity), input.now_ms);

            self.pulse_ms.insert(input.entity, input.now_ms);

            return Some(PulsePlan {
                side: PulseSide::Off,
                selector,
                exits_function: false,
                clock_write: Some(input.now_ms),
                steps: anchor
                    .filter(|_| due)
                    .map(|anchor| off_pulse(anchor, input))
                    .unwrap_or_default(),
            });
        }

        None
    }

    pub fn retain_targeted_anchor(
        &mut self,
        entity: u16,
        parked: bool,
        caster_position: Vec2,
        target: AnchorTarget,
        row: u16,
    ) -> Option<Anchor> {
        if parked {
            return self.anchor(entity);
        }

        let &(radius, height) = super::pick::PICK_VOLUME
            .get(target.model as usize)
            .expect("a live target's BoneAni model indexes PICK_VOLUME");

        let unbounded = ((f64::from(radius).powi(2) + f64::from(height).powi(2)).sqrt()
            * f64::from(target.scale)
            * TARGET_SCALE_FACTOR) as f32;

        let scale = if unbounded > TARGET_SCALE_MAX {
            TARGET_SCALE_MAX
        } else {
            unbounded
        };

        let point = if admits_targeted(row).is_some() {
            Vec2::ZERO
        } else if target.class == SPECIAL_TARGET_CLASS
            && target.part0_mesh == SPECIAL_TARGET_PART0_MESH
        {
            Vec2::new(
                target.position.x * SPECIAL_TARGET_WEIGHT
                    + caster_position.x * SPECIAL_CASTER_WEIGHT,
                target.position.y * SPECIAL_TARGET_WEIGHT
                    + caster_position.y * SPECIAL_CASTER_WEIGHT,
            )
        } else {
            Vec2::new(
                target.position.x * TARGET_WEIGHT + caster_position.x * CASTER_WEIGHT,
                target.position.y * TARGET_WEIGHT + caster_position.y * CASTER_WEIGHT,
            )
        };

        let anchor = Anchor { point, scale };

        self.anchors.insert(entity, anchor);

        Some(anchor)
    }

    pub fn retain_ground_anchor(
        &mut self,
        entity: u16,
        parked: bool,
        caster_position: Vec2,
        ground: Vec2,
        row: i8,
    ) -> Option<Anchor> {
        if parked {
            return self.anchor(entity);
        }

        let point = match admits_ground(row) {
            Some(_) => Vec2::ZERO,
            None => Vec2::new(
                caster_position.x * CASTER_WEIGHT + ground.x * TARGET_WEIGHT,
                caster_position.y * CASTER_WEIGHT + ground.y * TARGET_WEIGHT,
            ),
        };

        let anchor = Anchor {
            point,
            scale: GROUND_SCALE,
        };

        self.anchors.insert(entity, anchor);

        Some(anchor)
    }

    pub fn clip_completed(&mut self, entity: u16) -> Advance {
        let Some(sequence) = self.sequences.get_mut(&entity) else {
            return Advance::Inactive;
        };

        let next = sequence.cursor + 1;

        if sequence.cursor > 2 || sequence.clips[next] == NO_CLIP {
            self.sequences.remove(&entity);

            return Advance::Stand02;
        }

        sequence.cursor = next;

        Advance::Play(sequence.clips[next] as usize)
    }

    pub fn targeted(
        &mut self,
        caster: Caster,
        fallback: usize,
        row: u16,
        table: Option<&SkillData>,
    ) -> Cast {
        let Some(row) = admits_targeted(row) else {
            return self.fallback(caster, fallback, true);
        };

        if SELF_ONLY.contains(&row) && !caster.is_local_player {
            return Cast::Silent(crate::internal_reason!(
                "NWYD-R0201",
                "rows 0x38..=0x3F animate on the local player only"
            ));
        }

        self.build(caster, row, true, table)
    }

    pub fn ground(
        &mut self,
        caster: Caster,
        fallback: usize,
        row: i8,
        table: Option<&SkillData>,
    ) -> Cast {
        let Some(row) = admits_ground(row) else {
            return self.fallback(caster, fallback, false);
        };

        self.build(caster, row, false, table)
    }

    fn build(
        &mut self,
        caster: Caster,
        row: usize,
        stand02_first: bool,
        table: Option<&SkillData>,
    ) -> Cast {
        let skill = table.and_then(|t| t.get(row)).copied().unwrap_or_default();

        let clips = clips(&skill, caster.model, caster.mounted);

        let count = 0;

        let played = clips[PLAYED_SLOT];

        let play = (played != NO_CLIP).then_some(played as usize);

        let mut queued = clips;

        queued[SLOTS - 1] = NO_CLIP;

        self.sequences.insert(
            caster.entity,
            Sequence {
                cursor: PLAYED_SLOT,
                clips: queued,
            },
        );

        Cast::Chain(Chain {
            row,
            clips,
            count,
            stand02_first,
            play,
        })
    }

    fn fallback(&mut self, caster: Caster, fallback: usize, targeted: bool) -> Cast {
        if fallback == caster.action {
            return Cast::Fallback(None);
        }

        let refused = targeted
            && !caster.is_local_player
            && caster.class == NO_INTERRUPT_CLASS
            && NO_INTERRUPT_ACTIONS.contains(&caster.action);

        match refused {
            true => Cast::Fallback(None),
            false => Cast::Fallback(Some(fallback)),
        }
    }
}

pub fn describe(cast: &Cast) -> Option<String> {
    match cast {
        Cast::Fallback(None) => None,
        Cast::Fallback(Some(clip)) => Some(format!("NWYD: cast falls back to clip {clip}")),
        Cast::Silent(why) => Some(format!("NWYD: cast plays no clip — {why}")),
        Cast::Chain(chain) => Some(format!(
            "NWYD: skill {} starts clip {:?} and queues {:?} (STAND02 first: {})",
            chain.row, chain.play, chain.clips, chain.stand02_first,
        )),
    }
}
