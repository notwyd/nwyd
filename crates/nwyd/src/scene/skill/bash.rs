use super::speedup;
use crate::scene::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use crate::scene::rng::Lcg;
use crate::scene::shade::Ground;
use crate::scene::vfx::{Ambient, Cx, Declined};
use formats::vfx::dispatch::Class as SkillClass;
use formats::vfx::{Axis, PositionRef, Spawn, SpawnArgument};
use glam::Vec3;

pub const CLASS: Class = Class {
    name: "SkillBash",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Effect,
};

pub const SPAWN_ARGUMENT_COUNT: usize = 5;

pub const OBJECT_SIZE: u16 = 156;

pub const LIFETIME_MS: u32 = 700;

pub const LAST_EMIT_INIT: u32 = 0;

pub const EMIT_GAP_MS: u32 = 250;

pub const EXPIRE_T: f32 = 1.0;

pub const DRIFT: f64 = 10.0;

pub const JITTER_SCALE: f32 = 0.2;

pub const JITTER_MOD: u32 = 3;

pub const CHILD_MODE: i32 = 1;

pub fn emitted_at(base: Vec3, t: f32, rx: u32, rz: u32) -> Vec3 {
    let drift = t as f64 * DRIFT;

    let axis = |r: u32, b: f32| {
        (((r % JITTER_MOD) as f64 - drift) * JITTER_SCALE as f64 + b as f64) as f32
    };

    Vec3::new(axis(rx, base.x), base.y, axis(rz, base.z))
}

#[derive(Debug)]
pub struct Bash {
    pub position: Vec3,
    pub birth_ms: u32,
    pub mode: i32,
    pub last_emit_ms: u32,
    bursts: Vec<speedup::Built>,
    explosion: Option<crate::scene::fire_ring::FireRing>,
    removed: bool,
    gone: bool,
}

impl Bash {
    pub fn new(at: Vec3, mode: i32, now_ms: u32) -> Bash {
        Bash {
            position: at,
            birth_ms: now_ms,
            mode,
            last_emit_ms: LAST_EMIT_INIT,
            bursts: Vec::new(),
            explosion: None,
            removed: false,
            gone: false,
        }
    }

    pub fn bursts(&self) -> &[speedup::Built] {
        &self.bursts
    }

    pub fn progress(&self, now_ms: u32) -> f32 {
        now_ms.wrapping_sub(self.birth_ms) as f32 / LIFETIME_MS as f32
    }

    pub fn due(&self, now_ms: u32) -> bool {
        now_ms.wrapping_sub(self.last_emit_ms) > EMIT_GAP_MS
    }

    pub fn tick_with(&mut self, now_ms: u32, rng: &mut Lcg, ground: Option<&Ground<'_>>) {
        if self.gone {
            return;
        }

        let t = self.progress(now_ms);

        if t > EXPIRE_T {
            self.gone = true;

            return;
        }

        if !self.due(now_ms) {
            return;
        }

        let (rx, rz) = (rng.next(), rng.next());

        let mut built = speedup::build_at(emitted_at(self.position, t, rx, rz), CHILD_MODE, now_ms);

        if let Some(g) = ground {
            built.place(g);
        }

        self.bursts.push(built);

        self.last_emit_ms = now_ms;
    }
}

impl Effect for Bash {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        let mut rng = Lcg::new(self.birth_ms);

        self.tick_with(tick.now_ms, &mut rng, None);
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn needs_ground(&self) -> bool {
        true
    }

    fn update_grounded(&mut self, tick: &Tick, ground: &Ground<'_>) {
        self.tick_with(tick.now_ms, &mut Lcg::new(self.birth_ms), Some(ground));
    }

    fn update_live(&mut self, tick: &Tick, live: &mut crate::scene::effect_context::Live<'_>) {
        self.tick_with(tick.now_ms, live.rng, live.ground);
    }

    fn removed(
        &mut self,
        reason: crate::scene::effect_tree::Removal,
        cx: &crate::scene::effect_tree::RemovalContext<'_>,
    ) {
        if self.removed {
            return;
        }

        self.removed = true;

        self.gone = true;

        if reason == crate::scene::effect_tree::Removal::SceneTeardown {
            return;
        }

        self.explosion = Some(crate::scene::fire_ring::FireRing::new(
            self.position,
            0,
            0.5,
            250,
            0xFFFF_FFFF,
            cx.now_ms,
            cx.ground,
        ));
    }

    fn submit(&self, _cx: &Submit<'_>, _out: &mut Sink<'_>) {}

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        let mut nodes: Vec<Box<dyn Effect>> = std::mem::take(&mut self.bursts)
            .into_iter()
            .flat_map(speedup::Built::into_nodes)
            .collect();

        if let Some(mut explosion) = self.explosion.take() {
            nodes.extend(explosion.take_spawned());

            nodes.push(Box::new(explosion));
        }

        nodes
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Order {
    pub at: PositionRef,
    pub mode: i32,
}

pub fn read(spawn: &Spawn) -> Option<Order> {
    if spawn.class != SkillClass::SkillBash || spawn.arguments.len() != SPAWN_ARGUMENT_COUNT {
        return None;
    }

    for (i, axis) in [Axis::X, Axis::Y, Axis::Z].iter().enumerate() {
        match spawn.arguments[1 + i] {
            SpawnArgument::Pos(a) if a == *axis => {}
            _ => return None,
        }
    }

    let mode = match spawn.arguments[4] {
        SpawnArgument::Bits(v) => v as i32,
        SpawnArgument::Int(v) => v,
        _ => return None,
    };

    Some(Order { at: spawn.at, mode })
}

pub fn build_spawn(
    spawn: &Spawn,
    cx: &Cx<'_>,
    _env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
) -> Result<(), Declined> {
    let Some(order) = read(spawn) else {
        return Err(Declined::Arguments);
    };

    let Some(at) = cx.at.at(order.at) else {
        return Err(Declined::Unplaced);
    };

    out.push(Box::new(build(&order, at, cx.now_ms)));

    Ok(())
}

pub fn build(order: &Order, at: Vec3, now_ms: u32) -> Bash {
    Bash::new(at, order.mode, now_ms)
}
