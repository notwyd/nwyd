use crate::scene::effect_tree::Effect;
use crate::scene::skinfx::{Blend, ColourAnim, Motion, SkinMeshEffect};
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Axis, Body, PositionRef, Spawn, SpawnArgument};
use glam::Vec3;

pub const SPAWN_ARGUMENT_COUNT: usize = 10;

pub const OBJECT_SIZE: u16 = 436;

pub const BUILD_SKINS_SLOT: u16 = 128;

pub const PLACE_SLOT: u16 = 60;

pub const STAGGER_BASE_MS: u32 = 100_000;

pub const STAGGER_MS: u32 = 100;

pub mod field {

    pub const SCALE: u16 = 0x38;

    pub const START: u16 = 0x118;

    pub const LIFETIME: u16 = 0x11C;

    pub const BLEND: u16 = 0x120;

    pub const MOTION: u16 = 0x124;

    pub const SPIN: u16 = 0x128;

    pub const COLOUR_A: u16 = 0x12C;

    pub const COLOUR_ANIM: u16 = 0x15C;

    pub const YAW: u16 = 0x17C;

    pub const APPEARANCE: std::ops::RangeInclusive<u16> = 0xE8..=0x104;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Unmodelled {
    Appearance { offset: u16, bytes: u32 },
    Patch { offset: u16, bytes: u32 },
}

#[derive(Debug, Clone, PartialEq)]

pub struct Order {
    pub mesh_id: i32,
    pub source: PositionRef,
    pub dest: PositionRef,
    pub variant: i32,
    pub owner: Option<Body>,
    pub placed: Option<PositionRef>,
    pub stagger_ms: u32,
    pub lifetime_ms: Option<u32>,
    pub blend: Option<Blend>,
    pub motion: Option<Motion>,
    pub spin: Option<f32>,
    pub colour_a: Option<[f32; 3]>,
    pub colour_anim: Option<ColourAnim>,
    pub yaw: Option<f32>,
    pub scale: Option<f32>,
    pub unmodelled: Vec<Unmodelled>,
}

pub fn start_ms(recorded: u32) -> u32 {
    recorded.saturating_sub(STAGGER_BASE_MS)
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::EffectSkinMesh || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    let bits = |arg: SpawnArgument| match arg {
        SpawnArgument::Bits(v) => Some(v),
        _ => None,
    };

    let axes = [Axis::X, Axis::Y, Axis::Z];

    let tagged_src = axes
        .iter()
        .enumerate()
        .all(|(i, a)| spawn.arguments[2 + i] == SpawnArgument::Pos(*a));

    let tagged_dst = axes
        .iter()
        .enumerate()
        .all(|(i, a)| spawn.arguments[5 + i] == SpawnArgument::Toward(*a));

    let literal = (2..8).all(|i| matches!(spawn.arguments[i], SpawnArgument::Bits(0)));

    if !(tagged_src && tagged_dst) && !literal {
        return None;
    }

    let mut order = Order {
        mesh_id: bits(spawn.arguments[1])? as i32,
        source: spawn.at,
        dest: spawn.toward,
        variant: bits(spawn.arguments[8])? as i32,
        owner: match spawn.arguments[9] {
            SpawnArgument::Body(body) => Some(body),
            SpawnArgument::Bits(0) => None,
            _ => return None,
        },
        placed: None,
        stagger_ms: 0,
        lifetime_ms: None,
        blend: None,
        motion: None,
        spin: None,
        colour_a: None,
        colour_anim: None,
        yaw: None,
        scale: None,
        unmodelled: Vec::new(),
    };

    let mut colour = [None; 3];

    for patch in spawn.property_overrides {
        let (offset, value) = (patch.offset, patch.bytes);

        if field::APPEARANCE.contains(&offset) {
            order.unmodelled.push(Unmodelled::Appearance {
                offset,
                bytes: value,
            });

            continue;
        }

        match offset {
            field::SCALE => order.scale = Some(f32::from_bits(value)),
            field::START => order.stagger_ms = start_ms(value),
            field::LIFETIME => order.lifetime_ms = Some(value),
            field::BLEND => order.blend = Blend::of(value as i32),
            field::MOTION => order.motion = Some(Motion::of(value as i32)),
            field::SPIN => order.spin = Some(f32::from_bits(value)),
            field::COLOUR_ANIM => order.colour_anim = Some(ColourAnim::of(value as i32)),
            field::YAW => order.yaw = Some(f32::from_bits(value)),
            o if (field::COLOUR_A..field::COLOUR_A + 12).contains(&o)
                && (o - field::COLOUR_A).is_multiple_of(4) =>
            {
                colour[usize::from((o - field::COLOUR_A) / 4)] = Some(f32::from_bits(value));
            }
            _ => order.unmodelled.push(Unmodelled::Patch {
                offset,
                bytes: value,
            }),
        }
    }

    if let [Some(r), Some(g), Some(b)] = colour {
        order.colour_a = Some([r, g, b]);
    }

    for call in spawn.commands {
        if call.slot == PLACE_SLOT {
            if call.args.len() != 3 {
                return None;
            }

            order.placed = Some(call.at?);
        }
    }

    Some(order)
}

pub fn build_spawn(
    spawn: &Spawn,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let _ = out;

    let Some(order) = read(spawn) else {
        return Err(Declined::Arguments);
    };

    let (source, dest) = match (order.source, order.dest) {
        (PositionRef::Unresolved, PositionRef::Unresolved) => (Vec3::ZERO, Vec3::ZERO),
        _ => match (cx.at.at(order.source), cx.at.at(order.dest)) {
            (Some(a), Some(b)) => (a, b),
            _ => return Err(Declined::Unplaced),
        },
    };

    let placed = match order.placed {
        Some(at) => {
            let body = env.construction.as_ref().ok_or(Declined::MissingContext)?;

            let live = crate::scene::vfx::Anchors {
                caster: body.caster.position,
                target: body.target.map(|t| t.position),
                target_cell: cx.at.target_cell,
            };

            Some(
                live.at(at)
                    .filter(|p| p.is_finite())
                    .ok_or(Declined::Unplaced)?,
            )
        }
        None => None,
    };

    let mut effect = build(&order, source, dest, placed, cx.now_ms);

    let context = env.construction.as_ref().ok_or(Declined::MissingContext)?;

    crate::scene::skin_inputs::bind(context, &order, &mut effect)?;

    env.skins.push(effect);

    Ok(())
}

pub fn build(
    order: &Order,
    source: Vec3,
    dest: Vec3,
    placed: Option<Vec3>,
    now_ms: u32,
) -> SkinMeshEffect {
    let mut effect = SkinMeshEffect::new(order.mesh_id, source, dest, order.variant);

    effect.start_ms = Some(now_ms.wrapping_add(order.stagger_ms));

    if let Some(ms) = order.lifetime_ms {
        effect.lifetime_ms = ms;
    }

    if let Some(blend) = order.blend {
        effect.blend = blend;
    }

    if let Some(motion) = order.motion {
        effect.motion = motion;
    }

    if let Some(spin) = order.spin {
        effect.spin = spin;
    }

    if let Some(colour) = order.colour_a {
        effect.colour_a = colour;
    }

    if let Some(anim) = order.colour_anim {
        effect.colour_anim = anim;
    }

    if let Some(yaw) = order.yaw {
        effect.yaw = yaw;
    }

    if let Some(scale) = order.scale {
        effect.scale = scale;
    }

    if let Some(point) = placed {
        effect.place(point);
    }

    effect
}

pub fn describe(effect: &SkinMeshEffect, order: &Order) -> String {
    let place = match order.placed {
        Some(p) => format!("terrain placement {p:?}"),
        None => format!("{:?} -> {:?}", order.source, order.dest),
    };

    let owner = match order.owner {
        Some(body) => format!(", owner {body:?}"),
        None => String::new(),
    };

    let appearance = order
        .unmodelled
        .iter()
        .filter(|u| matches!(u, Unmodelled::Appearance { .. }))
        .count();

    let hydration = if effect.rig.is_some() {
        "rig hydrated"
    } else {
        "rig hydration pending"
    };

    format!(
        "NWYD: EffectSkinMesh mesh {:#X} {place}{owner} — {:?}, {:?}, life {} ms, +{} ms\
         , {appearance} captured appearance dword(s); {hydration}",
        effect.mesh_id, effect.motion, effect.blend, effect.lifetime_ms, order.stagger_ms,
    )
}
