use crate::scene::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use crate::scene::effects::{Billboard, SECTOR_UNASSIGNED};
use crate::scene::rng::Lcg;
use crate::scene::shade::{Ground, Shade};
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::{Axis, Spawn, SpawnArgument};
use glam::Vec3;
use render::billboard::BillboardPass;

pub const CLASS: Class = Class {
    name: "SkillThunderBolt",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const LIFETIME_MS: u32 = 2000;

pub const BOLT_TEXTURE: usize = 0x2D;

pub const BOLT_FRAMES: u32 = 6;

pub const BOLT_FRAME_MS: u32 = 0x50;

pub const BOLT_THIN: f32 = 0.55;

pub const BOLT_TALL: f32 = 10.0;

pub const BOLT_WIDE: f32 = 1.55;

pub const BOLT_DROP: f64 = 1.0;

pub const COLOURS: [u32; 9] = [
    0xFF77_AAAA,
    0xFF77_77AA,
    0xFF77_77FF,
    0xFFFF_AA00,
    0xFFFF_AA00,
    0xFFFF_AA00,
    0xFF00_FF77,
    0xFF00_FFAA,
    0xFF00_FFAA,
];

pub const MODES: i32 = 3;

pub const OUT_OF_RANGE_MODES: [i32; 2] = [3, 5];

pub const SHADE_GRID: i32 = 4;

pub const SHADE_TEXTURE: usize = 7;

pub const POISON_COUNT: i32 = 10;

pub const POISON_PLAY_SOUND: i32 = 1;

pub const POISON_STYLE: i32 = 0;

pub const SOUND: u16 = 0x98;

pub const fn colours(mode: i32) -> Option<[u32; 3]> {
    if mode < 0 || mode >= MODES {
        return None;
    }

    let base = (mode * 3) as usize;

    Some([COLOURS[base], COLOURS[base + 1], COLOURS[base + 2]])
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct PoisonChild {
    pub at: Vec3,
    pub argb: Option<u32>,
    pub count: i32,
    pub play_sound: i32,
    pub style: i32,
}

pub type PoisonBuild = fn(&PoisonChild, &Cx<'_>, &mut Lcg, &mut Vec<Box<dyn Effect>>);

pub const POISON: Option<PoisonBuild> = Some(|child, cx, rng, out| {
    use crate::scene::skill::poison;

    let recipe = poison::Recipe {
        at: child.at,
        argb: child.argb.unwrap_or(0xFFFF_FFFF),
        count: child.count,
        play_sound: child.play_sound,
        style: child.style,
    };

    poison::push(poison::build(&recipe, cx.now_ms, rng, cx.detail), out);
});

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct ThunderBolt {
    pub at: Vec3,
    pub mode: i32,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub gone: bool,
}

impl ThunderBolt {
    pub fn new(at: Vec3, mode: i32, now_ms: u32) -> ThunderBolt {
        ThunderBolt {
            at,
            mode,
            birth_ms: now_ms,
            lifetime_ms: LIFETIME_MS,
            gone: false,
        }
    }
}

impl Effect for ThunderBolt {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let elapsed = tick.now_ms.wrapping_sub(self.birth_ms);

        if elapsed as f32 / self.lifetime_ms as f32 > 1.0 {
            self.gone = true;
        }
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, _cx: &Submit<'_>, _out: &mut Sink<'_>) {}

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}

#[derive(Debug, Clone, PartialEq)]

pub struct Built {
    pub bolts: [Billboard; 2],
    pub poison: PoisonChild,
    pub shade: Shade,
    pub token: ThunderBolt,
    pub colours: Option<[u32; 3]>,
}

impl Built {
    pub fn place(&mut self, ground: &Ground<'_>) {
        self.shade.place(ground);
    }

    pub fn into_nodes(self, poison: Vec<Box<dyn Effect>>) -> Vec<Box<dyn Effect>> {
        let [first, second] = self.bolts;

        let mut nodes: Vec<Box<dyn Effect>> = vec![Box::new(first), Box::new(second)];

        nodes.extend(poison);

        nodes.push(Box::new(self.shade));

        nodes.push(Box::new(self.token));

        nodes
    }

    pub fn describe(&self) -> String {
        let colours = match self.colours {
            Some([a, b, c]) => format!("{a:#010X}/{b:#010X}/{c:#010X}"),
            None => String::from("none — mode out of range for the 9-entry table (D-354)"),
        };

        let poison = match POISON {
            Some(_) => "poison built",
            None => "poison unbuilt (poison effect)",
        };

        format!(
            "NWYD: SkillThunderBolt mode {} — 2 bolt(s), 1 shade, colours {colours}, \
             {poison}, sound {SOUND:#04X} (named, not played)",
            self.token.mode,
        )
    }
}

pub fn build_at(at: Vec3, mode: i32, now_ms: u32) -> Built {
    let colours = colours(mode);

    let dropped = Vec3::new(at.x, (f64::from(at.y) - BOLT_DROP) as f32, at.z);

    Built {
        bolts: [
            bolt(
                dropped,
                Vec3::new(BOLT_THIN, BOLT_TALL, BOLT_WIDE),
                colours.map(|c| c[0]),
            ),
            bolt(
                dropped,
                Vec3::new(BOLT_THIN, BOLT_TALL, BOLT_THIN),
                colours.map(|c| c[1]),
            ),
        ],
        poison: PoisonChild {
            at,
            argb: colours.map(|c| c[0]),
            count: POISON_COUNT,
            play_sound: POISON_PLAY_SOUND,
            style: POISON_STYLE,
        },
        shade: Shade::new(
            SHADE_GRID,
            SHADE_TEXTURE,
            colours.map_or(0xFFFF_FFFF, |c| c[2]),
            at,
            SECTOR_UNASSIGNED,
        )
        .with_lifetime(LIFETIME_MS),
        token: ThunderBolt::new(at, mode, now_ms),
        colours,
    }
}

fn bolt(at: Vec3, scale: Vec3, argb: Option<u32>) -> Billboard {
    let mut child = Billboard::new(
        BOLT_TEXTURE,
        LIFETIME_MS,
        scale,
        0.0,
        BOLT_FRAMES,
        BOLT_FRAME_MS,
    );

    child.position = at;

    child.origin = at;

    child.pass = BillboardPass::Additive;

    child.lift = true;

    child.colour_mode = 0;

    if let Some(argb) = argb {
        child.set_color(argb);
    }

    child
}

pub fn mode_of(spawn: &Spawn) -> Option<i32> {
    match spawn.arguments {
        [
            SpawnArgument::Allocation(_),
            SpawnArgument::Pos(Axis::X),
            SpawnArgument::Pos(Axis::Y),
            SpawnArgument::Pos(Axis::Z),
            mode,
        ] => match mode {
            SpawnArgument::Int(n) => Some(*n),
            SpawnArgument::Bits(b) => Some(*b as i32),
            _ => None,
        },
        _ => None,
    }
}

pub fn build_into(
    spawn: &Spawn,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let Some(at) = cx.at.at(spawn.at) else {
        return Err(Declined::Unplaced);
    };

    let Some(mode) = mode_of(spawn) else {
        return Err(Declined::Arguments);
    };

    let mut built = build_at(at, mode, cx.now_ms);

    if let Some(ground) = env.ground {
        built.place(ground);
    }

    let mut poison: Vec<Box<dyn Effect>> = Vec::new();

    if let Some(build_poison) = POISON {
        build_poison(&built.poison, cx, env.rng, &mut poison);
    }

    out.extend(built.into_nodes(poison));

    Ok(())
}
