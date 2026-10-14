use crate::net::account;

pub const PART_CLAMP: u8 = 15;

pub const GLOW_CLAMP: u8 = 12;

pub const DOUBLE_AT: u8 = 7;

pub const MULTI_LIMIT: u8 = 13;

pub const MOUNT_KIND_LIMIT: u8 = 12;

pub const SCROLL_PERIOD_MS: u32 = 10_000;

pub mod base {

    pub const CLASS4: u32 = 0xB3;

    pub const BLUE: u32 = 0x99;

    pub const SKY: u32 = 0xA6;

    pub const PINK: u32 = 0xF6;

    pub const RED: u32 = 0x104;

    pub const BRAVE: u32 = 0x1B8;

    pub const MULTI: u32 = 0xE9;

    pub const MULTI_OVER: u32 = 0x1C8;

    pub const MOUNT_MULTI: u32 = 0x1B8;

    pub const MOUNT_MULTI_OVER: u32 = 0x23C;

    pub const MOUNT_KIND: u32 = 0x200;

    pub const MOUNT_KIND_OVER: u32 = 0x206;

    pub const DRAGON: [u32; 4] = [0x20B, 0x217, 0x223, 0x22F];

    pub const TINT: [u32; 10] = [
        0x113, 0x120, 0x12D, 0x13A, 0x147, 0x154, 0x161, 0x16E, 0x1A9, 0x188,
    ];
}

pub const DRAGON_TEXTURES: core::ops::RangeInclusive<i32> = 0x828..=0x83B;

pub const TINT_FIRST: u8 = 0x74;

pub const TINT_LAST: u8 = 0x7D;

pub const TINT_WINDOW: core::ops::RangeInclusive<u8> = 0x73..=0x7E;

pub const TINT_FACTOR: u32 = 0xFFFF_8C78;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Uv {
    Sphere,
    Mesh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Blend {
    Modulate,
    AddSmooth,
    ModulateAlphaAddColor,
    DotProduct3,
    AddSigned,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Glow {
    pub texture: u32,
    pub uv: Uv,
    pub blend: Blend,
    pub double: bool,
    pub emissive: bool,
}

pub const EMISSIVE_MATERIAL: (f32, f32, f32) = (0.1, 0.55, 0.9);

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Part {
    pub texture: i32,
    pub color_key: bool,
    pub costume: bool,
    pub mmulti: bool,
    pub mount_kind: u8,
}

pub fn for_part(refine: u8, class: u8, part: Part) -> Option<Glow> {
    if refine < 1 && !(4..=8).contains(&class) {
        return None;
    }

    let frame = |base: u32| base + u32::from(refine.min(GLOW_CLAMP));

    if class == 4 {
        return Some(Glow {
            texture: frame(base::CLASS4),
            uv: Uv::Sphere,
            blend: Blend::Modulate,
            double: true,
            emissive: false,
        });
    }

    if (TINT_FIRST..=TINT_LAST).contains(&class) && refine >= 1 {
        let index = usize::from(class - TINT_FIRST);

        return Some(Glow {
            texture: base::TINT[index] + u32::from(refine.min(GLOW_CLAMP)) - 1,
            uv: Uv::Mesh,
            blend: match part.costume {
                true => Blend::AddSigned,
                false => Blend::DotProduct3,
            },
            double: false,
            emissive: true,
        });
    }

    let base = match class {
        5 => base::BLUE,
        6 => base::SKY,
        7 => base::PINK,
        8 => base::RED,
        9..=12 if DRAGON_TEXTURES.contains(&part.texture) => base::DRAGON[usize::from(class - 9)],
        9 => base::BLUE,
        10 => base::SKY,
        11 => base::PINK,
        12 => base::BRAVE,
        _ => {
            return Some(Glow {
                texture: match (part.mmulti, part.mount_kind) {
                    (false, _) => match refine < MULTI_LIMIT {
                        true => base::MULTI + u32::from(refine),
                        false => base::MULTI_OVER,
                    },
                    (true, 0) => match refine < MULTI_LIMIT {
                        true => base::MOUNT_MULTI + u32::from(refine),
                        false => base::MOUNT_MULTI_OVER,
                    },
                    (true, _) => match refine < MOUNT_KIND_LIMIT {
                        true => base::MOUNT_KIND + u32::from(refine / 2),
                        false => base::MOUNT_KIND_OVER,
                    },
                },
                uv: Uv::Sphere,
                blend: stage_one(part.color_key),
                double: refine >= DOUBLE_AT || (part.mmulti && part.mount_kind != 0),
                emissive: false,
            });
        }
    };

    Some(Glow {
        texture: frame(base),
        uv: Uv::Sphere,
        blend: stage_one(part.color_key),
        double: refine >= DOUBLE_AT,
        emissive: false,
    })
}

fn stage_one(color_key: bool) -> Blend {
    match color_key {
        true => Blend::ModulateAlphaAddColor,
        false => Blend::AddSmooth,
    }
}

pub fn effect_class(tier: u8, tint: u8) -> u8 {
    match (TINT_FIRST..=TINT_LAST).contains(&tint) {
        true => tint,
        false => tier.min(PART_CLAMP),
    }
}

pub fn tint(item: &account::Item) -> u8 {
    let ids = [item.effects[0].id, item.effects[1].id, item.effects[2].id];

    if ids.contains(&b'+') {
        return b'+';
    }

    ids.into_iter()
        .find(|id| TINT_WINDOW.contains(id))
        .unwrap_or(0)
}

pub fn tint_from_packet(level: u8, packed: u8) -> u8 {
    if level < 10 {
        return packed;
    }

    match packed & 0x0F {
        0 => 0,
        nibble => nibble + 0x73,
    }
}

pub fn tier_from_packet(tier: u8, level: u8, packed: u8, hand: bool) -> u8 {
    let high = packed >> 4;

    if tier >= 5 || level < 10 {
        return tier;
    }

    match hand || high != 0 {
        true => high + 8,
        false => tier,
    }
}

pub const TIER_PROMOTION_DEAD_ARM: u8 = 4;
