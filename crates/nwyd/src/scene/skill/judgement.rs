use crate::scene::effect_mesh::EffectMesh;
use crate::scene::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use crate::scene::quads::{GroundQuad, Slot};
use crate::scene::timers::TimedQuad;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::{Spawn, SpawnArgument};
use glam::Vec3;
use render::billboard::BillboardPass;

pub const CLASS: Class = Class {
    name: "SkillJudgement",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const SPAWN_ARGUMENT_COUNT: usize = 6;

pub const LIFETIME_MS: u32 = 300;

pub const SOUND: u16 = 0x26;

pub const DISC_SCALE: f32 = 8.0;

pub const EXTRA_SCALE: f32 = 5.0;

pub const MESH_SCALE: f32 = 4.0;

pub const EXTRA_SPIN_MULTIPLIER: f64 = -1.5;

pub const LIFT_A: f32 = 0.31;

pub const LIFT_B: f32 = 0.3;

pub const LIFT_SIX_A: f32 = 0.51;

pub const LIFT_SIX_B: f32 = 0.61;

pub const EXTRA_TEXTURE: usize = 0x1A3;

pub const SIX_TEXTURE: usize = 0x1A2;

pub const SIX_LIFE_MS: u32 = 3000;

pub const SIX_SCALES: [f32; 2] = [4.0, 6.4];

pub const SIX_PERIODS: [i32; 2] = [1000, -2000];

pub const SIX_COLOURS: [u32; 2] = [0x4433_FF33, 0xFFAA_FFAA];

pub const MESH_KIND: i32 = 10;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Mode {
    pub scale: f32,
    pub period_ms: i32,
    pub life_ms: u32,
    pub col1: u32,
    pub col2: u32,
    pub texture: usize,
}

impl Mode {
    pub const DEFAULT: Mode = Mode {
        scale: 1.0,
        period_ms: 5000,
        life_ms: 3000,
        col1: 0x8833_3355,
        col2: 0x8855_5555,
        texture: 0x7C,
    };

    pub fn of(mode: i32, scale_multiplier: f32) -> Mode {
        let mut m = Mode::DEFAULT;

        match mode {
            1 => {
                m.scale = 0.5;

                m.col1 = 0x8833_5533;

                m.life_ms = 0;
            }
            2 => {
                m.period_ms = 0;

                m.col1 = 0x8833_33FF;

                m.scale = 0.7;

                m.col2 = 0x8855_55FF;

                m.life_ms = 1500;

                m.texture = 0x1A0;
            }
            3 => {
                m.period_ms = 600;

                m.col1 = 0x8833_33FF;

                m.scale = 0.7;

                m.col2 = 0xFFAA_AAAA;

                m.life_ms = 3000;

                m.texture = 0x1A2;
            }
            4 => {
                m.period_ms = 0;

                m.col1 = 0x8822_7777;

                m.scale = 0.3;

                m.col2 = 0xFF77_FFFF;

                m.life_ms = 3000;

                m.texture = 0x1A5;
            }
            5 => {
                m.period_ms = 8000;

                m.col1 = 0x6699_5533;

                m.scale = 8.0;

                m.life_ms = 10000;
            }
            SIX => {}
            7 => {
                m.scale = scale_multiplier * 0.7;

                m.period_ms = 600;

                m.col1 = 0xFFAA_AAAA;

                m.col2 = 0xFFAA_AAAA;

                m.life_ms = 1500;

                m.texture = 0x1A2;
            }
            8 => {
                m.scale = scale_multiplier * 0.3;

                m.period_ms = 500;

                m.col1 = 0x8822_7777;

                m.col2 = 0xFF77_FFFF;

                m.life_ms = 1500;

                m.texture = 0x1A5;
            }
            _ => {}
        }

        m
    }

    pub const fn has_extra(mode: i32) -> bool {
        matches!(mode, 3 | 7 | 8)
    }
}

pub const SIX: i32 = 6;

pub fn spin_period(mode: i32, period_ms: i32, scale_multiplier: f32) -> i32 {
    if !Mode::has_extra(mode) {
        return period_ms;
    }

    let turned = period_ms as f64 * EXTRA_SPIN_MULTIPLIER;

    let turned = match mode {
        7 | 8 => turned * scale_multiplier as f64,
        _ => turned,
    };

    turned as i32
}

#[derive(Debug, Clone, PartialEq)]

pub struct Judgement {
    pub at: Vec3,
    pub mode: i32,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub gone: bool,
    owned: Vec<TimedQuad>,
}

impl Judgement {
    pub fn new(at: Vec3, mode: i32, now_ms: u32) -> Judgement {
        Judgement {
            at,
            mode,
            birth_ms: now_ms,
            lifetime_ms: LIFETIME_MS,
            gone: false,
            owned: Vec::new(),
        }
    }

    pub fn held(&self) -> &[TimedQuad] {
        &self.owned
    }

    pub fn held_count(&self) -> usize {
        self.owned.len()
    }
}

impl Effect for Judgement {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        for child in &mut self.owned {
            child.update(tick);
        }

        self.owned.retain(|child| !child.expired());

        let elapsed = tick.now_ms.wrapping_sub(self.birth_ms) as f32;

        if elapsed / self.lifetime_ms as f32 > 1.0 {
            self.gone = true;
        }
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        for child in &self.owned {
            child.submit(cx, out);
        }
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}

#[derive(Debug, Clone, PartialEq)]

pub struct Built {
    pub token: Judgement,
    pub orphans: Vec<TimedQuad>,
    pub mesh: Option<EffectMesh>,
    pub sound: Option<u16>,
}

impl Built {
    pub fn nodes(&self) -> usize {
        self.orphans.len() + usize::from(self.mesh.is_some()) + 1
    }

    pub fn discs(&self) -> usize {
        self.orphans.len() + self.token.held_count()
    }
}

pub fn build_at(at: Vec3, mode: i32, scale_multiplier: f32, now_ms: u32) -> Built {
    let mut token = Judgement::new(at, mode, now_ms);

    if mode == SIX {
        for i in 0..2 {
            token.owned.push(disc(
                at,
                [LIFT_SIX_A, LIFT_SIX_B][i],
                SIX_TEXTURE,
                SIX_LIFE_MS,
                SIX_SCALES[i],
                SIX_COLOURS[i],
                SIX_PERIODS[i],
                i,
            ));
        }

        return Built {
            token,
            orphans: Vec::new(),
            mesh: None,
            sound: sound_for(mode),
        };
    }

    let m = Mode::of(mode, scale_multiplier);

    token.owned.push(disc(
        at,
        LIFT_A,
        m.texture,
        m.life_ms,
        m.scale * DISC_SCALE,
        m.col1,
        m.period_ms,
        0,
    ));

    let period = spin_period(mode, m.period_ms, scale_multiplier);

    let mut held_b = disc(
        at,
        LIFT_B,
        m.texture,
        m.life_ms,
        m.scale * DISC_SCALE,
        m.col2,
        period.wrapping_neg(),
        1,
    );

    let mut orphans = Vec::new();

    let mut scale = m.scale;

    if mode == 3 {
        scale *= 0.5;

        let c = disc(
            at,
            LIFT_B,
            EXTRA_TEXTURE,
            m.life_ms,
            scale * DISC_SCALE,
            m.col2,
            period,
            2,
        );

        orphans.push(std::mem::replace(&mut held_b, c));
    }

    if mode == 7 || mode == 8 {
        let d = disc(
            at,
            LIFT_B,
            EXTRA_TEXTURE,
            m.life_ms,
            scale * EXTRA_SCALE,
            m.col1,
            period,
            2,
        );

        orphans.push(std::mem::replace(&mut held_b, d));
    }

    token.owned.push(held_b);

    Built {
        token,
        orphans,
        mesh: mesh_for(at, mode, &m, scale, now_ms),
        sound: sound_for(mode),
    }
}

#[allow(clippy::too_many_arguments)]

fn disc(
    at: Vec3,
    lift: f32,
    texture: usize,
    life_ms: u32,
    scale: f32,
    argb: u32,
    period_ms: i32,
    ordinal: usize,
) -> TimedQuad {
    TimedQuad::new(
        GroundQuad {
            owner: 0,
            slot: Slot::Placed,
            texture,
            pass: BillboardPass::Additive,
            anchor: Vec3::new(at.x, at.y + lift, at.z),
            base_scale: Vec3::splat(scale),
            grow: 0.0,
            spin_period: period_ms as u32,
            ordinal,
            argb,
        },
        life_ms,
    )
}

fn mesh_for(at: Vec3, mode: i32, m: &Mode, scale: f32, now_ms: u32) -> Option<EffectMesh> {
    let (flavour, scale_xz, scale_y, lifetime_ms, period_ms) = match mode {
        2 | 4 => (
            0,
            scale * EXTRA_SCALE,
            scale * EXTRA_SCALE,
            m.life_ms,
            m.life_ms,
        ),
        _ if Mode::has_extra(mode) => return None,
        _ => (
            mode.wrapping_add(2),
            scale * MESH_SCALE,
            scale * MESH_SCALE + mode as f32,
            m.life_ms,
            3000,
        ),
    };

    let mut mesh = EffectMesh::new(MESH_KIND, m.col1, 0.0, flavour, now_ms);

    mesh.position = at;

    mesh.scale_xz = scale_xz;

    mesh.scale_y = scale_y;

    mesh.lifetime_ms = lifetime_ms;

    mesh.period_ms = period_ms;

    Some(mesh)
}

pub fn sound_for(mode: i32) -> Option<u16> {
    match mode {
        0 => Some(SOUND),
        _ => None,
    }
}

pub fn arguments(spawn: &Spawn) -> Option<(i32, f32)> {
    if spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    let mode = match spawn.arguments[4] {
        SpawnArgument::Float(f) => f.to_bits() as i32,
        SpawnArgument::Int(n) => n,
        SpawnArgument::Bits(b) => b as i32,
        _ => return None,
    };

    let scale_multiplier = match spawn.arguments[5] {
        SpawnArgument::Float(f) => f,
        SpawnArgument::Bits(b) => f32::from_bits(b),
        SpawnArgument::Int(n) => n as f32,
        _ => return None,
    };

    Some((mode, scale_multiplier))
}

pub fn build(
    spawn: &Spawn,
    cx: &Cx<'_>,
    _env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let Some(at) = cx.at.at(spawn.at) else {
        return Err(Declined::Unplaced);
    };

    let Some((mode, scale_multiplier)) = arguments(spawn) else {
        return Err(Declined::Arguments);
    };

    let Built {
        token,
        orphans,
        mesh,
        sound: _,
    } = build_at(at, mode, scale_multiplier, cx.now_ms);

    for orphan in orphans {
        out.push(Box::new(orphan));
    }

    if let Some(mesh) = mesh {
        out.push(Box::new(mesh));
    }

    out.push(Box::new(token));

    Ok(())
}
