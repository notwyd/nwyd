use crate::scene::effect_tree::Effect;
use crate::scene::effects::{Billboard, Motion};
use crate::scene::vfx::{Ambient, Cx, Declined, Recipe};
use formats::vfx::dispatch::{self, Class as SkillClass};
use formats::vfx::{Axis, EffectCommand, PropertyOverride, Spawn, SpawnArgument};
use glam::Vec3;
use render::billboard::BillboardPass;

pub const SPAWN_ARGUMENT_COUNT: usize = 9;

pub const OBJECT_BYTES: u16 = 0x190;

pub const COLOUR_SLOT: u16 = 0x3C;

pub const COLOUR_ARGS: usize = 1;

pub const CONSTRUCTOR_COLOUR: u32 = 0xFFFF_FFFF;

pub const UV_SWAP_KIND: usize = 0x21;

pub const POSITION: [u16; 3] = [0x74, 0x78, 0x7C];

pub const ORIGIN: [u16; 3] = [0x158, 0x15C, 0x160];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Store {
    Pass,
    Position(Axis),
    Lifetime,
    Lift,
    ColourMode,
    Roll,
    AmpH,
    Rise,
    Motion,
    Origin(Axis),
}

impl Store {
    pub fn at(offset: u16) -> Option<Store> {
        if let Some(i) = POSITION.iter().position(|o| *o == offset) {
            return Some(Store::Position(AXES[i]));
        }

        if let Some(i) = ORIGIN.iter().position(|o| *o == offset) {
            return Some(Store::Origin(AXES[i]));
        }

        match offset {
            0x6C => Some(Store::Pass),
            0x104 => Some(Store::Lifetime),
            0x10C => Some(Store::Lift),
            0x110 => Some(Store::ColourMode),
            0x120 => Some(Store::Roll),
            0x148 => Some(Store::AmpH),
            0x14C => Some(Store::Rise),
            0x154 => Some(Store::Motion),
            _ => None,
        }
    }

    pub fn is_float(self) -> bool {
        matches!(self, Store::Roll | Store::AmpH | Store::Rise)
    }

    pub fn is_positional(self) -> bool {
        matches!(self, Store::Position(_) | Store::Origin(_))
    }
}

const AXES: [Axis; 3] = [Axis::X, Axis::Y, Axis::Z];

pub fn field_float(bytes: u32) -> Option<f32> {
    let value = f32::from_bits(bytes);

    value.is_finite().then_some(value)
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Args {
    pub kind: usize,
    pub lifetime_ms: u32,
    pub scale: Vec3,
    pub growth: f32,
    pub frames: u32,
    pub frame_ms: u32,
}

pub fn arguments(values: &[SpawnArgument]) -> Option<Args> {
    if values.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    if !matches!(values[0], SpawnArgument::Allocation(_)) {
        return None;
    }

    Some(Args {
        kind: usize::try_from(integer(values[1])?).ok()?,
        lifetime_ms: integer(values[2])?,
        scale: Vec3::new(float(values[3])?, float(values[4])?, float(values[5])?),
        growth: float(values[6])?,
        frames: integer(values[7])?,
        frame_ms: integer(values[8])?,
    })
}

fn integer(arg: SpawnArgument) -> Option<u32> {
    match arg {
        SpawnArgument::Int(n) => u32::try_from(n).ok(),
        SpawnArgument::Bits(n) => Some(n),
        _ => None,
    }
}

fn float(arg: SpawnArgument) -> Option<f32> {
    match arg {
        SpawnArgument::Float(f) => Some(f),
        SpawnArgument::Bits(n) => field_float(n),
        SpawnArgument::Int(0) => Some(0.0),
        _ => None,
    }
}

pub fn colour(call: &EffectCommand) -> Option<u32> {
    if call.slot != COLOUR_SLOT || call.args.len() != COLOUR_ARGS {
        return None;
    }

    match call.args[0] {
        SpawnArgument::Bits(argb) => Some(argb),
        SpawnArgument::Int(n) => Some(n as u32),
        _ => None,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct Unread {
    pub invalid_float: u32,
    pub unmodelled: u32,
    pub uncalled: u32,
}

impl Unread {
    pub fn is_empty(self) -> bool {
        self == Unread::default()
    }

    pub fn len(self) -> u32 {
        self.invalid_float + self.unmodelled + self.uncalled
    }
}

#[derive(Debug, Clone, PartialEq)]

pub struct Built {
    pub billboard: Billboard,
    pub unread: Unread,
}

pub fn build(
    args: &Args,
    at: Vec3,
    property_overrides: &[PropertyOverride],
    commands: &[EffectCommand],
) -> Result<Built, Declined> {
    let mut bb = Billboard::new(
        args.kind,
        args.lifetime_ms,
        args.scale,
        args.growth,
        args.frames,
        args.frame_ms,
    );

    let mut unread = Unread::default();

    let mut placed = [false; 3];

    for patch in property_overrides {
        let Some(store) = Store::at(patch.offset) else {
            unread.unmodelled += 1;

            continue;
        };

        if store.is_positional() {
            if patch.axis.is_none() {
                return Err(Declined::Unplaced);
            }

            let Some(axis) = patch.axis else {
                return Err(Declined::Unplaced);
            };

            match store {
                Store::Position(_) => {
                    placed[axis as usize] = true;

                    bb.position[axis as usize] = at[axis as usize];
                }
                Store::Origin(_) => bb.origin[axis as usize] = at[axis as usize],
                _ => unreachable!("is_positional names exactly these two"),
            }

            continue;
        }

        if store.is_float() {
            let Some(value) = field_float(patch.bytes) else {
                unread.invalid_float += 1;

                continue;
            };

            match store {
                Store::Roll => bb.roll = value,
                Store::AmpH => bb.amp_h = value,
                Store::Rise => bb.rise = value,
                _ => unreachable!("is_float names exactly these three"),
            }

            continue;
        }

        match store {
            Store::Pass => bb.pass = BillboardPass::from_additive(patch.bytes == 1),
            Store::Lifetime => bb.lifetime_ms = patch.bytes,
            Store::Lift => bb.lift = patch.bytes == 1,
            Store::ColourMode => bb.colour_mode = patch.bytes,
            Store::Motion => bb.motion = Motion::from_field(patch.bytes),
            _ => unreachable!("every remaining variant is one of these five"),
        }
    }

    if placed != [true; 3] {
        return Err(Declined::Unplaced);
    }

    for call in commands {
        match colour(call) {
            Some(argb) => bb.set_color(argb),
            None => unread.uncalled += 1,
        }
    }

    Ok(Built {
        billboard: bb,
        unread,
    })
}

pub const RECIPE: Recipe = Recipe {
    class: SkillClass::EffectBillboard,
    build: build_spawn,
};

pub fn build_spawn(
    spawn: &Spawn,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let Some(args) = arguments(spawn.arguments) else {
        return Err(Declined::Arguments);
    };

    let at = if let Some(context) = env.construction.as_ref().filter(|context| {
        matches!(
            context.producer,
            Some((formats::vfx::Chain::Caster, 0x48 | 0x50))
        )
    }) {
        context.target.ok_or(Declined::MissingContext)?.position + Vec3::Y
    } else {
        cx.at.at(spawn.at).ok_or(Declined::Unplaced)?
    };

    out.push(Box::new(
        build(&args, at, spawn.property_overrides, spawn.commands)?.billboard,
    ));

    Ok(())
}

pub fn caster_pairs(
    caster: &crate::scene::effect_context::Snapshot,
    now_ms: u32,
    rng: &mut crate::scene::rng::Lcg,
) -> Vec<Billboard> {
    let mounted = if caster
        .appearance
        .is_some_and(|appearance| appearance.mounted)
    {
        1.3
    } else {
        1.0
    };

    let phase = (now_ms % 1000) as f32 * std::f32::consts::TAU / 1000.0;

    let mut out = Vec::with_capacity(4);

    for pair in 0..2 {
        let scale = ((rng.next() % 5) as f32 * 0.2 + 1.0) * mounted;

        for following in [false, true] {
            let size = if following {
                scale
            } else {
                scale - pair as f32 * 0.4
            };

            let mut sprite = Billboard::new(109, 2000, Vec3::splat(size), 0.0, 8, 80);

            sprite.set_color(0xFFFFDD00);

            sprite.pass = BillboardPass::Additive;

            sprite.colour_mode = u32::from(following);

            sprite.roll = phase + if following { 0.0 } else { pair as f32 };

            sprite.position = caster.position + Vec3::new(0.5, 1.8, 0.5);

            sprite.owner_key = following.then_some(caster.key);

            out.push(sprite);
        }
    }

    out
}

pub fn describe(built: &Built) -> String {
    let bb = &built.billboard;

    let unread = match built.unread.len() {
        0 => String::new(),
        n => format!(", {n} unsupported field(s)"),
    };

    format!(
        "NWYD: EffectBillboard kind {:#04X} — {} ms, scale ({}, {}, {}), {:?}, {:?}, \
         colour {:#010X}{unread}",
        bb.kind,
        bb.lifetime_ms,
        bb.scale.x,
        bb.scale.y,
        bb.scale.z,
        bb.motion,
        bb.pass,
        u32::from_be_bytes(bb.base),
    )
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct Census {
    pub spawns: usize,
    pub references: usize,
    pub built: usize,
    pub unplaced: usize,
    pub refused: usize,
    pub unread: Unread,
}

impl Census {
    pub fn report_lines(&self) -> Vec<String> {
        vec![format!(
            "NWYD: EffectBillboard — {} of {} table entr(ies) build \
             ({} reference(s) across the table), {} unplaced, {} refused; {} invalid float \
             store(s), {} unmodelled store(s), {} unsupported effect call(s)",
            self.built,
            self.spawns,
            self.references,
            self.unplaced,
            self.refused,
            self.unread.invalid_float,
            self.unread.unmodelled,
            self.unread.uncalled,
        )]
    }
}

pub fn census() -> Census {
    let mut census = Census::default();

    for spawn in dispatch::SPAWNS
        .iter()
        .filter(|s| s.class == SkillClass::EffectBillboard)
    {
        census.spawns += 1;

        let Some(args) = arguments(spawn.arguments) else {
            census.refused += 1;

            continue;
        };

        match build(&args, Vec3::ZERO, spawn.property_overrides, spawn.commands) {
            Err(Declined::Arguments) => census.refused += 1,
            Err(_) => census.unplaced += 1,
            Ok(built) => {
                census.built += 1;

                census.unread.invalid_float += built.unread.invalid_float;

                census.unread.unmodelled += built.unread.unmodelled;

                census.unread.uncalled += built.unread.uncalled;
            }
        }
    }

    census.references = dispatch::ROWS
        .iter()
        .flat_map(|row| row.spawns())
        .filter(|s| s.class == SkillClass::EffectBillboard)
        .count();

    census
}

pub fn report(census: &Census) {
    for line in census.report_lines() {
        tracing::info!("{line}");
    }
}
