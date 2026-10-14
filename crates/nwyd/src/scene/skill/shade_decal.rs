use crate::scene::effect_tree::Effect;
use crate::scene::effects::SECTOR_UNASSIGNED;
use crate::scene::lod::DetailGate;
use crate::scene::shade::{Ground, Shade};
use crate::scene::vfx::{Ambient, Cx, Declined, Recipe};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Anchor, Axis, PositionRef, Spawn, SpawnArgument};
use glam::Vec3;
use render::billboard::BillboardPass;

pub const SPAWN_ARGUMENT_COUNT: usize = 4;

pub const CTOR_GRID: usize = 1;

pub const CTOR_TEXTURE: usize = 2;

pub const CTOR_RADIUS: usize = 3;

pub const PLACE_SLOT: u16 = 0x3C;

pub const PLACE_ARGS: usize = 2;

pub const COLOUR_SLOT: u16 = 0x44;

pub const COLOUR_ARGS: usize = 1;

pub const ADDITIVE_OFFSET: u16 = 0x6C;

pub const LIFETIME_OFFSET: u16 = 0xAC;

pub const CONSTRUCTOR_ARGB: u32 = 0xFFFF_FFFF;

pub const SPAWN_DETAIL: DetailGate = DetailGate::Always;

pub const DRAW_GATE: DetailGate = DetailGate::NotReduced;

pub const SWEEP_ANCHORS: [(Anchor, f32, f32); 4] = [
    (Anchor::Caster, 1000.0, 3000.0),
    (Anchor::Target, 4000.0, 6000.0),
    (Anchor::TargetCell, 7000.0, 9000.0),
    (Anchor::Midpoint, 2500.0, 4500.0),
];

pub const SEED_TOLERANCE: f32 = 16.0;

pub fn as_f32(arg: SpawnArgument) -> Option<f32> {
    match arg {
        SpawnArgument::Float(f) => Some(f),
        SpawnArgument::Bits(b) => Some(f32::from_bits(b)),
        SpawnArgument::Int(n) => Some(f32::from_bits(n as u32)),
        _ => None,
    }
}

pub fn as_u32(arg: SpawnArgument) -> Option<u32> {
    match arg {
        SpawnArgument::Bits(b) => Some(b),
        SpawnArgument::Int(n) => Some(n as u32),
        _ => None,
    }
}

pub fn anchor_of(x: f32, z: f32) -> Option<(Anchor, f32, f32)> {
    SWEEP_ANCHORS
        .iter()
        .find(|(_, sx, sz)| (x - sx).abs() <= SEED_TOLERANCE && (z - sz).abs() <= SEED_TOLERANCE)
        .map(|(anchor, sx, sz)| (*anchor, x - sx, z - sz))
}

pub fn placement(spawn: &Spawn) -> Option<PositionRef> {
    if spawn.class != SkillClass::Shade {
        return None;
    }

    let call = spawn.commands.iter().rev().find(|c| c.slot == PLACE_SLOT)?;

    if call.args.len() != PLACE_ARGS {
        return None;
    }

    match (call.args[0], call.args[1]) {
        (SpawnArgument::Pos(Axis::X), SpawnArgument::Pos(Axis::Z)) => return Some(spawn.at),
        (SpawnArgument::Toward(Axis::X), SpawnArgument::Toward(Axis::Z)) => {
            return Some(spawn.toward);
        }
        _ => {}
    }

    let (x, z) = (as_f32(call.args[0])?, as_f32(call.args[1])?);

    let (anchor, dx, dz) = anchor_of(x, z)?;

    Some(PositionRef::Offset(anchor, [dx, 0.0, dz]))
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Decal {
    pub grid: i32,
    pub texture: usize,
    pub radius: f32,
    pub argb: u32,
    pub additive: bool,
    pub lifetime_ms: u32,
    pub at: PositionRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Unmodelled {
    Patch { offset: u16, bytes: u32 },
    Call { slot: u16, args: usize },
}

pub fn read(spawn: &Spawn) -> Option<(Decal, Vec<Unmodelled>)> {
    if spawn.class != SkillClass::Shade || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    let grid = as_u32(spawn.arguments[CTOR_GRID])? as i32;

    let texture = as_u32(spawn.arguments[CTOR_TEXTURE])? as usize;

    let radius = as_f32(spawn.arguments[CTOR_RADIUS])?;

    let mut unmodelled = Vec::new();

    let (mut additive, mut lifetime_ms) = (false, 0u32);

    for patch in spawn.property_overrides {
        match patch.offset {
            ADDITIVE_OFFSET => additive = patch.bytes != 0,
            LIFETIME_OFFSET => lifetime_ms = patch.bytes,
            offset => unmodelled.push(Unmodelled::Patch {
                offset,
                bytes: patch.bytes,
            }),
        }
    }

    let mut argb = CONSTRUCTOR_ARGB;

    for call in spawn.commands {
        match call.slot {
            COLOUR_SLOT if call.args.len() == COLOUR_ARGS => argb = as_u32(call.args[0])?,
            COLOUR_SLOT => return None,
            PLACE_SLOT => {}
            slot => unmodelled.push(Unmodelled::Call {
                slot,
                args: call.args.len(),
            }),
        }
    }

    let at = placement(spawn)?;

    Some((
        Decal {
            grid,
            texture,
            radius,
            argb,
            additive,
            lifetime_ms,
            at,
        },
        unmodelled,
    ))
}

#[derive(Debug, Clone, PartialEq)]

pub struct Built {
    pub shade: Shade,
    pub placed: bool,
    pub unmodelled: Vec<Unmodelled>,
}

impl Built {
    pub fn push_into(self, out: &mut Vec<Box<dyn Effect>>) {
        out.push(Box::new(self.shade));
    }
}

pub fn build(decal: &Decal, at: Vec3, now_ms: u32, ground: Option<&Ground<'_>>) -> Built {
    let mut shade = Shade::new(decal.grid, decal.texture, decal.argb, at, SECTOR_UNASSIGNED)
        .with_lifetime(decal.lifetime_ms);

    shade.radius = decal.radius;

    shade.pass = BillboardPass::from_additive(decal.additive);

    shade.birth_ms = Some(now_ms);

    let placed = match ground {
        Some(ground) => {
            shade.place(ground);

            true
        }
        None => false,
    };

    Built {
        shade,
        placed,
        unmodelled: Vec::new(),
    }
}

pub const RECIPE: Recipe = Recipe {
    class: SkillClass::Shade,
    build: build_spawn,
};

pub fn build_spawn(
    spawn: &Spawn,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let Some((decal, unmodelled)) = read(spawn) else {
        return Err(Declined::Arguments);
    };

    let point = if let Some(context) = env.construction.as_ref().filter(|context| {
        matches!(
            context.producer,
            Some((formats::vfx::Chain::Caster, 0x48 | 0x50))
        )
    }) {
        context.target.ok_or(Declined::MissingContext)?.position
    } else {
        cx.at.at(decal.at).ok_or(Declined::Unplaced)?
    };

    let at = Vec3::new(point.x, 0.0, point.z);

    let mut built = build(&decal, at, cx.now_ms, env.ground);

    built.unmodelled = unmodelled;

    built.push_into(out);

    Ok(())
}

pub fn describe(built: &Built) -> String {
    let placed = match built.placed {
        true => "",
        false => " (no terrain — terrain placement did not run)",
    };

    let mut line = format!(
        "NWYD: Shade grid {} texture {:#04X} — {} vertices, life {} ms, argb {:08X}{placed}",
        built.shade.grid,
        built.shade.texture,
        built.shade.vertices.len(),
        built.shade.lifetime_ms,
        built.shade.colour,
    );

    for skipped in &built.unmodelled {
        match skipped {
            Unmodelled::Patch { offset, bytes } => {
                line.push_str(&format!("; unmodelled store +{offset:#X} = {bytes:#010X}"));
            }
            Unmodelled::Call { slot, args } => {
                line.push_str(&format!(
                    "; unsupported callback {} ({args} arg(s))",
                    slot / 4
                ));
            }
        }
    }

    line
}
