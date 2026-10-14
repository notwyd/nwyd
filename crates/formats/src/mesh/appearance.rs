use crate::data::item_effect::{Effects, effect};
use crate::data::item_list::{self, ItemList};
use crate::data::mount::MountVisual;
use crate::mesh::costume;
use crate::mesh::glow;
use crate::mesh::{Matrix4, matrix_multiply};
use crate::net::account::{self, Equipment};
use crate::scene::demo::{APPEARANCE_EFFECT, Actor};

pub fn model_for_class(class: i32) -> Option<u32> {
    let model = match class {
        0x01 => 0,
        0x02 => 1,
        0x04 => 0,
        0x08 => 1,
        0x10 => 20,
        0x11 => 21,
        0x12 => 22,
        0x13 => 23,
        0x14 => 24,
        0x15 => 2,
        0x16 => 25,
        0x17 => 26,
        0x18 => 27,
        0x19 => 2,
        0x1A => 3,
        0x1B => 28,
        0x1C => 29,
        0x1D => 6,
        0x1E => 4,
        0x1F => 32,
        0x20 => 7,
        0x21 => 8,
        0x22 => 0,
        0x23 => 29,
        0x24 => 0,
        0x25 => 1,
        0x26 => 1,
        0x27 => 0,
        0x28 => 0,
        0x29 => 69,
        0x2A => 30,
        0x2B => 31,
        0x2C => 33,
        0x2D => 23,
        0x2E => 11,
        0x2F => 35,
        0x30 => 34,
        0x31 => 36,
        0x32 => 37,
        0x33 => 38,
        0x34 => 39,
        0x35 => 40,
        0x36 => 9,
        0x37 => 10,
        0x38 => 41,
        0x39 => 12,
        0x3A => 42,
        0x3B => 43,
        0x3C => 0,
        0x3D => 1,
        0x3E => 5,
        0x3F => 0,
        0x40 => 44,
        0x42 => 45,
        0x43 => 46,
        0x44 => 47,
        0x45 => 48,
        0x46 => 53,
        0x47 => 54,
        0x48 => 55,
        0x49 => 56,
        0x4A => 57,
        _ => return None,
    };

    Some(model)
}

pub fn model_for_class_or_default(class: i32) -> u32 {
    model_for_class(class).unwrap_or(0)
}

pub fn known_classes() -> Vec<i32> {
    (0..=0x4B)
        .filter(|&c| model_for_class(c).is_some())
        .collect()
}

pub const PARTS: usize = 8;

pub const MOUNT_PARTS: usize = 3;

pub const BODY_PARTS: usize = 6;

pub const CAPE_MODEL: u32 = 0x55;

pub const CAPE_MODELS: [u32; 5] = [0, 1, 8, 3, 2];

pub const VARIANT_CLASSES: [i32; 7] = [4, 8, 0x24, 0x27, 0x28, 0x3C, 0x3F];

pub const MMULTI_CLASSES: [i32; 4] = [0x1A, 0x21, 0x28, 0x3F];

pub const MOUNT_ITEM_RANGES: [(i32, i32); 2] = [(2360, 2389), (2960, 2999)];

pub const MOUNT_APPEARANCE_SHIFT: i32 = 0x7FD;

pub const MOUNT_KIND_BASE: i32 = 0x938;

pub const MOUNT_SCALE_OVERRIDE_ITEM: i32 = 0x14D;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]

pub struct PartRef {
    pub mesh: i16,
    pub tex: i16,
}

impl PartRef {
    pub const EMPTY: PartRef = PartRef { mesh: 0, tex: 0 };

    pub fn is_empty(self) -> bool {
        self == PartRef::EMPTY
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Cape {
    pub item: i32,
    pub mesh: i16,
    pub rank: u8,
    pub refine: u8,
    pub tier: u8,
    pub side: u8,
}

impl Cape {
    pub fn part_for_body(self, body: i32) -> PartRef {
        if body != 100 {
            return self.part();
        }

        PartRef {
            mesh: 0,
            tex: match self.item {
                546 | 3198 | 1770 | 1769 => 1,
                548 | 3199 | 572..=574 => 2,
                _ => 0,
            },
        }
    }

    pub fn part(self) -> PartRef {
        PartRef {
            mesh: 0,
            tex: self.mesh,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct CapeSeat {
    pub offset: [f32; 2],
    pub scale: f32,
    pub yaw: f32,
    pub pitch: f32,
}

pub const CAPE_ANCHOR_BONE: i32 = 6;

#[derive(Debug, Clone, Copy, PartialEq)]

pub enum AnchorAdjust {
    Copy,
    RotateX(f32),
    RotateZ(f32),
    RotateXThenZ(f32, f32),
    Rows([(usize, f32); 3]),
}

impl AnchorAdjust {
    pub fn apply(self, m: &Matrix4) -> Matrix4 {
        match self {
            AnchorAdjust::Copy => *m,
            AnchorAdjust::RotateX(a) => matrix_multiply(&rotation_x(a), m),
            AnchorAdjust::RotateZ(a) => matrix_multiply(&rotation_z(a), m),
            AnchorAdjust::RotateXThenZ(a, b) => {
                matrix_multiply(&rotation_z(b), &matrix_multiply(&rotation_x(a), m))
            }
            AnchorAdjust::Rows(rows) => {
                let mut out = *m;

                for (dst, &(src, sign)) in rows.iter().enumerate() {
                    for c in 0..4 {
                        out[dst * 4 + c] = m[src * 4 + c] * sign;
                    }
                }

                out
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct AnchorRule {
    pub bones: &'static [i32],
    pub adjust: AnchorAdjust,
}

pub fn anchor_rule(model: u32, mesh: i16) -> Option<AnchorRule> {
    use AnchorAdjust::{Copy, RotateX, RotateXThenZ, RotateZ, Rows};

    let (bones, adjust): (&'static [i32], AnchorAdjust) = match model {
        0x14 => match mesh {
            7 | 0x2F | 0x30 => (&[3], Copy),
            6 => (&[7], Copy),
            _ => (&[4], Copy),
        },
        0 | 1 => (&[CAPE_ANCHOR_BONE], Copy),
        3 => (&[9], Copy),
        8 => (&[16], Copy),
        0x1D => (&[7, 5], Copy),
        2 | 4 => (&[7], Copy),
        0x19 | 0x1C => (&[3], Copy),
        0x1E | 0x1F => (&[4], Copy),
        0x26 => (&[3], Copy),
        0x29 => (&[3], RotateXThenZ(-core::f32::consts::FRAC_PI_2, 0.43633)),
        0x27 => (&[7], Copy),
        0x28 => (&[15], Copy),
        0x36 | 0x38 | 0x39 => (&[CAPE_ANCHOR_BONE], Copy),
        0x30 => (&[1], Rows([(1, 1.0), (2, -1.0), (0, -1.0)])),
        0x31 | 0x34 => (&[3], Rows([(0, -1.0), (2, -1.0), (1, -1.0)])),
        0x32 => (&[2], Rows([(0, -1.0), (2, -1.0), (1, -1.0)])),
        0x33 => (&[4], Rows([(0, 1.0), (2, 1.0), (1, -1.0)])),
        0x35 => (&[4], RotateX(core::f32::consts::FRAC_PI_2)),
        0x23 => (&[3], RotateZ(3.2288)),
        0x3B => (&[3], RotateZ(3.3510)),
        _ => return None,
    };

    Some(AnchorRule { bones, adjust })
}

pub fn cape_anchor_bone(model: u32) -> Option<&'static [i32]> {
    CAPE_MODELS
        .contains(&model)
        .then(|| anchor_rule(model, 0).map(|rule| rule.bones))
        .flatten()
}

fn rotation_x(a: f32) -> Matrix4 {
    let (s, c) = a.sin_cos();

    [
        1.0, 0.0, 0.0, 0.0, 0.0, c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

fn rotation_z(a: f32) -> Matrix4 {
    let (s, c) = a.sin_cos();

    [
        c, s, 0.0, 0.0, -s, c, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]
}

pub const CAPE_SUB_FRAME_MS: u32 = 0x28;

pub const CAPE_CLIP_STANDING: usize = 0;

pub const CAPE_CLIP_WALKING: usize = 1;

pub const CAPE_CLIP_RUNNING: usize = 2;

pub const CAPE_CLIP_MOUNTED: usize = 3;

pub fn cape_clip(mounted: bool, action: usize) -> Option<usize> {
    if mounted {
        return Some(CAPE_CLIP_MOUNTED);
    }

    match action {
        0 | 1 => Some(CAPE_CLIP_STANDING),
        2 => Some(CAPE_CLIP_WALKING),
        3 => Some(CAPE_CLIP_RUNNING),
        _ => None,
    }
}

pub const DEMO_WCLASS: i16 = 0;

pub fn cape_row(class: i32) -> usize {
    match class {
        1 => 0,
        2 | 0x26 => 1,
        4 => 2,
        8 => 3,
        0x19 => 5,
        0x1A => 6,
        0x21 => 7,
        0x28 => 8,
        _ => 0,
    }
}

pub const CAPE_BACK_OFFSETS: [f32; 160] = [
    0.09, 0.08, 0.06, 0.07, 0.08, 0.08, 0.08, 0.065, 0.065, 0.055, 0.055, 0.01, 0.08, 0.0, 0.02,
    0.01, 0.01, 0.035, 0.1, 0.03, 0.1, 0.1, 0.1, 0.1, 0.1, 0.08, 0.1, 0.1, 0.1, 0.11, 0.1, 0.08,
    0.09, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.08, 0.08, 0.06, 0.05, 0.07, 0.05, 0.08, 0.08, 0.05, 0.0,
    0.11, 0.08, 0.06, 0.055, 0.0, 0.0, 0.0, 0.0, 0.0, 0.1, 0.01, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1,
    0.08, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.08, 0.09, 0.035, 0.06,
    0.07, 0.08, 0.08, 0.08, 0.065, 0.065, 0.055, 0.055, 0.01, 0.08, 0.0, 0.02, 0.01, 0.01, 0.035,
    0.01, 0.03, 0.1, 0.09, 0.1, 0.1, 0.1, 0.08, 0.1, 0.1, 0.1, 0.11, 0.1, 0.08, 0.09, 0.1, 0.1,
    0.1, 0.1, 0.1, 0.01, 0.08, 0.08, 0.03, 0.05, 0.07, 0.05, 0.08, 0.08, 0.05, 0.0, 0.11, 0.08,
    0.06, 0.055, 0.0, 0.0, 0.0, 0.0, 0.0, 0.01, 0.01, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.08, 0.1, 0.1,
    0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.1, 0.01, 0.08,
];

pub const CAPE_TABLE_SPLIT: (usize, i16) = (80, 40);

pub fn cape_back_offset(class: i32, armour_mesh: i16) -> Option<f32> {
    let (split, first_high) = CAPE_TABLE_SPLIT;

    let row = cape_row(class) * 20;

    let index = if armour_mesh < first_high {
        row + usize::try_from(armour_mesh).ok()?
    } else {
        split + row + usize::try_from(armour_mesh - first_high).ok()?
    };

    CAPE_BACK_OFFSETS.get(index).copied()
}

fn cape_offset_override(cape_item: i32, cape_mesh: i16) -> Option<[f32; 2]> {
    let mut over = None;

    if (0xC7D..=0xC7F).contains(&(cape_item as i16 as i32)) {
        over = Some([0.05, 0.11]);
    }

    if matches!(cape_item as i16 as i32, 0x23D | 0x6E7 | 0x6EA) {
        over = Some([0.05, 0.15]);
    }

    if (0xC7..=0xCB).contains(&cape_mesh) {
        over = Some([0.0, 0.01]);
    }

    if cape_mesh == 0xDC {
        over = Some([0.0, 0.05]);
    }

    over
}

fn cape_seat_for_stance(wclass: i16) -> Option<([f32; 2], f32)> {
    match wclass {
        0x16 | 0x17 => Some(([-0.2, 0.07], 1.2)),
        0x18 => Some(([0.0, 0.0], 1.2)),
        0x19 => Some(([0.05, 0.01], 1.0)),
        0x20 => Some(([0.05, 0.07], 1.2)),
        _ => None,
    }
}

pub fn seat_pitch(mount_model: u32) -> f32 {
    match mount_model {
        0x19 => 0.1,
        0x1C | 0x1F => 0.15,
        0x14 => 0.5,
        0x1E | 0x27 => 0.25,
        0x26 => 0.26,
        0x1D | 0x28 => 0.18,
        _ => 0.0,
    }
}

pub fn seat_rotation(variant: u32, mount_model: u32) -> Option<(f32, f32)> {
    use core::f32::consts::{FRAC_PI_2, FRAC_PI_6, PI};

    let pitch = seat_pitch(mount_model);

    Some(match variant {
        1 => (-FRAC_PI_2, pitch - PI),
        2 => (-FRAC_PI_2, pitch + FRAC_PI_2),
        3 => (FRAC_PI_2, (pitch - FRAC_PI_2) - (pitch + FRAC_PI_6)),
        4 => (FRAC_PI_2, -1.3707963),
        5 => (-FRAC_PI_2, 1.9707963),
        _ => return None,
    })
}

pub const CAPE_SEAT_VARIANT: u32 = 1;

pub fn rider_seat_variant(mount_model: u32, mesh: i16) -> u32 {
    match (mount_model, mesh) {
        (0x14, 7 | 0x2F | 0x30) => 4,
        (0x14, _) => 3,
        _ => 2,
    }
}

pub fn cape_rotation(mount_model: u32) -> (f32, f32) {
    seat_rotation(CAPE_SEAT_VARIANT, mount_model).expect("variant 1 is in range")
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct RiderSeat {
    pub offset: [f32; 2],
    pub scale: f32,
    pub yaw: f32,
    pub pitch: f32,
}

pub fn rider_offsets(
    mount_model: u32,
    parts: [PartRef; MOUNT_PARTS],
    scale: f32,
    class: i32,
    body_scale: f32,
) -> ([f32; 2], f32) {
    let mesh = parts[0].mesh;

    let (mut y, mut x) = (scale * -0.37, scale * 0.2);

    let inverse = 1.0 / scale;

    let mut s = inverse;

    match mount_model {
        0x19 => (y, x) = (scale * -0.35, scale * -0.1),
        0x1C => (y, x) = (scale * -0.2, scale * 0.1),
        0x1D => match mesh {
            1 | 4 | 0x28 | 0x29 => (y, x) = (scale * -0.1, scale * -0.6),
            0 | 0x2A => (s, x) = (1.2, scale * -0.6),
            0x0B | 0x0C => (y, x) = (-0.28, x - 0.8),
            _ => x = scale * -0.6,
        },
        0x14 => match mesh {
            0x32 => (y, x) = (scale * 0.5 - 0.3, -0.5),
            7 | 0x2F | 0x30 => (y, x) = (scale * 0.5, scale * -0.1),
            6 => (y, x) = (0.0, scale * -0.8),
            _ => (y, x) = (scale * 0.25, scale * -0.1),
        },
        _ => {}
    }

    if class == 0x28 {
        if mount_model == 0x14 && mesh != 7 {
            s = body_scale / 1.3;

            y /= 1.45;
        } else {
            s = if mount_model == 0x14 {
                body_scale * 1.7
            } else {
                body_scale
            };

            if mount_model != 0x14 {
                y += 0.05;
            }
        }
    }

    let one = parts[1].mesh;

    if mount_model == 0x1D && matches!(one, 5 | 10) {
        (y, s) = (-0.2, inverse);
    } else if mount_model == 0x1D && matches!(one, 6..=8) {
        (y, s) = (if one == 6 { -0.2 } else { -0.4 }, 1.1 / scale);
    } else if (mount_model == 0x26 && (matches!(one, 1..=4) || one == 0x2D))
        || (mount_model == 0x1F && one == 0x11)
    {
        y = -0.6;
    } else {
        match mount_model {
            0x30 => (y, x) = (y - 0.8, x + 0.2),
            0x31 | 0x34 => {
                (y, x) = (y - 0.2, x - 0.2);

                if mesh == 0x18 {
                    x += 0.2;
                }
            }
            0x32 => {
                (y, x) = (y - 0.5, x - 0.2);

                if mesh == 4 {
                    (y, x) = (y - 0.2, x + 0.2);
                }

                if mesh == 5 {
                    (y, x) = (y - 0.2, x + 0.1);
                }
            }
            0x35 => (y, x, s) = (-0.28, 0.4, 1.0),
            0x33 => (y, x) = (-0.38, 0.3),
            0x23 => (y, x) = (-0.3, -0.25),
            0x29 => (y, x) = (-0.45, -0.25),
            0x22 | 0x3B => (y, x, s) = (-0.18, -0.2, 1.0),
            _ => {}
        }
    }

    ([x, y], s)
}

pub fn rider_seat(mount: Mount, class: i32, body_scale: f32) -> Option<RiderSeat> {
    let model = mount.model.unwrap_or(0);

    let (yaw, pitch) = seat_rotation(rider_seat_variant(model, mount.parts[0].mesh), model)?;

    let (offset, scale) = rider_offsets(model, mount.parts, mount.scale, class, body_scale);

    Some(RiderSeat {
        offset,
        scale,
        yaw,
        pitch,
    })
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Mount {
    pub item: i32,
    pub appearance_item: i32,
    pub kind: i16,
    pub model: Option<u32>,
    pub parts: [PartRef; MOUNT_PARTS],
    pub part_color: [u8; MOUNT_PARTS],
    pub visual_kind: u8,
    pub scale: f32,
}

pub fn mount_world_scale(class: i32, body_scale: f32, mount_scale: f32) -> f32 {
    match class {
        MOUNT_SCALE_IGNORES_RIDER => mount_scale,
        _ => body_scale * mount_scale,
    }
}

pub const MOUNT_SCALE_IGNORES_RIDER: i32 = 0x28;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct Wearer {
    pub rank_word: u16,
    pub flags: u8,
}

fn appearance_class(effects: &Effects, body_item: u16, costume_id: i32, costume_item: u16) -> i32 {
    let class_of = |index| {
        effects.total_scaled(
            &account::Item {
                index,
                ..account::Item::default()
            },
            APPEARANCE_EFFECT as u8,
        )
    };

    let body_class = class_of(body_item);

    if costume_id == 0 || costume_item == 0 {
        return body_class;
    }

    class_of(costume_item)
}

pub const FLAG_NO_COSTUME: u8 = 1;

pub const FLAG_NO_HELMET: u8 = 4;

pub const FLAG_MOUNT_AS_WORN: u8 = 2;

pub fn cape_side(cape_mesh: i16) -> u8 {
    match i32::from(cape_mesh) {
        0 | 2 | 8 | 9 | 0xB | 0x18 | 0x1B | 0x1F | 0x22 | 0xDC => 1,
        1 | 3 | 0xC | 0xD | 0xE | 0x19 | 0x1C | 0x1E | 0x20 | 0x23 | 0xDD | 0xDF => 2,
        6 | 7 | 0xF | 0x10 | 0x11 | 0x13 | 0x1A | 0x1D | 0x21 | 0x24 | 0xDE | 0xE0 => 3,
        _ => 4,
    }
}

pub fn cape_rank(word: u16) -> u8 {
    match word {
        200..=399 => 5,
        400..=599 => 6,
        600..=799 => 7,
        800..=999 => 8,
        1000..=1199 => 9,
        1200.. => 10,
        _ => 0,
    }
}

pub const MOUNT_SLOT: usize = 14;

pub const MOUNT_ITEM_TYPES: [i16; 2] = [
    item_list::item_type::PET_STAGE2,
    item_list::item_type::MOUNT,
];

pub const MOUNT_SKIN_ITEM_TYPE: i16 = 0x18;

pub const MOUNT_ID_WINDOW: core::ops::Range<i32> = 0xB90..3000;

pub const MOUNT_UNCONDITIONAL_IDS: core::ops::RangeInclusive<i32> = 0xB90..=0xBB6;

pub const MOUNT_COLOUR_CAP: i32 = 12;

pub fn mount_for_field(
    equip: &Equipment,
    effects: &Effects,
    visuals: &[MountVisual],
    flags: u8,
) -> Option<Mount> {
    mount_for_field_override(equip, effects, visuals, flags, 0)
}

fn mount_for_field_override(
    equip: &Equipment,
    effects: &Effects,
    visuals: &[MountVisual],
    flags: u8,
    arena_item: u16,
) -> Option<Mount> {
    use crate::data::item_effect::effect;

    let items = effects.items;

    let worn = equip.items[MOUNT_SLOT];

    let item = i32::from(worn.index as i16);

    let kind_of = |id: i32| -> i16 {
        usize::try_from(id)
            .ok()
            .and_then(|id| items.get(id))
            .map_or(0, |e| e.item_type)
    };

    use item_list::item_type::{MOUNT, PET_STAGE2};

    let kind = kind_of(item);

    if !MOUNT_ITEM_TYPES.contains(&kind) && !MOUNT_ID_WINDOW.contains(&item) {
        return None;
    }

    let progress = effects.total_scaled(&worn, effect::PET_PROGRESS);

    if progress < 1
        && kind != MOUNT
        && (kind == PET_STAGE2 || !MOUNT_UNCONDITIONAL_IDS.contains(&item))
    {
        return None;
    }

    let mut grown = true;

    let mut appearance_item = item;

    let over = i32::from(worn.appearance as i16);

    if over == 0 {
        grown = kind != MOUNT;
    } else if MOUNT_ITEM_TYPES.contains(&kind_of(over)) || kind_of(over) == MOUNT_SKIN_ITEM_TYPE {
        if flags & FLAG_MOUNT_AS_WORN == 0 {
            appearance_item = over;

            grown = false;
        }
    }

    if arena_item != 0 {
        appearance_item = i32::from(arena_item as i16);

        grown = false;
    }

    let mesh = usize::try_from(appearance_item)
        .ok()
        .and_then(|id| items.get(id))?
        .mesh;

    let row = crate::data::mount::visual_for_mesh(visuals, mesh)?;

    let parts = [
        PartRef {
            mesh: row.grid[0][0],
            tex: row.grid[1][0],
        },
        PartRef {
            mesh: row.grid[0][1],
            tex: row.grid[1][1],
        },
        PartRef {
            mesh: row.grid[0][2],
            tex: row.grid[1][2],
        },
    ];

    let model = row.kind as u32;

    let maturity = effects.total_scaled(&worn, effect::PET_MATURITY);

    let mut part_color = [0u8; MOUNT_PARTS];

    match grown {
        true => {
            let band = (maturity / 10).min(MOUNT_COLOUR_CAP) as u8;

            for (slot, colour) in part_color.iter_mut().enumerate() {
                if row.grid[2][slot] != -1 {
                    *colour = band;
                }
            }
        }
        false => {
            for (slot, colour) in part_color.iter_mut().enumerate() {
                if row.grid[2][slot] != -1 {
                    *colour = row.grid[2][slot] as u8;
                }
            }

            if row.grid[2][0] != -1 && part_color[0] == 0 {
                part_color = [(maturity / 10) as u8; MOUNT_PARTS];
            }
        }
    }

    Some(Mount {
        item,
        appearance_item,
        kind: 0,
        model: Some(model),
        parts,
        part_color,
        visual_kind: row.visual_kind as u8,
        scale: match row.scale == 1.0 {
            true => mount_auto_scale(model, parts[0].mesh),
            false => row.scale,
        },
    })
}

pub fn mount_for_spawn(
    equip: &[u16],
    effect: &[u8],
    refine: &[u8],
    flags: u8,
    effects: &Effects,
    visuals: &[MountVisual],
) -> Option<Mount> {
    use item_list::item_type::{MOUNT, PET_STAGE2};

    let items = effects.items;

    let slot = |table: &[u8]| table.get(MOUNT_SLOT).copied().unwrap_or(0);

    let item = i32::from(equip.get(MOUNT_SLOT).copied().unwrap_or(0) as i16);

    let entry = usize::try_from(item).ok().and_then(|id| items.get(id));

    let mut row_index = entry.as_ref().map_or(0, |e| e.mesh);

    let mut grown = true;

    let over = slot(refine);

    if over != 0 && flags & FLAG_MOUNT_AS_WORN == 0 {
        row_index = i16::from(over);

        grown = false;
    }

    let kind = entry.as_ref().map_or(0, |e| e.item_type);

    match kind {
        MOUNT => grown = false,
        PET_STAGE2 | MOUNT_SKIN_ITEM_TYPE => {}
        _ => return None,
    }

    let row = crate::data::mount::visual_for_mesh(visuals, row_index)?;

    let parts = [
        PartRef {
            mesh: row.grid[0][0],
            tex: row.grid[1][0],
        },
        PartRef {
            mesh: row.grid[0][1],
            tex: row.grid[1][1],
        },
        PartRef {
            mesh: row.grid[0][2],
            tex: row.grid[1][2],
        },
    ];

    let model = row.kind as u32;

    let band = slot(effect);

    let mut part_color = [0u8; MOUNT_PARTS];

    for (n, colour) in part_color.iter_mut().enumerate() {
        if row.grid[2][n] == -1 {
            continue;
        }

        *colour = match grown {
            true => band,
            false => row.grid[2][n] as u8,
        };
    }

    if !grown && row.grid[2][0] != -1 && part_color[0] == 0 {
        part_color = [band; MOUNT_PARTS];
    }

    Some(Mount {
        item,
        appearance_item: item,
        kind: 0,
        model: Some(model),
        parts,
        part_color,
        visual_kind: row.visual_kind as u8,
        scale: match row.scale == 1.0 {
            true => mount_auto_scale(model, parts[0].mesh),
            false => row.scale,
        },
    })
}

#[derive(Debug, Clone, PartialEq)]

pub struct Look {
    pub class: i32,
    pub helmet_item: i32,
    pub model: Option<u32>,
    pub variant: u32,
    pub parts: [PartRef; PARTS],
    pub part_color: [u8; PARTS],
    pub part_tier: [u8; PARTS],
    pub part_tint: [u8; PARTS],
    pub costume: Option<i32>,
    pub costume_body: i32,
    pub kingdom: u8,
    pub cape: Option<Cape>,
    pub mount: Option<Mount>,
    pub speed: f32,
    pub action: i32,
}

impl Look {
    pub fn hide_helmet(&mut self) {
        if self.costume.is_some() || !matches!(self.class, 1 | 2 | 4 | 8) {
            return;
        }

        self.parts[1] = PartRef {
            mesh: body_head_mesh(self.parts[0].mesh).unwrap_or(0),
            tex: 0,
        };

        self.part_color[1] = 0;

        self.part_tier[1] = 0;

        self.part_tint[1] = 0;
    }

    pub fn is_dressed(&self) -> bool {
        self.cape.is_some() || self.mount.is_some() || self.parts[1..].iter().any(|p| !p.is_empty())
    }

    pub fn is_mounted(&self) -> bool {
        self.mount.is_some()
    }

    pub fn wears_cape(&self) -> bool {
        self.cape.is_some() && self.model.is_some_and(|m| CAPE_MODELS.contains(&m))
    }

    pub fn cape_anchor(&self) -> Option<&'static [i32]> {
        self.wears_cape()
            .then(|| self.model.and_then(cape_anchor_bone))
            .flatten()
    }

    pub fn rider_anchor(&self) -> Option<AnchorRule> {
        let mount = self.mount?;

        anchor_rule(mount.model?, mount.parts[0].mesh)
    }

    pub fn rider_seat(&self, body_scale: f32) -> Option<RiderSeat> {
        let mount = self.mount?;

        rider_seat(mount, self.class, body_scale)
    }

    pub fn hand(
        &self,
        hand: Hand,
        main: i32,
        off: i32,
        items: &ItemList,
    ) -> Option<(i32, u32, HandGrip)> {
        let model = self.model?;

        let bone = hand_bone(model, hand)?;

        let mesh = self.parts[hand.part()].mesh;

        if !hand_mesh_effect_draws(model, mesh)
            && !(hand_model_draws(model) && hand_mesh_draws(mesh))
        {
            return None;
        }

        Some((
            bone,
            mesh as u32,
            self.equipped_grip(hand, main, off, items),
        ))
    }

    pub fn equipped_grip(&self, hand: Hand, main: i32, off: i32, items: &ItemList) -> HandGrip {
        let model = self.model.unwrap_or(0);

        let alternate = alternate_hand_grips(self.class, model, main, off, items);

        let other = self.parts[match hand {
            Hand::Off => Hand::Main.part(),
            Hand::Main => Hand::Off.part(),
        }]
        .mesh;

        hand_grip(model, hand, alternate[hand.part() - 6], other)
    }

    pub fn cape_clip(&self, action: usize) -> Option<usize> {
        cape_clip(self.is_mounted(), action)
    }

    pub fn cape_seat(&self, wclass: i16) -> Option<CapeSeat> {
        let cape = self.cape?;

        let mount_model = self.mount.and_then(|m| m.model).unwrap_or(0);

        let (yaw, pitch) = cape_rotation(mount_model);

        if let Some((offset, scale)) = cape_seat_for_stance(wclass) {
            return Some(CapeSeat {
                offset,
                scale,
                yaw,
                pitch,
            });
        }

        let scale = if self.model == Some(1) { 0.9 } else { 1.0 };

        let offset = match cape_offset_override(cape.item, cape.mesh) {
            Some(offset) => offset,
            None => {
                let back = cape_back_offset(self.class, self.parts[2].mesh)
                    .or_else(|| (self.costume_body == 100).then_some(0.1))?;

                [0.0, back]
            }
        };

        Some(CapeSeat {
            offset,
            scale,
            yaw,
            pitch,
        })
    }
}

pub const HAND_BONES: [[i32; 2]; 20] = [
    [19, 25],
    [18, 24],
    [15, 21],
    [15, 21],
    [12, 18],
    [22, 28],
    [23, 29],
    [20, 26],
    [24, 30],
    [23, 31],
    [32, 17],
    [22, 35],
    [34, 44],
    [34, 44],
    [34, 44],
    [34, 44],
    [34, 44],
    [34, 44],
    [34, 44],
    [34, 44],
];

pub const HAND_BONE_MODELS: u32 = 0x13;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]

pub enum Hand {
    Off,
    Main,
}

impl Hand {
    pub const BOTH: [Hand; 2] = [Hand::Off, Hand::Main];

    pub fn part(self) -> usize {
        match self {
            Hand::Off => 6,
            Hand::Main => 7,
        }
    }

    pub fn column(self) -> usize {
        match self {
            Hand::Off => 0,
            Hand::Main => 1,
        }
    }
}

pub fn hand_bone(model: u32, hand: Hand) -> Option<i32> {
    if model >= HAND_BONE_MODELS {
        return None;
    }

    HAND_BONES
        .get(model as usize)
        .map(|pair| pair[hand.column()])
}

pub fn hand_model_draws(model: u32) -> bool {
    matches!(model, 0 | 1 | 2 | 4 | 6 | 11 | 9 | 12 | 10 | 5 | 45 | 46)
}

pub fn hand_mesh_draws(mesh: i16) -> bool {
    let Ok(id) = u32::try_from(mesh) else {
        return false;
    };

    (12 < id && id < 1000) || (2701..=3000).contains(&id)
}

pub const EFFECT_MESH_MODELS: [u32; 13] = [0, 1, 2, 4, 6, 0x0B, 9, 0x0C, 10, 5, 0x2D, 0x2E, 0x2F];

pub const EFFECT_MESH_RANGES: [core::ops::RangeInclusive<u32>; 15] = [
    0xB49..=0xB55,
    0xB5A..=0xB5D,
    0xB60..=0xB6B,
    0xC37..=0xC41,
    0xBFC..=0xC06,
    0xC2B..=0xC35,
    0xC12..=0xC1B,
    0xC2A..=0xC2A,
    0xB79..=0xB83,
    0xB84..=0xB8E,
    0xB8F..=0xB99,
    0xB9A..=0xBBC,
    0xC07..=0xC11,
    0xBF4..=0xBF6,
    0xC1F..=0xC29,
];

pub const EFFECT_MESH_TAIL: [core::ops::RangeInclusive<u32>; 2] = [0xC42..=0xC4C, 0xB6C..=0xB78];

pub fn hand_mesh_effect_draws(model: u32, mesh: i16) -> bool {
    let Ok(id) = u32::try_from(mesh) else {
        return false;
    };

    if id == 0 || !EFFECT_MESH_MODELS.contains(&model) {
        return false;
    }

    EFFECT_MESH_RANGES
        .iter()
        .chain(EFFECT_MESH_TAIL.iter())
        .any(|range| range.contains(&id))
}

pub fn hand_offset(model: u32) -> f32 {
    if model == 1 { 0.07 } else { 0.1 }
}

pub const OFF_HAND_DROP_MESHES: core::ops::RangeInclusive<u32> = 0x6A5..=0x708;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct HandGrip {
    pub offset: [f32; 2],
    pub yaw: f32,
    pub pitch: f32,
    pub roll: f32,
}

fn alternate_hand_grips(
    class: i32,
    model: u32,
    main: i32,
    off: i32,
    items: &ItemList,
) -> [bool; 2] {
    if [main, off].into_iter().any(|id| {
        !(0..ITEM_ID_MODULUS).contains(&id)
            || items.get(id as usize).is_some_and(|item| item.mesh < 0)
    }) {
        return [false, false];
    }

    let claw = weapon_class(main, items) == 0x29;

    match class {
        0x1A | 0x21 | 0x28 | 0x24 | 0x25 | 0x3D | 0x3E => [false, false],
        0x3C => [false, claw && hand_mask(off, items) == 0],
        _ => [model == 10, claw || model == 11],
    }
}

pub fn hand_grip(model: u32, hand: Hand, alternate: bool, other: i16) -> HandGrip {
    use core::f32::consts::PI;

    let back = -hand_offset(model);

    match (hand, alternate) {
        (Hand::Off, true) => HandGrip {
            offset: [back, 0.01],
            yaw: 0.0,
            pitch: 0.0,
            roll: PI,
        },
        (Hand::Main, true) => HandGrip {
            offset: [back, 0.01],
            yaw: PI,
            pitch: 0.0,
            roll: 0.0,
        },
        (Hand::Off, false) => {
            let drop = match u32::try_from(other) {
                Ok(id) if OFF_HAND_DROP_MESHES.contains(&id) => -0.05,
                _ => 0.0,
            };

            HandGrip {
                offset: [back, drop + 0.01],
                yaw: PI,
                pitch: 0.0,
                roll: 0.0,
            }
        }
        (Hand::Main, false) => {
            let (x, y) = match model {
                6 => (back * 0.5, -0.01),
                9 => (back * 1.6, 0.03),
                _ => (back, -0.01),
            };

            let (yaw, pitch) = match model {
                1 => (-0.2618, -0.17453),
                6 => (-0.349, 0.0),
                _ => (0.0, 0.0),
            };

            HandGrip {
                offset: [x, y],
                yaw,
                pitch,
                roll: PI,
            }
        }
    }
}

pub const MOUNTED_ACTION_SHIFT: usize = 28;

pub fn mounted_action(action: usize) -> usize {
    action + MOUNTED_ACTION_SHIFT
}

pub const ACTION_STAND01: usize = 0;

pub const ACTION_STAND02: usize = 1;

pub const ACTION_WALK: usize = 2;

pub const ACTION_RUN: usize = 3;

pub const ACTION_ATTACK1: usize = 4;

pub const ACTION_ATTACK2: usize = 5;

pub const ACTION_ATTACK3: usize = 6;

pub const ACTION_SKILL01: usize = 7;

pub const ACTION_SKILL02: usize = 8;

pub const ACTION_SKILL03: usize = 9;

pub const ACTION_DIE: usize = 11;

pub const ACTION_DEAD: usize = 12;

pub const ACTION_STRIKE: usize = 10;

pub const ACTION_SEAT: usize = 13;

pub const ACTION_LEVELUP: usize = 14;

pub const ACTION_PUNISH: usize = 15;

pub const ACTION_CACKLE: usize = 16;

pub const ACTION_SEATING: usize = 24;

pub const ACTION_PUNING: usize = 26;

pub const ACTION_PUNEND: usize = 27;

pub const EMOTE_ACTIONS: usize = ACTION_PUNEND - ACTION_SEAT + 1;

pub fn is_emote_action(action: usize) -> bool {
    (ACTION_SEAT..=ACTION_PUNEND).contains(&action)
}

pub fn play_action(model: u32, parts: &[PartRef], is_dead: bool, action: usize) -> usize {
    let mesh = |i: usize| parts.get(i).map_or(0, |p: &PartRef| p.mesh);

    match model {
        0x1F if (ACTION_ATTACK1..=ACTION_ATTACK3).contains(&action) => {
            action + (ACTION_SKILL01 - ACTION_ATTACK1)
        }
        0x15 if mesh(0) > 1 && action == ACTION_WALK => ACTION_RUN,
        0x18 if is_dead && action == ACTION_WALK => ACTION_RUN,
        0x14 if mesh(0) != 0 && mesh(1) != 2 && action == ACTION_RUN => ACTION_WALK,
        _ => action,
    }
}

pub fn mount_auto_scale(model: u32, mesh: i16) -> f32 {
    match (model, mesh) {
        (0x1C, _) => 1.45,
        (0x23, 1) => 1.2,
        (0x19, 1) => 1.4,
        (0x14, 7 | 0x2F) => 0.6,
        (0x14, 0 | 0x28 | 0x29) => 1.3,
        (0x1D, 4) => 1.3,
        _ => 1.0,
    }
}

pub fn is_mount_item(item: i32) -> bool {
    MOUNT_ITEM_RANGES
        .iter()
        .any(|&(lo, hi)| (lo..=hi).contains(&item))
}

fn barding_mesh(item: i32) -> i16 {
    match item {
        2366..=2370 => (item - 2365) as i16,
        2371..=2375 => (item - 2370) as i16,
        _ => 0,
    }
}

pub const WEAPON_CLASS_EFFECT: i16 = 0x15;

const ONE_HANDED: i32 = item_list::equip::WEAPON as i32;

const SHIELD: i32 = item_list::equip::SHIELD as i32;

const TWO_HANDED: i32 = item_list::equip::TWO_HANDED as i32;

const TWO_HANDED_ARMOUR: i32 = (item_list::equip::TWO_HANDED | item_list::equip::ARMOR) as i32;

#[allow(clippy::manual_range_patterns)]

pub fn anim_group(
    class: i32,
    model: u32,
    mounted: bool,
    body_mesh: i16,
    main: i32,
    off: i32,
    items: &ItemList,
) -> Option<usize> {
    if !(0..ITEM_ID_MODULUS).contains(&main) || !(0..ITEM_ID_MODULUS).contains(&off) {
        return None;
    }

    let mesh = |id: i32| {
        usize::try_from(id)
            .ok()
            .and_then(|id| items.get(id))
            .map_or(0, |e| e.mesh)
    };

    if mesh(off) < 0 || mesh(main) < 0 {
        return None;
    }

    let weapon = weapon_class(main, items);

    let off_class = weapon_class(off, items);

    let main_mask = hand_mask(main, items);

    let off_mask = hand_mask(off, items);

    match class {
        0x1A | 0x21 | 0x28 => return Some(0),
        0x24 => {
            return match weapon {
                1 | 0x3D | 0x1F => Some(0xB),
                0x0B => Some(0xF),
                0x0D => Some(0xC),
                0x15 | 0x16 | 0x17 => Some(0xD),
                3 | 0x3F => Some(0xE),
                _ => None,
            };
        }
        0x25 => return Some(0xB),
        0x3C => {
            return match (weapon, off_mask) {
                (0x29, 0) => Some(0),
                (0x0B, SHIELD) => Some(1),
                (0x0B, TWO_HANDED) => Some(4),
                _ => Some(0),
            };
        }
        0x3D => return Some(1),
        0x3E => {
            return match weapon {
                1 | 0x0B | 0x3D | 2 | 0x0C | 0x3E | 0x1F => Some(1),
                _ => Some(0),
            };
        }
        _ => {}
    }

    let ladder = match (model, mounted) {
        (0, false) => ch01_animation_group(weapon, off_class, main_mask, off_mask),
        (0, true) => mounted_ch01_animation_group(weapon, main_mask, off_mask),
        (1, false) => ch02_animation_group(weapon, off_class, main_mask, off_mask),
        (1, true) => mounted_ch02_animation_group(weapon, main_mask, off_mask),
        (2, false) => or01_animation_group(weapon, off_mask),
        (2, true) => mounted_or01_animation_group(weapon),
        (3, _) => match mounted || body_mesh == 0 {
            true => Some(0),
            false => match body_mesh == 1 {
                true => Some(1),
                false => None,
            },
        },
        (4, _) => match weapon {
            1 | 0x0B | 0x3D => Some(1),
            0x15 | 0x16 | 0x17 | 0x0D => Some(2),
            0x66 | 0x67 => Some(3),
            _ => Some(0),
        },
        _ => None,
    };

    match model {
        0x0A => Some(0),
        _ => ladder,
    }
}

pub fn weapon_class(item: i32, items: &ItemList) -> i32 {
    unmodified_item_effect(item, items, WEAPON_CLASS_EFFECT as u8)
}

pub fn hand_mask(item: i32, items: &ItemList) -> i32 {
    unmodified_item_effect(item, items, effect::EQUIP_MASK)
}

pub fn ground_item_mesh(item: i32, items: &ItemList) -> i32 {
    let mask = hand_mask(item, items);

    let class = unmodified_item_effect(item, items, APPEARANCE_EFFECT as u8);

    let t = i32::from(
        u16::try_from(item)
            .ok()
            .and_then(|id| items.get(usize::from(id)))
            .map_or(0, |e| e.mesh),
    );

    if (0x28..0x32).contains(&t) {
        if mask & 4 == 0 && mask & 0x38 == 0 {
            return standard_ground_item_mesh(t, class, mask);
        }

        if t == 0x28 {
            return 0;
        }

        let class_off = match class {
            1 => 0,
            2 => 2,
            4 => 1,
            8 => 3,
            _ => 0,
        };

        let hand = match () {
            _ if mask & 4 != 0 => 0,
            _ if mask & 8 != 0 => 1,
            _ if mask & 0x10 != 0 => 2,
            _ => 3,
        };

        return 0x4D5 + 4 * (t + class_off) + hand;
    }

    standard_ground_item_mesh(t, class, mask)
}

fn standard_ground_item_mesh(t: i32, class: i32, mask: i32) -> i32 {
    let class_off = match class {
        1 => 0,
        2 => 0xC8,
        4 => 0x14,
        8 => 0xDC,
        c if c > 8 => return t,
        _ => 0,
    };

    match () {
        _ if mask & 0x02 != 0 => t + class_off + 0x3E9,
        _ if mask & 0x04 != 0 => t + class_off + 0x411,
        _ if mask & 0x08 != 0 => t + class_off + 0x439,
        _ if mask & 0x10 != 0 => t + class_off + 0x461,
        _ if mask & 0x20 != 0 => t + class_off + 0x489,
        _ => t,
    }
}

fn unmodified_item_effect(item: i32, items: &ItemList, id: u8) -> i32 {
    let index = u16::try_from(item).unwrap_or(0);

    let bare = account::Item {
        index,
        ..account::Item::default()
    };

    Effects::new(items, &[]).total_scaled(&bare, id)
}

fn animation_group_if_one_handed(main_mask: i32, group: usize) -> Option<usize> {
    match main_mask == ONE_HANDED {
        true => Some(group),
        false => None,
    }
}

#[allow(clippy::manual_range_patterns)]

fn ch01_animation_group(
    weapon: i32,
    off_class: i32,
    main_mask: i32,
    off_mask: i32,
) -> Option<usize> {
    if weapon == 0 && off_mask == SHIELD {
        return Some(2);
    }

    if main_mask == TWO_HANDED && off_mask == TWO_HANDED {
        return Some(4);
    }

    match weapon {
        1 | 0x0B | 0x3D | 0x1F if off_class == 0 => Some(1),
        1 | 0x0B | 0x3D | 0x1F => match off_mask {
            SHIELD => Some(1),
            TWO_HANDED_ARMOUR => Some(4),
            _ => None,
        },
        2 | 0x0C | 0x3E if off_mask == SHIELD => Some(3),
        2 | 0x0C | 0x3E if off_class == 0 => Some(5),
        2 | 0x0C | 0x3E => match off_mask == TWO_HANDED_ARMOUR {
            true => Some(4),
            false => None,
        },
        3 | 0x3F => Some(6),
        0x0D => animation_group_if_one_handed(main_mask, 7),
        0x15 | 0x16 | 0x17 => animation_group_if_one_handed(main_mask, 8),
        0x66 | 0x67 => animation_group_if_one_handed(main_mask, 10),
        0x68 => animation_group_if_one_handed(main_mask, 9),
        0x65 => Some(3),
        0x20 | 0x21 => Some(5),
        _ => Some(0),
    }
}

fn mounted_ch01_animation_group(weapon: i32, main_mask: i32, off_mask: i32) -> Option<usize> {
    if weapon == 0 && off_mask == SHIELD {
        return Some(3);
    }

    if main_mask == TWO_HANDED && off_mask == TWO_HANDED {
        return Some(2);
    }

    match weapon {
        1 | 0x0B | 0x3D | 2 | 0x0C | 0x3E | 0x1F => Some(1),
        0x15 | 0x16 | 0x17 | 0x0D | 3 | 0x3F | 0x20 | 0x21 => {
            animation_group_if_one_handed(main_mask, 4)
        }
        0x65 => Some(5),
        0x66 | 0x67 => animation_group_if_one_handed(main_mask, 1),
        _ => Some(0),
    }
}

#[allow(clippy::manual_range_patterns)]

fn ch02_animation_group(
    weapon: i32,
    off_class: i32,
    main_mask: i32,
    off_mask: i32,
) -> Option<usize> {
    if weapon == 0 && off_mask == SHIELD {
        return Some(2);
    }

    if main_mask == TWO_HANDED && off_mask == TWO_HANDED {
        return Some(4);
    }

    match weapon {
        3 | 0x3F => Some(10),
        1 | 0x0B | 0x3D | 2 | 0x0C | 0x3E | 0x1F if off_class == 0 => Some(1),
        1 | 0x0B | 0x3D | 2 | 0x0C | 0x3E | 0x1F => match off_mask {
            SHIELD => Some(3),
            TWO_HANDED_ARMOUR => Some(4),
            _ => None,
        },
        0x15 | 0x16 | 0x17 => Some(5),
        0x66 | 0x67 => animation_group_if_one_handed(main_mask, 3),
        0x0D => Some(7),
        0x20 | 0x21 => animation_group_if_one_handed(main_mask, 9),
        0x65 => animation_group_if_one_handed(main_mask, 6),
        _ => Some(0),
    }
}

fn mounted_ch02_animation_group(weapon: i32, main_mask: i32, off_mask: i32) -> Option<usize> {
    if weapon == 0 && off_mask == SHIELD {
        return Some(3);
    }

    if main_mask == TWO_HANDED && off_mask == TWO_HANDED {
        return Some(2);
    }

    match weapon {
        1 | 0x0B | 0x3D | 2 | 0x0C | 0x3E | 0x1F => Some(1),
        0x15 | 0x16 | 0x17 | 0x0D | 3 | 0x3F | 0x20 | 0x21 => {
            animation_group_if_one_handed(main_mask, 4)
        }
        0x65 => Some(5),
        0x66 | 0x67 => animation_group_if_one_handed(main_mask, 1),
        _ => Some(0),
    }
}

#[allow(clippy::manual_range_patterns)]

fn or01_animation_group(weapon: i32, off_mask: i32) -> Option<usize> {
    if weapon == 0x65 {
        return Some(1);
    }

    if weapon == 0x0C {
        return Some(2);
    }

    if off_mask == SHIELD {
        return Some(5);
    }

    match weapon {
        1 | 0x0B => Some(5),
        0x0D | 0x15 => Some(3),
        0x1F | 0x20 | 0x21 => Some(4),
        _ => Some(0),
    }
}

fn mounted_or01_animation_group(weapon: i32) -> Option<usize> {
    match weapon {
        0x65 => Some(1),
        1 | 0x0B | 0x0C | 0x0D | 0x1F | 0x20 | 0x21 => Some(5),
        _ => Some(0),
    }
}

pub const TINT_SLOTS: [(usize, usize); PARTS] = [
    (0, 0),
    (1, 1),
    (2, 2),
    (3, 3),
    (4, 4),
    (5, 5),
    (7, 6),
    (6, 7),
];

pub fn item_rank(entry: &item_list::Entry) -> u8 {
    entry.tier as u8
}

pub const ITEM_ID_MODULUS: i32 = 0x38A4;

pub fn is_costume(id: i32, items: &ItemList) -> bool {
    let Ok(narrow) = i16::try_from(id) else {
        return false;
    };

    if (narrow as u16).wrapping_sub(1) > 0x38A2 {
        return false;
    }

    if COSTUME_ID_WINDOWS
        .iter()
        .any(|&(lo, hi)| (lo..=hi).contains(&id))
    {
        return true;
    }

    usize::try_from(id)
        .ok()
        .and_then(|id| items.get(id))
        .is_some_and(|entry| entry.item_type == item_list::item_type::COSTUME_SET)
}

pub const COSTUME_ID_WINDOWS: [(i32, i32); 2] = [(0x1036, 0x105D), (0x10CC, 0x1193)];

pub fn refine_level(item: &account::Item, items: &ItemList) -> u32 {
    let Some(raw) = refine_byte(item, items) else {
        return 0;
    };

    let id = i32::from(item.index as i16);

    let mut value = u32::from(raw);

    if !matches!(id, 0x312 | 0x790 | 0x791) {
        value = if value < 0xE6 {
            value % 10
        } else {
            value - 0xDC
        };

        if (10..36).contains(&value) {
            value = (value - 10) / 4 + 10;
        }
    }

    value
}

pub fn refine_uncompressed(item: &account::Item, items: &ItemList) -> u32 {
    let Some(raw) = refine_byte(item, items) else {
        return 0;
    };

    let value = u32::from(raw);

    if value < 0xE6 {
        value % 10
    } else {
        value - 0xDC
    }
}

fn refine_byte(item: &account::Item, items: &ItemList) -> Option<u8> {
    let id = i32::from(item.index as i16);

    if !(1..=0x38A3).contains(&id) {
        return None;
    }

    let kind = usize::try_from(id)
        .ok()
        .and_then(|id| items.get(id))
        .map_or(0, |e| e.item_type);

    if matches!(kind, 0x15..=0x17) || (0xC7F..0xCE4).contains(&id) {
        return None;
    }

    let is_alt = |e: &account::Effect| (0x73..=0x7E).contains(&e.id);

    Some(if item.effects[0].id == b'+' {
        item.effects[0].value
    } else if let Some(e) = item.effects.iter().find(|e| e.id == b'+') {
        e.value
    } else {
        item.effects
            .iter()
            .find(|e| is_alt(e))
            .map_or(0, |e| e.value)
    })
}

pub fn refine_rank(item: &account::Item, items: &ItemList) -> u8 {
    if refine_level(item, items) < 10 {
        return 0;
    }

    let carries = |e: &&account::Effect| e.id == b'+' || (0x73..=0x7E).contains(&e.id);

    let value = item.effects.iter().find(carries).map_or(0, |e| e.value);

    (value.wrapping_sub(2) & 3) + 5
}

fn spread(value: u8, helmet_is_armour: bool) -> [u8; PARTS] {
    let mut out = [0u8; PARTS];

    let first = if helmet_is_armour { 1 } else { 2 };

    out[first..].fill(value);

    out
}

pub const PART_ONE_FROM_PART_ZERO: [u32; 32] = [
    0x14, 0x27, 0x15, 0x16, 0x17, 0x18, 0x28, 3, 4, 0x19, 0x1C, 0x1D, 2, 6, 7, 8, 0x1E, 0x1F, 0x21,
    0x24, 0x0C, 0x2B, 0x0A, 5, 0x2D, 0x2E, 0x2F, 0x35, 0x36, 0x37, 0x38, 0x39,
];

fn cape_mesh_override(armour_mesh: i16, side: u8, current: i16) -> Option<i16> {
    let raised = i16::from(side as i8) + 199;

    match armour_mesh {
        0x2F | 0x30 | 0x33 if i32::from(current as u16) < i32::from(raised) => Some(raised),
        0x32 => Some(i16::from(side as i8) + 0xD1),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

struct Dressing {
    class: i32,
    body: i32,
    helmet: i32,
    costume_id: i32,
    cape_side: u8,
}

fn apply_dressing_overrides(
    items: &ItemList,
    dressing: Dressing,
    model: &mut Option<u32>,
    cape_mesh: &mut i16,
    parts: &mut [PartRef; PARTS],
    part_color: &mut [u8; PARTS],
    part_tier: &mut [u8; PARTS],
    part_tint: &mut [u8; PARTS],
) -> (bool, i32) {
    let Dressing {
        class,
        body,
        helmet,
        costume_id,
        cape_side,
    } = dressing;

    if let Some(raised) = cape_mesh_override(parts[2].mesh, cape_side, *cape_mesh) {
        *cape_mesh = raised;
    }

    let (lo, hi) = costume::EXTRA_ID_WINDOW;

    let wears_costume =
        is_costume(costume_id, items) || ((lo..=hi).contains(&costume_id) && class != 0x1D);

    let mut costume_body = 0;

    if wears_costume {
        let chosen = costume::body_for(costume_id, body, cape_side);

        parts[0] = PartRef::EMPTY;

        parts[1].mesh = 0;

        parts[2..=5].fill(PartRef::EMPTY);

        part_color[..6].fill(chosen.colour);

        part_tier[..6].fill(0);

        *model = Some(chosen.model);

        costume_body = chosen.body;
    } else if (3503..=3506).contains(&helmet) {
        costume_body = 100;

        parts[1] = PartRef::EMPTY;

        for part in &mut parts[2..6] {
            part.tex = 0;
        }

        let mesh = match body {
            6..=9 => Some(95),
            16..=19 => Some(97),
            26..=29 => Some(76),
            36..=39 => Some(78),
            _ => None,
        };

        if let Some(mesh) = mesh {
            for part in &mut parts[2..6] {
                part.mesh = mesh;
            }
        }

        if (22..=25).contains(&body) || class == 63 {
            let part = parts[0];

            parts[1..6].fill(part);

            let color = part_color[0];

            part_color[1..6].fill(color);

            let tier = part_tier[0];

            part_tier[1..6].fill(tier);

            let tint = part_tint[0];

            part_tint[1..6].fill(tint);
        } else {
            part_color[..6].fill(0);

            part_tier[..6].fill(0);
        }
    }

    if costume_body == 0 && PART_ONE_FROM_PART_ZERO.contains(&model.unwrap_or(0)) {
        parts[1] = parts[0];

        part_color[1] = part_color[0];

        part_tier[1] = part_tier[0];

        part_tint[1] = part_tint[0];

        match model.unwrap_or(0) {
            45 | 46 => {
                part_color.fill(0);

                part_tier.fill(0);

                part_color[2] = if *model == Some(45) { 8 } else { 13 };

                part_tier[2] = 2;
            }
            53 | 55 | 56 | 57 => {
                part_color.fill(0);

                part_tier.fill(0);

                let count = match *model {
                    Some(53) => 3,
                    Some(55) => 2,
                    _ => 5,
                };

                part_color[..count].fill(13);
            }
            _ => {}
        }
    }

    if class == 0x28 {
        parts[6].mesh = 0;

        parts[7].mesh = 0;
    }

    let last = if matches!(class, 0x27 | 0x28 | 0x3f) {
        5
    } else if matches!(model.unwrap_or(0), 0x1a | 0x23 | 0x27) {
        2
    } else if matches!(model.unwrap_or(0), 0x0b | 0x25 | 0x2c) {
        4
    } else {
        0
    };

    if last != 0 {
        let part = parts[0];

        parts[1..=last].fill(part);

        let color = part_color[0];

        part_color[1..=last].fill(color);

        let tier = part_tier[0];

        part_tier[1..=last].fill(tier);

        let tint = part_tint[0];

        part_tint[1..=last].fill(tint);
    }

    if class == 0x3e && parts[0].mesh == 2 {
        part_color[0] = 4;

        part_color[1] = 4;
    }

    apply_body_defaults(parts);

    if !MMULTI_CLASSES.contains(&class) && matches!(*model, Some(0 | 1)) {
        apply_armour_head(parts, part_color, part_tier);
    }

    (wears_costume, costume_body)
}

fn apply_armour_head(
    parts: &mut [PartRef; PARTS],
    part_color: &mut [u8; PARTS],
    part_tier: &mut [u8; PARTS],
) {
    let armour = parts[2].mesh;

    if matches!(armour, 47 | 48 | 50 | 51) {
        parts[0].mesh = if armour == 51 { 47 } else { armour };

        parts[1].mesh = armour;

        part_color[1] = part_color[2];

        part_tier[1] = part_tier[2];

        if matches!(armour, 50 | 51) {
            part_color[0] = part_color[2];

            part_tier[0] = part_tier[2];
        }
    }
}

fn body_head_mesh(face: i16) -> Option<i16> {
    matches!(face, 40 | 64 | 80).then_some(face)
}

fn apply_body_defaults(parts: &mut [PartRef; PARTS]) {
    const DEFAULT_MESH: i16 = 0x28;

    let Some(head) = body_head_mesh(parts[0].mesh) else {
        return;
    };

    for slot in &mut parts[2..=5] {
        if slot.mesh == 0 {
            slot.mesh = DEFAULT_MESH;
        }
    }

    parts[1].mesh = head;
}

impl Look {
    pub fn resolve(actor: &Actor, items: &ItemList) -> Option<Look> {
        let part_of = |id: i32| -> PartRef {
            match usize::try_from(id).ok().and_then(|id| items.get(id)) {
                Some(entry) => PartRef {
                    mesh: entry.mesh,
                    tex: entry.mesh_sub,
                },
                None => PartRef::EMPTY,
            }
        };

        let tier_of = |id: i32| -> u8 {
            usize::try_from(id)
                .ok()
                .and_then(|id| items.get(id))
                .map_or(0, |entry| item_rank(&entry))
        };

        let face = items.get(usize::try_from(actor.face).ok()?)?;

        let class = face.effect_sum(APPEARANCE_EFFECT);

        let model = model_for_class(class);

        let variant = u32::from(VARIANT_CLASSES.contains(&class));

        let armour = part_of(actor.armour);

        let mut parts = [PartRef::EMPTY; PARTS];

        parts[0] = PartRef {
            mesh: face.mesh,
            tex: face.mesh_sub,
        };

        parts[1] = part_of(actor.helmet);

        parts[2..=5].fill(armour);

        parts[6] = part_of(actor.left_hand);

        parts[7] = part_of(actor.right_hand);

        let colour = actor.equipment_color as u8;

        let helmet_is_armour = actor.helmet == actor.armour;

        let mut part_color = spread(colour, helmet_is_armour);

        let mut part_tier = spread(tier_of(actor.armour), helmet_is_armour);

        let side = u8::from(actor.cape > 0);

        let mut cape_mesh = part_of(actor.cape).tex;

        let mut model = model;

        apply_dressing_overrides(
            items,
            Dressing {
                class,
                body: 0,
                helmet: 0,
                costume_id: 0,
                cape_side: side,
            },
            &mut model,
            &mut cape_mesh,
            &mut parts,
            &mut part_color,
            &mut part_tier,
            &mut [0; PARTS],
        );

        let cape = (actor.cape > 0).then(|| Cape {
            item: actor.cape,
            mesh: cape_mesh,
            rank: colour,
            refine: 0,
            tier: tier_of(actor.cape),
            side,
        });

        let mount = is_mount_item(actor.mount).then(|| {
            let appearance_item = actor.mount - MOUNT_APPEARANCE_SHIFT;

            let body = part_of(appearance_item);

            let model = usize::try_from(appearance_item)
                .ok()
                .and_then(|id| items.get(id))
                .and_then(|entry| model_for_class(entry.effect_sum(APPEARANCE_EFFECT)));

            let scale = if appearance_item == MOUNT_SCALE_OVERRIDE_ITEM {
                1.1
            } else {
                mount_auto_scale(model.unwrap_or(0), body.mesh)
            };

            Mount {
                item: actor.mount,
                appearance_item,
                kind: (actor.mount - MOUNT_KIND_BASE) as i16,
                model,
                parts: [
                    body,
                    body,
                    PartRef {
                        mesh: barding_mesh(actor.mount),
                        tex: 0,
                    },
                ],
                part_color: [0; MOUNT_PARTS],
                visual_kind: 0,
                scale,
            }
        });

        Some(Look {
            class,
            helmet_item: 0,
            model,
            variant,
            parts,
            part_color,
            part_tier,
            part_tint: [0; PARTS],
            costume: None,
            costume_body: 0,
            kingdom: side,
            cape,
            mount,
            speed: actor.tail.first().copied().unwrap_or(0) as f32,
            action: actor.tail.get(1).copied().unwrap_or(0),
        })
    }

    pub fn for_character(equip: &Equipment, items: &ItemList) -> Option<Look> {
        let worn = |slot: usize| -> i32 {
            let item = equip.items[slot];

            match item.appearance {
                0 => i32::from(item.index as i16) % ITEM_ID_MODULUS,
                over => i32::from(over as i16),
            }
        };

        let part_of = |id: i32| -> PartRef {
            match usize::try_from(id).ok().and_then(|id| items.get(id)) {
                Some(entry) => PartRef {
                    mesh: entry.mesh,
                    tex: entry.mesh_sub,
                },
                None => PartRef::EMPTY,
            }
        };

        let tier_of = |id: i32| -> u8 {
            usize::try_from(id)
                .ok()
                .and_then(|id| items.get(id))
                .map_or(0, |e| item_rank(&e))
        };

        let authored = i32::from(equip.body().index as i16) % ITEM_ID_MODULUS;

        let body = match GM_BODY_IDS.contains(&authored) {
            true => GM_BODY_SUBSTITUTE,
            false => authored,
        };

        let face = items.get(usize::try_from(body).ok()?)?;

        let costume = equip.costume();

        let class = face.effect_sum(APPEARANCE_EFFECT);

        let model = model_for_class(class);

        let variant = u32::from(VARIANT_CLASSES.contains(&class));

        let helmet = worn(1);

        let mut parts = [PartRef::EMPTY; PARTS];

        parts[0] = match i32::from(costume.index) == body {
            true => PartRef {
                mesh: 0x28,
                tex: 0x28,
            },
            false => PartRef {
                mesh: face.mesh,
                tex: face.mesh_sub,
            },
        };

        if !HELMET_SKIPPED
            .iter()
            .any(|&(lo, hi)| (lo..=hi).contains(&helmet))
            && !HELMET_SKIPPED_IDS.contains(&helmet)
        {
            parts[1] = part_of(helmet);
        }

        for (slot, part) in parts.iter_mut().enumerate().take(6).skip(2) {
            *part = part_of(worn(slot));
        }

        parts[7] = part_of(worn(6));

        parts[6] = part_of(worn(7));

        let mut part_color = [0u8; PARTS];

        let mut part_tier = [0u8; PARTS];

        if authored % 10 != 8 && !COLOURLESS_BODY_IDS.contains(&authored) {
            part_color[0] = refine_level(&equip.body(), items) as u8;
        }

        part_tier[0] = tier_of(body);

        for slot in 1..=5 {
            part_color[slot] = refine_level(&equip.items[slot], items) as u8;

            part_tier[slot] = tier_of(worn(slot));
        }

        part_color[7] = refine_level(&equip.items[6], items) as u8;

        part_tier[7] = tier_of(worn(6));

        part_color[6] = refine_level(&equip.items[7], items) as u8;

        part_tier[6] = tier_of(worn(7));

        let mut part_tint = [0u8; PARTS];

        for (part, slot) in TINT_SLOTS {
            part_tint[part] = glow::tint(&equip.items[slot]);
        }

        for (part, slot) in [(1, 1), (2, 2), (3, 3), (4, 4), (5, 5), (7, 6), (6, 7)] {
            if part_tier[part] < 5 && part_color[part] >= 10 {
                part_tier[part] = refine_rank(&equip.items[slot], items) + 4;
            }
        }

        let cape_item = i32::from(equip.cape().index as i16);

        let mut cape_mesh = part_of(cape_item).tex;

        let side = match cape_item > 0 {
            true => cape_side(cape_mesh),
            false => 0,
        };

        let left = worn(6);

        let left_class = usize::try_from(left)
            .ok()
            .and_then(|id| items.get(id))
            .map_or(0, |e| e.effect_sum(WEAPON_CLASS_EFFECT));

        if left_class == 0x29 {
            parts[6] = parts[7];
        }

        let costume_id = i32::from(if costume.appearance >= 500 {
            costume.appearance
        } else {
            costume.index
        });

        let mut model = model;

        let (wears_costume, costume_body) = apply_dressing_overrides(
            items,
            Dressing {
                class,
                body,
                helmet,
                costume_id,
                cape_side: side,
            },
            &mut model,
            &mut cape_mesh,
            &mut parts,
            &mut part_color,
            &mut part_tier,
            &mut part_tint,
        );

        let cape = (cape_item > 0 && matches!(costume_body, 0 | 100)).then(|| Cape {
            item: cape_item,
            mesh: cape_mesh,
            rank: refine_level(&equip.cape(), items) as u8,
            refine: refine_level(&equip.cape(), items) as u8,
            tier: tier_of(worn(15)),
            side,
        });

        Some(Look {
            class,
            helmet_item: helmet,
            model,
            variant,
            parts,
            part_color,
            part_tier,
            part_tint,
            costume: wears_costume.then_some(costume_id),
            costume_body,
            kingdom: side,
            cape,
            mount: None,
            speed: 0.0,
            action: 0,
        })
    }

    pub fn for_field(
        equip: &Equipment,
        effects: &Effects,
        visuals: &[MountVisual],
        wearer: Wearer,
        costume_item: u16,
    ) -> Option<Look> {
        Self::for_field_with_arena(equip, effects, visuals, wearer, costume_item, None)
    }

    pub fn for_field_with_arena(
        equip: &Equipment,
        effects: &Effects,
        visuals: &[MountVisual],
        mut wearer: Wearer,
        costume_item: u16,
        arena: Option<(&crate::data::arena_visual::Record, i32)>,
    ) -> Option<Look> {
        let items = effects.items;

        let arena_hands = [6usize, 7].map(|slot| {
            arena.and_then(|(row, _)| {
                let index = equip.items[slot].index;

                if index == 0 {
                    return None;
                }

                let family = effects.total_scaled(
                    &account::Item {
                        index,
                        ..Default::default()
                    },
                    0x15,
                );

                row.equip_override_for(family).map(|id| id as u16)
            })
        });

        let arena_costume = arena
            .map(|(row, class)| row.head_for_class_family(class as usize) as u16)
            .filter(|id| *id != 0);

        if arena_costume.is_some() {
            wearer.flags = 0;
        }

        let worn = |slot: usize| -> i32 {
            if (6..=7).contains(&slot)
                && let Some(id) = arena_hands[slot - 6]
            {
                return i32::from(id as i16);
            }

            let item = equip.items[slot];

            match item.appearance {
                0 => i32::from(item.index as i16),
                over => i32::from(over as i16),
            }
        };

        let part_of = |id: i32| -> PartRef {
            match usize::try_from(id).ok().and_then(|id| items.get(id)) {
                Some(entry) => PartRef {
                    mesh: entry.mesh,
                    tex: entry.mesh_sub,
                },
                None => PartRef::EMPTY,
            }
        };

        let authored_tier = |slot: usize| -> u8 {
            let id = i32::from(equip.items[slot].index as i16);

            usize::try_from(id)
                .ok()
                .and_then(|id| items.get(id))
                .map_or(0, |e| item_rank(&e))
        };

        let body = i32::from(equip.body().index as i16);

        let face = items.get(usize::try_from(body).ok()?)?;

        let raw_costume = i32::from(arena_costume.unwrap_or_else(|| {
            let item = equip.costume();

            if item.appearance >= 500 {
                item.appearance
            } else {
                item.index
            }
        }) as i16);

        let costume_id = match is_costume(raw_costume, items) && wearer.flags & FLAG_NO_COSTUME == 0
        {
            true => raw_costume,
            false => 0,
        };

        let class = appearance_class(effects, equip.body().index, costume_id, costume_item);

        let model = model_for_class(class);

        let variant = u32::from(VARIANT_CLASSES.contains(&class));

        let mut parts = [PartRef::EMPTY; PARTS];

        parts[0] = PartRef {
            mesh: face.mesh,
            tex: face.mesh_sub,
        };

        let helmet = i32::from(equip.items[1].index as i16);

        if !HELMET_SKIPPED
            .iter()
            .any(|&(lo, hi)| (lo..=hi).contains(&helmet))
            && !HELMET_SKIPPED_IDS.contains(&helmet)
        {
            parts[1] = part_of(worn(1));
        }

        for (slot, part) in parts.iter_mut().enumerate().take(6).skip(2) {
            *part = part_of(worn(slot));
        }

        parts[7] = part_of(worn(6));

        parts[6] = part_of(worn(7));

        let mut part_color = [0u8; PARTS];

        let mut part_tier = [0u8; PARTS];

        for slot in 0..=5 {
            part_color[slot] = refine_level(&equip.items[slot], items) as u8;

            part_tier[slot] = authored_tier(slot);
        }

        part_color[7] = refine_level(&equip.items[6], items) as u8;

        part_tier[7] = authored_tier(6);

        part_color[6] = refine_level(&equip.items[7], items) as u8;

        part_tier[6] = authored_tier(7);

        if arena_hands.iter().any(Option::is_some)
            && let Some((row, _)) = arena
        {
            part_color[6] = row.color_effect as u8;

            part_color[7] = row.color_effect as u8;
        }

        let mut part_tint = [0u8; PARTS];

        for (part, slot) in TINT_SLOTS {
            part_tint[part] = glow::tint(&equip.items[slot]);
        }

        for (part, slot) in [
            (0, 0),
            (1, 1),
            (2, 2),
            (3, 3),
            (4, 4),
            (5, 5),
            (7, 6),
            (6, 7),
        ] {
            if part_tier[part] < 5 && part_color[part] >= 10 {
                part_tier[part] = refine_rank(&equip.items[slot], items) + 4;
            }
        }

        let wears_cape = equip.cape().index as i16 > 0;

        let cape_item = worn(15);

        let mut cape_mesh = part_of(cape_item).tex;

        let side = match wears_cape {
            true => cape_side(cape_mesh),
            false => 0,
        };

        if effects.total_scaled(&equip.items[6], WEAPON_CLASS_EFFECT as u8) == 0x29 {
            parts[6] = parts[7];

            part_color[6] = part_color[7];

            part_tier[6] = part_tier[7];
        }

        let helmet_colors = part_color;

        let helmet_tiers = part_tier;

        let mut model = model;

        let (wears_costume, costume_body) = apply_dressing_overrides(
            items,
            Dressing {
                class,
                body,
                helmet,
                costume_id,
                cape_side: side,
            },
            &mut model,
            &mut cape_mesh,
            &mut parts,
            &mut part_color,
            &mut part_tier,
            &mut part_tint,
        );

        if wearer.flags & 4 != 0
            && let Some(face) = usize::try_from(body).ok().and_then(|id| items.get(id))
        {
            for part in 0..2 {
                if matches!(
                    i32::from(parts[part].mesh) + variant as i32 * 20,
                    47 | 48 | 51 | 67 | 68 | 71
                ) {
                    parts[part] = PartRef {
                        mesh: face.mesh,
                        tex: 0,
                    };

                    part_color[part] = helmet_colors[part];

                    part_tier[part] = helmet_tiers[part];
                }
            }
        }

        let cape = (wears_cape && matches!(costume_body, 0 | 100)).then(|| Cape {
            item: cape_item,
            mesh: cape_mesh,
            rank: cape_rank(wearer.rank_word),
            refine: refine_level(&equip.cape(), items) as u8,
            tier: usize::try_from(cape_item)
                .ok()
                .and_then(|id| items.get(id))
                .map_or(0, |e| item_rank(&e)),
            side,
        });

        Some(Look {
            class,
            helmet_item: worn(1),
            model,
            variant,
            parts,
            part_color,
            part_tier,
            part_tint,
            costume: wears_costume.then_some(costume_id),
            costume_body,
            kingdom: side,
            cape,
            mount: mount_for_field_override(
                equip,
                effects,
                visuals,
                wearer.flags,
                arena.map_or(0, |(row, _)| row.mob_template_id as u16),
            ),
            speed: 0.0,
            action: 0,
        })
    }

    pub fn for_spawn(
        equip: &[u16],
        effect: &[u8],
        refine: &[u8],
        level: u16,
        flags: u8,
        costume_item: u16,
        effects: &Effects,
        visuals: &[MountVisual],
    ) -> Option<Look> {
        let items = effects.items;

        let worn = |slot: usize| -> i32 { i32::from(equip.get(slot).copied().unwrap_or(0) as i16) };

        let part_of = |id: i32| -> PartRef {
            match usize::try_from(id).ok().and_then(|id| items.get(id)) {
                Some(entry) => PartRef {
                    mesh: entry.mesh,
                    tex: entry.mesh_sub,
                },
                None => PartRef::EMPTY,
            }
        };

        let tier_of = |slot: usize| -> u8 {
            usize::try_from(worn(slot))
                .ok()
                .and_then(|id| items.get(id))
                .map_or(0, |e| item_rank(&e))
        };

        let colour_of = |slot: usize| -> u8 { effect.get(slot).copied().unwrap_or(0) };

        let body = worn(0);

        let face = items.get(usize::try_from(body).ok()?)?;

        let raw_costume = worn(12);

        let costume_id = match is_costume(raw_costume, items) && flags & FLAG_NO_COSTUME == 0 {
            true => raw_costume,
            false => 0,
        };

        let class = appearance_class(
            effects,
            equip.first().copied().unwrap_or(0),
            costume_id,
            costume_item,
        );

        let model = model_for_class(class);

        let variant = u32::from(VARIANT_CLASSES.contains(&class));

        let mut parts = [PartRef::EMPTY; PARTS];

        parts[0] = PartRef {
            mesh: face.mesh,
            tex: face.mesh_sub,
        };

        let helmet = worn(1);

        if !HELMET_SKIPPED
            .iter()
            .any(|&(lo, hi)| (lo..=hi).contains(&helmet))
            && !HELMET_SKIPPED_IDS.contains(&helmet)
        {
            parts[1] = part_of(helmet);
        }

        for (slot, part) in parts.iter_mut().enumerate().take(6).skip(2) {
            *part = part_of(worn(slot));
        }

        parts[7] = part_of(worn(6));

        parts[6] = part_of(worn(7));

        let mut part_color = [0u8; PARTS];

        let mut part_tier = [0u8; PARTS];

        for slot in 0..=5 {
            part_color[slot] = colour_of(slot);

            part_tier[slot] = tier_of(slot);
        }

        part_color[7] = colour_of(6);

        part_tier[7] = tier_of(6);

        part_color[6] = colour_of(7);

        part_tier[6] = tier_of(7);

        let mut part_tint = [0u8; PARTS];

        for (part, slot) in TINT_SLOTS {
            part_tint[part] =
                glow::tint_from_packet(part_color[part], refine.get(slot).copied().unwrap_or(0));
        }

        for (part, slot) in TINT_SLOTS {
            part_tier[part] = glow::tier_from_packet(
                part_tier[part],
                part_color[part],
                refine.get(slot).copied().unwrap_or(0),
                slot >= 6,
            );
        }

        let cape_item = worn(15);

        let wears_cape = equip.get(15).copied().unwrap_or(0) != 0;

        let mut cape_mesh = part_of(cape_item).tex;

        let side = match wears_cape {
            true => cape_side(cape_mesh),
            false => 0,
        };

        let off_hand = account::Item {
            index: equip.get(6).copied().unwrap_or(0),
            ..Default::default()
        };

        if effects.total_scaled(&off_hand, WEAPON_CLASS_EFFECT as u8) == 0x29 {
            parts[6] = parts[7];

            part_color[6] = part_color[7];

            part_tier[6] = part_tier[7];
        }

        let helmet_colors = part_color;

        let helmet_tiers = part_tier;

        let mut model = model;

        let (wears_costume, costume_body) = apply_dressing_overrides(
            items,
            Dressing {
                class,
                body,
                helmet,
                costume_id,
                cape_side: side,
            },
            &mut model,
            &mut cape_mesh,
            &mut parts,
            &mut part_color,
            &mut part_tier,
            &mut part_tint,
        );

        if flags & 4 != 0
            && let Some(face) = usize::try_from(body).ok().and_then(|id| items.get(id))
        {
            for part in 0..2 {
                if matches!(
                    i32::from(parts[part].mesh) + variant as i32 * 20,
                    47 | 48 | 51 | 67 | 68 | 71
                ) {
                    parts[part] = PartRef {
                        mesh: face.mesh,
                        tex: 0,
                    };

                    part_color[part] = helmet_colors[part];

                    part_tier[part] = helmet_tiers[part];
                }
            }
        }

        let cape = (wears_cape && matches!(costume_body, 0 | 100)).then(|| Cape {
            item: cape_item,
            mesh: cape_mesh,
            rank: cape_rank(level),
            refine: colour_of(15),
            tier: usize::try_from(cape_item)
                .ok()
                .and_then(|id| items.get(id))
                .map_or(0, |e| item_rank(&e)),
            side,
        });

        Some(Look {
            class,
            helmet_item: helmet,
            model,
            variant,
            parts,
            part_color,
            part_tier,
            part_tint,
            costume: wears_costume.then_some(costume_id),
            costume_body,
            kingdom: side,
            cape,
            mount: mount_for_spawn(equip, effect, refine, flags, effects, visuals),
            speed: 0.0,
            action: 0,
        })
    }

    pub fn for_class_preview(row: &[i32; CLASS_PREVIEW_COLUMNS], items: &ItemList) -> Option<Look> {
        let part_of = |id: i32| -> PartRef {
            match usize::try_from(id).ok().and_then(|id| items.get(id)) {
                Some(entry) => PartRef {
                    mesh: entry.mesh,
                    tex: entry.mesh_sub,
                },
                None => PartRef::EMPTY,
            }
        };

        let tier_of = |id: i32| -> u8 {
            usize::try_from(id)
                .ok()
                .and_then(|id| items.get(id))
                .map_or(0, |e| item_rank(&e))
        };

        let [face_id, helmet, armour, cape_id, left, right, colour] = *row;

        let face = items.get(usize::try_from(face_id).ok()?)?;

        let class = face.effect_sum(APPEARANCE_EFFECT);

        let model = model_for_class(class);

        let variant = u32::from(VARIANT_CLASSES.contains(&class));

        let mut parts = [PartRef::EMPTY; PARTS];

        parts[0] = PartRef {
            mesh: face.mesh,
            tex: face.mesh_sub,
        };

        parts[1] = part_of(helmet);

        parts[2..=5].fill(part_of(armour));

        parts[6] = part_of(left);

        parts[7] = part_of(right);

        let mut part_color = [colour as u8; PARTS];

        part_color[0] = 0;

        let mut part_tier = [tier_of(armour); PARTS];

        part_tier[0] = 0;

        part_tier[6] = tier_of(left);

        part_tier[7] = tier_of(right);

        let side = u8::from(cape_id > 0);

        let mut cape_mesh = part_of(cape_id).tex;

        let mut model = model;

        apply_dressing_overrides(
            items,
            Dressing {
                class,
                body: 0,
                helmet: 0,
                costume_id: 0,
                cape_side: side,
            },
            &mut model,
            &mut cape_mesh,
            &mut parts,
            &mut part_color,
            &mut part_tier,
            &mut [0; PARTS],
        );

        let cape = (cape_id > 0).then(|| Cape {
            item: cape_id,
            mesh: cape_mesh,
            rank: colour as u8,
            refine: 0,
            tier: tier_of(cape_id),
            side,
        });

        Some(Look {
            class,
            helmet_item: 0,
            model,
            variant,
            parts,
            part_color,
            part_tier,
            part_tint: [0; PARTS],
            costume: None,
            costume_body: 0,
            kingdom: side,
            cape,
            mount: None,
            speed: 0.0,
            action: 0,
        })
    }

    pub fn substitutes_part_names(&self) -> bool {
        self.costume_body != 0 && self.costume_body != costume::CAPE_BODY
    }
}

pub const GM_BODY_IDS: [i32; 5] = [0x16, 0x17, 0x18, 0x19, 0x20];

pub const GM_BODY_SUBSTITUTE: i32 = 0;

pub const HELMET_SKIPPED: [(i32, i32); 1] = [(0xDAC, 0xDAE)];

pub const HELMET_SKIPPED_IDS: [i32; 5] = [0xDB3, 0xDB5, 0xDB6, 0xDB7, 0xDB8];

pub const COLOURLESS_BODY_IDS: [i32; 6] = [0x15, 0x16, 0x17, 0x18, 0x19, 0x20];

pub const CLASS_PREVIEW_COLUMNS: usize = 7;
