use crate::scene::effect_mesh::{EffectMesh, UvScroll};
use crate::scene::effect_tree::Effect;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{PositionRef, PropertyOverride, Spawn, SpawnArgument};
use glam::Vec3;

pub const SPAWN_ARGUMENT_COUNT: usize = 5;

pub const OBJECT_SIZE: u16 = 216;

pub mod field {

    pub const BLEND: u32 = 0x6C;

    pub const POSITION: u32 = 0x74;

    pub const SCALE_XZ: u32 = 0xA4;

    pub const SCALE_Y: u32 = 0xA8;

    pub const TEXTURE_BIAS: u32 = 0xB0;

    pub const FLAGS: u32 = 0xB8;

    pub const LIFETIME: u32 = 0xBC;

    pub const PERIOD: u32 = 0xC4;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Unmodelled {
    pub offset: u32,
    pub bytes: u32,
}

#[derive(Debug, Clone, PartialEq)]

pub struct Order {
    pub at: PositionRef,
    pub mesh_id: i32,
    pub argb: u32,
    pub yaw: f32,
    pub mode: i32,
    pub scale_xz: Option<f32>,
    pub scale_y: Option<f32>,
    pub texture_bias: Option<i32>,
    pub lifetime_ms: Option<u32>,
    pub period_ms: Option<u32>,
    pub pulse: Option<bool>,
    pub uv_scroll: Option<UvScroll>,
    pub unmodelled: Vec<Unmodelled>,
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::EffectMesh || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    let bits = |arg: SpawnArgument| match arg {
        SpawnArgument::Bits(v) => Some(v),
        SpawnArgument::Int(v) => Some(v as u32),
        _ => None,
    };

    let mut order = Order {
        at: spawn.at,
        mesh_id: bits(spawn.arguments[1])? as i32,
        argb: bits(spawn.arguments[2])?,
        yaw: f32::from_bits(bits(spawn.arguments[3])?),
        mode: bits(spawn.arguments[4])? as i32,
        scale_xz: None,
        scale_y: None,
        texture_bias: None,
        lifetime_ms: None,
        period_ms: None,
        pulse: None,
        uv_scroll: None,
        unmodelled: Vec::new(),
    };

    for PropertyOverride { offset, bytes, .. } in spawn.property_overrides.iter() {
        match u32::from(*offset) {
            field::SCALE_XZ => order.scale_xz = Some(f32::from_bits(*bytes)),
            field::SCALE_Y => order.scale_y = Some(f32::from_bits(*bytes)),
            field::TEXTURE_BIAS => order.texture_bias = Some(*bytes as i32),
            field::LIFETIME => order.lifetime_ms = Some(*bytes),
            field::PERIOD => order.period_ms = Some(*bytes),
            field::FLAGS => {
                if *bytes > 0xFFFF {
                    return None;
                }

                order.pulse = Some(bytes & 0xFF == 1);

                order.uv_scroll = Some(match bytes >> 8 {
                    1 => UvScroll::U,
                    2 => UvScroll::V,
                    _ => UvScroll::None,
                });
            }
            field::BLEND => {}
            o if (field::POSITION..field::POSITION + 12).contains(&o) => {}
            offset => order.unmodelled.push(Unmodelled {
                offset,
                bytes: *bytes,
            }),
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
    let Some(mut order) = read(spawn) else {
        return Err(Declined::Arguments);
    };

    let at = if let Some(context) = env.construction.as_ref().filter(|context| {
        matches!(
            context.producer,
            Some((formats::vfx::Chain::Caster, 0x0D | 0x0F))
        )
    }) {
        order.yaw = context.caster.yaw;

        caster_hit_position(&context.caster).ok_or(Declined::MissingContext)?
    } else if env
        .construction
        .as_ref()
        .is_some_and(|context| context.producer == Some((formats::vfx::Chain::Caster, 0x4C)))
    {
        env.construction
            .as_ref()
            .and_then(|context| context.caster.skill_anchor)
            .map(|point| point + Vec3::Y * 1.1)
            .filter(|point| point.is_finite())
            .ok_or(Declined::MissingContext)?
    } else {
        cx.at.at(order.at).ok_or(Declined::Unplaced)?
    };

    out.push(Box::new(build(&order, at, cx.now_ms)));

    Ok(())
}

fn caster_hit_position(caster: &crate::scene::effect_context::Snapshot) -> Option<Vec3> {
    let (_, height) = caster.dimensions?;

    let mounted = caster.appearance?.mounted;

    let point = if mounted {
        caster.skill_anchor?
    } else {
        caster.position
    };

    let at = point + Vec3::Y * (height * caster.scale * 0.5 + if mounted { -0.3 } else { 0.3 });

    at.is_finite().then_some(at)
}

pub fn build(order: &Order, at: Vec3, now_ms: u32) -> EffectMesh {
    let mut mesh = EffectMesh::new(order.mesh_id, order.argb, order.yaw, order.mode, now_ms);

    mesh.position = at;

    if let Some(s) = order.scale_xz {
        mesh.scale_xz = s;
    }

    if let Some(s) = order.scale_y {
        mesh.scale_y = s;
    }

    if let Some(b) = order.texture_bias {
        mesh.texture_bias = b;
    }

    if let Some(ms) = order.lifetime_ms {
        mesh.lifetime_ms = ms;
    }

    if let Some(ms) = order.period_ms {
        mesh.period_ms = ms;
    }

    if let Some(pulse) = order.pulse {
        mesh.pulse = pulse;
    }

    if let Some(scroll) = order.uv_scroll {
        mesh.uv_scroll = scroll;
    }

    mesh
}
