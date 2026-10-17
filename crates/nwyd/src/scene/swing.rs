use super::aura;
use super::rng::Lcg;
use formats::audio::ani_sound::AniSound;
use formats::data::item_list::{self as item_list, Entry, ItemList};
use formats::mesh::appearance::{self, Hand, HandGrip, Look};
use formats::net::affect::Flags;
use formats::scene::demo;
use glam::Vec3;
use render::camera::Matrix4;
use std::ops::RangeInclusive;

pub const OBJECT_BYTES: usize = 0x2930;

pub const TRAIL_TEXTURE: u32 = 0xDD;

pub const TRAIL_PRIMITIVES: usize = 30;

pub const TRAIL_VERTICES: usize = TRAIL_PRIMITIVES + 2;

pub const SWING_MS: u32 = 0x5DC;

pub const SWING_TIME_BASE_BUILT: u32 = 0x28;

pub const SWING_TIME_BASE_DEFAULT: u32 = 0x1E;

pub const TRAIL_FRAMES: RangeInclusive<u32> = 4..=23;

pub const DUAL_WIELD_CLASSES: [i32; 3] = [0x1A, 0x21, 0x28];

pub const DUAL_WIELD_SIZE: f32 = 0.03;

pub fn trail_radius(weapon_max_z: f32) -> f32 {
    weapon_max_z - 0.1
}

pub fn colour(rank: u8, part_colour: u8) -> u8 {
    (rank << 4).wrapping_add(part_colour)
}

pub const TRAIL_COLOUR_WINDOW: RangeInclusive<u8> = 0x50..=0x90;

pub const TRAIL_COLOURS: [u32; 4] = [0x0030_30FF, 0x0000_B6B6, 0x00B6_00B6, 0x00FF_1010];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Silence {
    NotSwinging,
    OwnerHidden,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Swing {
    pub record: usize,
    pub hand: Hand,
    pub bone: i32,
    pub item: i32,
    pub grip: HandGrip,
    pub colour: u8,
    pub weapon_class: i32,
    pub texture: u32,
    pub suppressed: bool,
    pub geometry: bool,
    pub size: f32,
    pub blade: f32,
    pub owner_visible: bool,
    pub radius: f32,
    pub swing_started_ms: u32,
    pub special: u8,
    pub emitters: aura::Emitters,
    pub sanc_gate: bool,
    pub clock: aura::Clock,
    pub spark_stamp_ms: u32,
    pub world: Matrix4,
    pub position: Vec3,
    pub history: Option<Box<TrailHistory>>,
}

impl Swing {
    pub fn new(
        record: usize,
        hand: Hand,
        bone: i32,
        item: i32,
        weapon: Option<&Entry>,
        look: &Look,
    ) -> Swing {
        let rank = weapon.map_or(0, appearance::item_rank);

        let dual_wield = DUAL_WIELD_CLASSES.contains(&look.class);

        let other = look.parts[match hand {
            Hand::Off => Hand::Main.part(),
            Hand::Main => Hand::Off.part(),
        }]
        .mesh;

        Swing {
            record,
            hand,
            bone,
            item,
            grip: appearance::hand_grip(look.model.unwrap_or(0), hand, false, other),
            colour: colour(rank, look.part_color[hand.part()]),
            weapon_class: weapon.map_or(0, |w| w.effect_sum(appearance::WEAPON_CLASS_EFFECT)),
            texture: TRAIL_TEXTURE,
            suppressed: false,
            geometry: false,
            size: if dual_wield { DUAL_WIELD_SIZE } else { 0.0 },
            blade: if dual_wield { DUAL_WIELD_SIZE } else { 0.0 },
            radius: match dual_wield {
                true => DUAL_WIELD_SIZE,
                false => trail_radius(0.0),
            },
            owner_visible: true,
            sanc_gate: item != 0
                && !(hand == Hand::Off
                    && weapon.is_some_and(|w| w.equip_mask == item_list::equip::SHIELD)),
            swing_started_ms: 0,
            special: 0,
            emitters: aura::Emitters::default(),
            clock: aura::Clock::default(),
            spark_stamp_ms: 0,
            world: Matrix4::IDENTITY,
            position: Vec3::ZERO,
            history: None,
        }
    }

    pub fn apply_affects(&mut self, flags: &Flags) {
        self.emitters
            .take_affects(&aura::Emitters::from_flags(flags.raw(), Flags::BASE));

        self.emitters.sanc &= self.sanc_gate;
    }

    pub fn blade(&self) -> aura::Blade {
        aura::Blade {
            world: self.world,
            size: self.size,
            radius: self.radius,
        }
    }

    pub fn emit(
        &mut self,
        detail: aura::Detail,
        old_skill_vfx: bool,
        now_ms: u32,
        rng: &mut Lcg,
        out: &mut aura::Sink<'_>,
    ) {
        let blade = self.blade();

        let mut emitters = self.emitters;

        if !old_skill_vfx {
            emitters.aura_mode = 0;

            emitters.spark = false;

            emitters.weapon_effect = 0;

            emitters.panel = false;
        }

        aura::frame(&emitters, detail, &mut self.clock, blade, now_ms, rng, out);

        self.end_swing(now_ms);
    }

    pub fn end_swing(&mut self, now_ms: u32) -> bool {
        if self.swing_started_ms == 0 {
            if self.emitters.aura_mode == aura::AURA_MODE_SWING {
                self.emitters.aura_mode = 0;
            }

            return false;
        }

        if self.suppressed {
            return false;
        }

        if now_ms.wrapping_sub(self.swing_started_ms) <= SWING_MS {
            return false;
        }

        self.swing_started_ms = 0;

        if self.emitters.aura_mode == aura::AURA_MODE_SWING {
            self.emitters.aura_mode = 0;
        }

        self.special = 0;

        true
    }

    pub fn follow(&mut self, world: Matrix4) {
        self.position = world.transform_coord(Vec3::ZERO);

        self.world = world;
    }

    pub fn silence(&self) -> Option<Silence> {
        if !self.owner_visible {
            return Some(Silence::OwnerHidden);
        }

        if self.swing_started_ms == 0 && self.special == 0 {
            return Some(Silence::NotSwinging);
        }

        None
    }

    pub fn drawn(&self) -> bool {
        self.silence().is_none()
    }

    pub fn special_meshes(
        &self,
        now: u32,
        detail: aura::Detail,
    ) -> Vec<super::effect_tree::AuthoredMesh> {
        use super::effect_tree::{AuthoredMesh, MeshMaterial};

        if !self.drawn() || !detail.effects || self.special == 0 {
            return Vec::new();
        }

        let life = if self.special == 2 { 5000 } else { 3000 };

        let elapsed = now.wrapping_sub(self.swing_started_ms);

        if elapsed >= life {
            return Vec::new();
        }

        let fade = 1.0 - elapsed as f32 / life as f32;

        let mut out = Vec::with_capacity(3);

        for (mesh, width, rgb) in [
            (10, 0.2, [51, 34, 68]),
            (19, 0.3, [51, 51, 136]),
            (20, 0.3, [0, 51, 136]),
        ] {
            let (width, length) = match self.special {
                2 => (width * 0.4, self.size * 1.4),
                3 => (width, self.size * 8.5),
                4 => (width, self.size * 1.5),
                5 => (width, 0.5),
                _ => (width * 0.4, self.size * 1.5),
            };

            let rotation = match self.special {
                2 | 3 | 5 => Matrix4::rotation_y(std::f32::consts::PI * 1.5),
                4 => Matrix4::rotation_y((now.wrapping_mul(5) % 6200) as f32 / -100.0),
                _ => Matrix4::IDENTITY,
            };

            let world = Matrix4::multiply(&Matrix4::scaling(width, width, length), &rotation);

            let world = Matrix4::multiply(&world, &self.world);

            let tint = [
                rgb[0] as f32 * fade / 255.0,
                rgb[1] as f32 * fade / 255.0,
                rgb[2] as f32 * fade / 255.0,
                1.0,
            ];

            out.push(AuthoredMesh {
                mesh_id: mesh,
                world,
                texture: None,
                material: MeshMaterial::Swing {
                    tint,
                    texture: (mesh == 20).then_some(201),
                },
            });
        }

        out
    }

    pub fn trail_colour(&self) -> Option<u32> {
        if !TRAIL_COLOUR_WINDOW.contains(&self.colour) {
            return None;
        }

        let nibble = u32::from(self.colour & 0x0F);

        if nibble == 0 {
            return Some(0);
        }

        let base = TRAIL_COLOURS[usize::from(self.colour >> 4) - 5];

        let scale = |shift: u32| ((base >> shift) & 0xFF) * nibble / 0x11;

        Some(scale(16) << 16 | scale(8) << 8 | scale(0))
    }

    pub fn emit_trail_spark(
        &mut self,
        now: u32,
        time_base: u32,
        detail: aura::Detail,
        owner: super::effect_context::ActorKey,
        bone: Option<Matrix4>,
        seed: u32,
    ) -> Option<super::spark::BoneSpark> {
        if !self.owner_visible
            || !detail.effects
            || self.swing_started_ms == 0
            || self.suppressed
            || now.wrapping_sub(self.swing_started_ms) > 1500
            || now.wrapping_sub(self.spark_stamp_ms) <= 100
        {
            return None;
        }

        let base = if time_base == 0 { 30 } else { time_base };

        let frame = (now.wrapping_sub(self.swing_started_ms) as f32 * (30.0 / base as f32))
            .round_ties_even() as u32
            / 100;

        if !(4..24).contains(&frame) {
            return None;
        }

        let colour = self.trail_colour()?;

        let bone = bone?;

        self.spark_stamp_ms = now;

        let mut spark = super::spark::BoneSpark::new(
            self.bone,
            if matches!(self.special, 2 | 4) {
                self.size
            } else {
                0.0
            },
            self.size,
            colour,
            colour,
            0.3,
            now,
            seed,
        )
        .with_jitter(0.13);

        spark.owner_key = Some(owner);

        spark.set_bone(Some(bone));

        Some(spark)
    }

    pub fn sanc_burst(&self) -> Option<SancBurst> {
        self.emitters.sanc.then_some(SANC_BURST)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SancBurst {
    pub billboards: usize,
    pub kind: usize,
    pub lifetime_ms: u32,
    pub rate: f32,
    pub additive: bool,
    pub argb: u32,
    pub cooldown_ms: u32,
}

pub const HISTORY_FRAMES: usize = 0x30;

pub const TRAIL_FRAME_LIMIT: u32 = 0x2F;

pub const TRAIL_SAMPLES: usize = TRAIL_VERTICES / 2;

pub const TRAIL_SAMPLE_LEAD: i32 = 0xFA;

pub const TRAIL_SAMPLE_STEP: i32 = 8;

pub const TRAIL_UNITS_PER_FRAME: i32 = 100;

pub const TRAIL_DRAW_FIRST_FRAME: i32 = 5;

pub const TRAIL_TAIL_MARGIN: i32 = 6;

pub const TRAIL_ROOT_V: f32 = 1.0;

pub const TRAIL_TIP_V: f32 = 0.02;

pub const TRAIL_GREY_TOP: f32 = 180.0;

pub const WEAPON_LENGTH_MUL: [(i32, f32); 22] = [
    (1, 1.0),
    (2, 1.0),
    (3, 1.0),
    (11, 1.0),
    (12, 1.0),
    (13, 1.0),
    (21, 1.0),
    (22, 1.0),
    (23, 1.0),
    (31, 1.0),
    (32, 1.0),
    (33, 1.3),
    (41, 1.0),
    (51, 1.0),
    (61, 1.0),
    (62, 1.0),
    (63, 1.0),
    (101, 1.0),
    (102, 1.0),
    (103, 1.0),
    (104, 1.0),
    (105, 1.0),
];

pub fn weapon_length_mul(weapon_class: i32) -> f32 {
    WEAPON_LENGTH_MUL
        .iter()
        .find(|(k, _)| *k == weapon_class)
        .map_or(0.0, |&(_, m)| m)
}

pub const MOUNTED_BLADE_SCALE: f32 = 1.5;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TrailVertex {
    pub position: Vec3,
    pub diffuse: u32,
    pub uv: [f32; 2],
}

pub const TRAIL_INDICES: [u16; TRAIL_PRIMITIVES * 3] = {
    let mut out = [0u16; TRAIL_PRIMITIVES * 3];

    let mut i = 0;

    while i < TRAIL_PRIMITIVES {
        let (a, b) = if i % 2 == 0 { (i, i + 1) } else { (i + 1, i) };

        out[i * 3] = a as u16;

        out[i * 3 + 1] = b as u16;

        out[i * 3 + 2] = (i + 2) as u16;

        i += 1;
    }

    out
};

#[derive(Debug, Clone, PartialEq)]
pub struct Ribbon {
    pub texture: u32,
    pub vertices: [TrailVertex; TRAIL_VERTICES],
}

impl Ribbon {
    pub fn billboard_vertices(&self) -> [render::billboard::BillboardVertex; TRAIL_VERTICES] {
        self.vertices.map(|v| render::billboard::BillboardVertex {
            position: [v.position.x, v.position.y, v.position.z],
            diffuse: v.diffuse,
            uv: v.uv,
        })
    }
}

pub const TRAIL_ADDITIVE: bool = true;

pub const TRAIL_CLASS: &super::effect_tree::Class = &super::effect_tree::Class {
    name: "EffectSWSwing",
    method_count: 16,
    base: super::effect_tree::Base::VisualEffect,
    pass: super::effect_tree::Pass::Effect,
};

#[derive(Debug, Clone, PartialEq)]
pub struct TrailHistory {
    pub world: [Matrix4; HISTORY_FRAMES],
    pub bone: [Matrix4; HISTORY_FRAMES],
}

impl Default for TrailHistory {
    fn default() -> TrailHistory {
        TrailHistory {
            world: [Matrix4::IDENTITY; HISTORY_FRAMES],
            bone: [Matrix4::IDENTITY; HISTORY_FRAMES],
        }
    }
}

pub const SANC_BURST: SancBurst = SancBurst {
    billboards: 3,
    kind: 0x38,
    lifetime_ms: 500,
    rate: 0.000_5,
    additive: true,
    argb: 0xFF33_3355,
    cooldown_ms: 0x96,
};

impl Swing {
    pub fn grip_offset(&self) -> Matrix4 {
        let t = Matrix4::translation(self.grip.offset[0], self.grip.offset[1], 0.0);

        let r = Matrix4::rotation_yaw_pitch_roll(self.grip.yaw, self.grip.pitch, self.grip.roll);

        Matrix4::multiply(&t, &Matrix4::multiply(&t, &r))
    }

    pub fn place_blade(&mut self, mesh_max_z: f32) {
        if self.size == DUAL_WIELD_SIZE && self.blade == DUAL_WIELD_SIZE {
            return;
        }

        self.blade = mesh_max_z;

        self.radius = trail_radius(mesh_max_z);
    }

    pub fn start(&mut self, now_ms: u32, special: u8, mounted: bool) {
        self.size = self.blade * if mounted { MOUNTED_BLADE_SCALE } else { 1.0 };

        self.swing_started_ms = now_ms;

        self.special = special;

        if self.history.is_none() {
            self.history = Some(Box::default());
        }
    }

    pub fn adopt_trail(&mut self, pick: TrailPick) {
        self.texture = pick.texture;

        if let Some(suppressed) = pick.suppressed {
            self.suppressed = suppressed;
        }
    }

    pub fn trail_frame(
        &mut self,
        now_ms: u32,
        time_base: u32,
        clip_frames: i32,
        root: Matrix4,
        bone: Matrix4,
    ) -> Option<Ribbon> {
        self.geometry = false;

        if self.swing_started_ms == 0 || self.suppressed {
            return None;
        }

        let elapsed = now_ms.wrapping_sub(self.swing_started_ms);

        if elapsed > SWING_MS {
            return None;
        }

        let base = match time_base {
            0 => SWING_TIME_BASE_DEFAULT,
            n => n,
        };

        let total = (elapsed as f32 * (30.0 / base as f32))
            .round_ties_even()
            .max(0.0) as u32;

        let frame = total / 100;

        if frame >= TRAIL_FRAME_LIMIT {
            self.trail_tail(frame as i32, clip_frames);

            return None;
        }

        {
            let history = self.history.as_mut()?;

            history.world[frame as usize] = root;

            history.bone[frame as usize] = bone;
        }

        let ribbon = match TRAIL_FRAMES.contains(&frame) {
            true => self.build_ribbon(frame as i32, total as i32),
            false => None,
        };

        self.geometry = ribbon.is_some();

        self.trail_tail(frame as i32, clip_frames).and(ribbon)
    }

    fn build_ribbon(&self, frame: i32, total: i32) -> Option<Ribbon> {
        let history = self.history.as_ref()?;

        let grip = self.grip_offset();

        let world = history.world[frame as usize];

        let quats: [[f32; 4]; 4] = std::array::from_fn(|i| {
            let at = frame - 3 + i as i32;

            quaternion_from_matrix(&history.bone[at.clamp(0, HISTORY_FRAMES as i32 - 1) as usize])
        });

        let len = weapon_length_mul(self.weapon_class) * self.size;

        let mut vertices = [TrailVertex {
            position: Vec3::ZERO,
            diffuse: 0,
            uv: [0.0; 2],
        }; TRAIL_VERTICES];

        let mut matrix = Matrix4::IDENTITY;

        for sample in 0..TRAIL_SAMPLES {
            let k = (sample * 2) as i32;

            let t = total - TRAIL_SAMPLE_LEAD + k / 2 * TRAIL_SAMPLE_STEP;

            for segment in 0..3i32 {
                let edge = (frame - 2 + segment) * TRAIL_UNITS_PER_FRAME;

                if t < edge {
                    let start = edge - TRAIL_UNITS_PER_FRAME;

                    let f = (t - start) as f32 / TRAIL_UNITS_PER_FRAME as f32;

                    let q =
                        quaternion_slerp(quats[segment as usize], quats[segment as usize + 1], f);

                    matrix = Matrix4::multiply(
                        &Matrix4::multiply(&grip, &matrix_from_quaternion(q)),
                        &world,
                    );

                    matrix.0[12] = self.position.x;

                    matrix.0[13] = self.position.y;

                    matrix.0[14] = self.position.z;

                    break;
                }
            }

            let f = (k as f32 / TRAIL_PRIMITIVES as f32).min(1.0);

            let grey = (f * TRAIL_GREY_TOP).round_ties_even() as u32 & 0xFF;

            let diffuse = ((grey << 8 | grey) << 8) | grey;

            let uv_u = 1.0 - f;

            vertices[sample * 2] = TrailVertex {
                position: matrix.transform_coord(Vec3::ZERO),
                diffuse,
                uv: [uv_u, TRAIL_ROOT_V],
            };

            vertices[sample * 2 + 1] = TrailVertex {
                position: matrix.transform_coord(Vec3::new(0.0, 0.0, len)),
                diffuse,
                uv: [uv_u, TRAIL_TIP_V],
            };
        }

        Some(Ribbon {
            texture: self.texture,
            vertices,
        })
    }

    fn trail_tail(&mut self, frame: i32, clip_frames: i32) -> Option<()> {
        let tail = clip_frames - TRAIL_TAIL_MARGIN;

        if frame < TRAIL_DRAW_FIRST_FRAME || tail < frame {
            self.geometry = false;
        }

        if tail + 1 < frame {
            if let Some(history) = self.history.as_mut() {
                **history = TrailHistory::default();
            }
        }

        self.geometry.then_some(())
    }

    pub fn for_actor(
        record: usize,
        actor: &demo::Actor,
        look: &Look,
        parts: usize,
        items: &ItemList,
        has_bone: impl Fn(i32) -> bool,
    ) -> Vec<Swing> {
        Swing::for_hands(
            record,
            (actor.left_hand, actor.right_hand),
            look,
            parts,
            items,
            has_bone,
        )
    }

    pub fn for_hands(
        record: usize,
        hands: (i32, i32),
        look: &Look,
        parts: usize,
        items: &ItemList,
        has_bone: impl Fn(i32) -> bool,
    ) -> Vec<Swing> {
        Hand::BOTH
            .into_iter()
            .filter(|&hand| Swing::wanted(look, parts, hand))
            .filter_map(|hand| {
                let model = look.model?;

                let bone = appearance::hand_bone(model, hand).filter(|&b| has_bone(b))?;

                let item = match hand {
                    Hand::Off => hands.0,
                    Hand::Main => hands.1,
                };

                let weapon = usize::try_from(item).ok().and_then(|id| items.get(id));

                let mut swing = Swing::new(record, hand, bone, item, weapon.as_ref(), look);

                swing.grip = look.equipped_grip(hand, hands.1, hands.0, items);

                Some(swing)
            })
            .collect()
    }

    pub fn count_for(look: &Look, parts: usize, has_bone: impl Fn(i32) -> bool) -> usize {
        Hand::BOTH
            .into_iter()
            .filter(|&hand| Swing::wanted(look, parts, hand))
            .filter(|&hand| {
                appearance::hand_bone(look.model.unwrap_or(u32::MAX), hand).is_some_and(&has_bone)
            })
            .count()
    }

    pub fn wanted(look: &Look, parts: usize, hand: Hand) -> bool {
        let part = hand.part();

        part < parts && !(look.parts[0].mesh > 0x59 && look.parts[part].mesh == 0)
    }
}

#[derive(Debug, Clone, Copy)]
struct HandActivity {
    class: i32,
    model: u32,
    mounted: bool,
    group: Option<usize>,
    items: [i32; 2],
    weapons: [i32; 2],
    masks: [i32; 2],
}

impl HandActivity {
    fn flags(self) -> [bool; 2] {
        let Self {
            class,
            model,
            mounted,
            group,
            items,
            weapons: [off, main],
            masks: [off_mask, main_mask],
        } = self;

        let basic = matches!(main, 1 | 0x0B | 0x3D | 2 | 0x0C | 0x3E | 0x1F);

        match class {
            0x1A | 0x21 | 0x28 => return [items[0] != 0, items[1] != 0],
            0x24 | 0x25 | 0x3D => return [false, true],
            0x3C => {
                return match (main, off_mask) {
                    (0x29, 0) | (0x0B, 0x80) => [true, false],
                    _ => [true, true],
                };
            }
            0x3E => return [basic, basic],
            _ => {}
        }

        match model {
            0 | 1 => {
                if main == 0 && off_mask == 0x80 {
                    return [false, false];
                }

                if main_mask == 0xC0 && off_mask == 0xC0 {
                    return [true, true];
                }

                if main == 0x29 {
                    return [true, true];
                }

                if mounted && basic && off != 0 && off_mask != 0x80 {
                    return [false, false];
                }

                let enabled = group.is_some_and(|g| g != 0);

                [enabled && !mounted && group == Some(4), enabled]
            }
            2 => [
                false,
                main != 0x65
                    && if group == Some(0) {
                        main_mask != 0
                    } else {
                        true
                    },
            ],
            4 => [
                false,
                !matches!(main, 0x66 | 0x67)
                    && if group == Some(0) {
                        main_mask != 0
                    } else {
                        true
                    },
            ],
            10 => [items[0] != 0, items[1] != 0],
            _ => [false, false],
        }
    }
}

pub(crate) fn hand_activity(look: &Look, hands: (i32, i32), items: &ItemList) -> [bool; 2] {
    let ids = [hands.0, hands.1];

    if ids.iter().any(|&id| {
        !(0..14500).contains(&id) || items.get(id as usize).is_some_and(|item| item.mesh < 0)
    }) {
        return [false, false];
    }

    let model = look.model.unwrap_or(0);

    let mounted = look.is_mounted();

    HandActivity {
        class: look.class,
        model,
        mounted,
        group: appearance::anim_group(
            look.class,
            model,
            mounted,
            look.parts[0].mesh,
            hands.1,
            hands.0,
            items,
        ),
        items: ids,
        weapons: ids.map(|id| appearance::weapon_class(id, items)),
        masks: ids.map(|id| appearance::hand_mask(id, items)),
    }
    .flags()
}

fn matrix_from_quaternion(q: [f32; 4]) -> Matrix4 {
    let [x, y, z, w] = q;

    Matrix4([
        1.0 - 2.0 * (y * y + z * z),
        2.0 * (x * y + z * w),
        2.0 * (x * z - y * w),
        0.0,
        2.0 * (x * y - z * w),
        1.0 - 2.0 * (x * x + z * z),
        2.0 * (y * z + x * w),
        0.0,
        2.0 * (x * z + y * w),
        2.0 * (y * z - x * w),
        1.0 - 2.0 * (x * x + y * y),
        0.0,
        0.0,
        0.0,
        0.0,
        1.0,
    ])
}

fn quaternion_from_matrix(m: &Matrix4) -> [f32; 4] {
    let m = &m.0;

    let trace = m[0] + m[5] + m[10];

    if trace > 0.0 {
        let s = (trace + 1.0).sqrt();

        let inv = 0.5 / s;

        return [
            (m[6] - m[9]) * inv,
            (m[8] - m[2]) * inv,
            (m[1] - m[4]) * inv,
            s * 0.5,
        ];
    }

    let i = if m[5] > m[0] {
        if m[10] > m[5] { 2 } else { 1 }
    } else if m[10] > m[0] {
        2
    } else {
        0
    };

    let (j, k) = ((i + 1) % 3, (i + 2) % 3);

    let s = (m[i * 4 + i] - m[j * 4 + j] - m[k * 4 + k] + 1.0).sqrt();

    let inv = 0.5 / s;

    let mut q = [0.0f32; 4];

    q[i] = s * 0.5;

    q[j] = (m[i * 4 + j] + m[j * 4 + i]) * inv;

    q[k] = (m[i * 4 + k] + m[k * 4 + i]) * inv;

    q[3] = (m[j * 4 + k] - m[k * 4 + j]) * inv;

    q
}

fn quaternion_slerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    let mut dot = a[0] * b[0] + a[1] * b[1] + a[2] * b[2] + a[3] * b[3];

    let mut b = b;

    if dot < 0.0 {
        dot = -dot;

        b = [-b[0], -b[1], -b[2], -b[3]];
    }

    let (wa, wb) = if dot > 0.999_999 {
        (1.0 - t, t)
    } else {
        let theta = dot.clamp(-1.0, 1.0).acos();

        let sin = theta.sin();

        (((1.0 - t) * theta).sin() / sin, (t * theta).sin() / sin)
    };

    [
        a[0] * wa + b[0] * wb,
        a[1] * wa + b[1] * wb,
        a[2] * wa + b[2] * wb,
        a[3] * wa + b[3] * wb,
    ]
}

pub const TRAIL_BY_EFFECT: [u8; 248] = {
    let mut out = [0u8; 248];

    let head: [u8; 101] = [
        1, 5, 3, 5, 1, 5, 5, 5, 4, 0, 5, 0, 0, 5, 5, 5, 2, 2, 4, 1, 3, 2, 4, 5, 1, 5, 3, 1, 5, 5,
        5, 5, 3, 1, 1, 3, 1, 0, 0, 5, 2, 0, 5, 1, 1, 5, 1, 5, 0, 0, 1, 1, 0, 1, 5, 5, 1, 0, 3, 0,
        0, 0, 5, 5, 2, 5, 2, 5, 0, 5, 5, 5, 0, 2, 1, 0, 2, 1, 5, 5, 5, 2, 3, 2, 3, 5, 5, 5, 0, 5,
        1, 2, 5, 5, 5, 5, 1, 5, 5, 5, 5,
    ];

    let mut i = 0;

    while i < head.len() {
        out[i] = head[i];

        i += 1;
    }

    out
};

pub const TRAIL_TEXTURE_BASE: u32 = 0xD8;

pub const TRAIL_SUPPRESS_MOTION: i8 = b'K' as i8;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrailPick {
    pub texture: u32,
    pub suppressed: Option<bool>,
}

pub fn trail_pick(entity_id: u32, motion: i8, class_switch: i32, weapon_id: i16) -> TrailPick {
    if entity_id < 1000 {
        let texture = match usize::try_from(motion) {
            Ok(at) => TRAIL_TEXTURE_BASE + u32::from(TRAIL_BY_EFFECT[at]),
            Err(_) => TRAIL_TEXTURE,
        };

        return TrailPick {
            texture,
            suppressed: Some(motion == TRAIL_SUPPRESS_MOTION),
        };
    }

    let texture = match class_switch {
        2 => 0xDE,
        4 => 0xDF,
        5 | 7 => match weapon_id {
            0x11F => 0xE2,
            0x11B => 0xE3,
            0xAF => 0xE4,
            _ => 0xE0,
        },
        9 => match weapon_id {
            0xAF => 0xE4,
            _ => 0xE1,
        },
        _ => TRAIL_TEXTURE,
    };

    TrailPick {
        texture,
        suppressed: None,
    }
}

pub fn time_base(sounds: &AniSound, model: u32, job: i16, action: usize) -> u32 {
    let Some(binding) = sounds.binding(model as usize, job, action) else {
        return 0;
    };

    u32::try_from(binding.speed).unwrap_or(0)
}
