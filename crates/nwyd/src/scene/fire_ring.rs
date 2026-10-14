use super::effect_tree::{Base, Class, Cull, Effect, Pass, Sink, Submit, Tick};
use super::fire::Fire;
use super::shade::{Ground, Shade};
use glam::Vec3;

pub const CLASS: Class = Class {
    name: "SkillFireRing",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

pub const DURATION_MS: i32 = 0x320;

pub const EXPIRE_T: f32 = 1.0;

pub const SHADE_EXTRA_MS: u32 = 1_000;

pub const SHADE_GRID: i32 = 4;

pub const SHADE_TEXTURE: usize = 7;

pub const SHADE_TINT_MODE_0: u32 = 0x7777_5511;

pub const SHADE_TINT_OTHER: u32 = 0x0000_1122;

pub const OPEN_SOUND: i64 = 0x9B;

pub const RING: usize = 8;

pub const RING_X: [f32; RING] = [1.0, 1.0, 0.0, -1.0, -1.0, -1.0, 0.0, 1.0];

pub const RING_Z: [f32; RING] = [0.0, 1.0, 1.0, 1.0, 0.0, -1.0, -1.0, -1.0];

pub const ZONE_MANHATTAN_RADIUS: f32 = 18.0;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Zone {
    pub manhattan: bool,
    pub low: (f32, f32),
    pub high: (f32, f32),
    pub centre: (f32, f32),
}

impl Zone {
    pub fn admits(&self, x: f32, z: f32) -> bool {
        match self.manhattan {
            false => self.low.0 < x && x < self.high.0 && self.low.1 < z && z < self.high.1,
            true => (self.centre.0 - x).abs() + (self.centre.1 - z).abs() < ZONE_MANHATTAN_RADIUS,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Child {
    pub fire_mode: i32,
    pub spark_colour: Option<i32>,
    pub colour: u32,
}

pub const CHILDREN: [Child; 3] = [
    Child {
        fire_mode: 2,
        spark_colour: None,
        colour: 0x2233_1100,
    },
    Child {
        fire_mode: 4,
        spark_colour: Some(0x1122),
        colour: 0x0000_1122,
    },
    Child {
        fire_mode: 5,
        spark_colour: Some(0x4488),
        colour: 0x0000_1122,
    },
];

#[derive(Debug)]

pub struct FireRing {
    pub position: Vec3,
    pub mode: i32,
    pub scale: f32,
    pub interval_ms: i32,
    pub colour: u32,
    pub birth_ms: u32,
    pub duration_ms: i32,
    pub last_emit_ms: u32,
    pub t: f32,
    decal: Option<Shade>,
    fires: Vec<Fire>,
    sounds: Vec<i64>,
    gone: bool,
}

impl FireRing {
    pub fn new(
        position: Vec3,
        mode: i32,
        scale: f32,
        interval_ms: i32,
        colour: u32,
        now_ms: u32,
        ground: Option<&Ground<'_>>,
    ) -> FireRing {
        let tint = match mode {
            0 => SHADE_TINT_MODE_0,
            _ => SHADE_TINT_OTHER,
        };

        let mut decal = Shade::new(
            SHADE_GRID,
            SHADE_TEXTURE,
            tint,
            position,
            super::effects::SECTOR_UNASSIGNED,
        )
        .with_lifetime(DURATION_MS.max(0) as u32 + SHADE_EXTRA_MS);

        if let Some(ground) = ground {
            decal.place(ground);
        }

        FireRing {
            position,
            mode,
            scale,
            interval_ms,
            colour,
            birth_ms: now_ms,
            duration_ms: DURATION_MS,
            last_emit_ms: 0,
            t: 0.0,
            decal: Some(decal),
            fires: Vec::new(),
            sounds: vec![OPEN_SOUND],
            gone: false,
        }
    }

    pub fn child_of(mode: i32) -> Option<Child> {
        usize::try_from(mode)
            .ok()
            .and_then(|m| CHILDREN.get(m).copied())
    }

    pub fn ring_at(&self, i: usize, t: f32) -> (f32, f32) {
        (
            RING_X[i] * t * self.scale + self.position.x,
            t * RING_Z[i] * self.scale + self.position.z,
        )
    }

    pub fn tick_with(&mut self, now_ms: u32, ground: Option<&Ground<'_>>, zone: Option<&Zone>) {
        let elapsed = now_ms.wrapping_sub(self.birth_ms) as f32;

        self.t = elapsed / self.duration_ms.max(1) as f32;

        if self.t > EXPIRE_T {
            self.gone = true;

            return;
        }

        if now_ms.wrapping_sub(self.last_emit_ms) <= self.interval_ms.max(0) as u32 {
            return;
        }

        let admitted = zone.is_none_or(|z| z.admits(self.position.x, self.position.z));

        if admitted {
            self.emit(now_ms, ground);
        }

        self.last_emit_ms = now_ms;
    }

    fn emit(&mut self, now_ms: u32, ground: Option<&Ground<'_>>) {
        let Some(child) = FireRing::child_of(self.mode) else {
            return;
        };

        let spark_colour = child.spark_colour.unwrap_or(self.colour as i32);

        for i in 0..RING {
            let (x, z) = self.ring_at(i, self.t);

            let y = ground.and_then(|g| g.surface_height(x, z)).unwrap_or(0.0);

            let at = Vec3::new(x, y, z);

            self.fires.push(Fire::new(
                at,
                child.fire_mode,
                None,
                spark_colour,
                child.colour,
                now_ms,
                ground,
            ));
        }
    }

    pub fn fires(&self) -> &[Fire] {
        &self.fires
    }

    pub fn decal(&self) -> Option<&Shade> {
        self.decal.as_ref()
    }
}

impl Effect for FireRing {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        self.tick_with(tick.now_ms, None, None);
    }

    fn needs_ground(&self) -> bool {
        true
    }

    fn update_grounded(&mut self, tick: &Tick, ground: &Ground<'_>) {
        if let Some(decal) = self.decal.as_mut() {
            decal.place(ground);
        }

        self.tick_with(tick.now_ms, Some(ground), None);
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn submit(&self, _cx: &Submit<'_>, _out: &mut Sink<'_>) {}

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        let mut out: Vec<Box<dyn Effect>> = Vec::new();

        if let Some(decal) = self.decal.take() {
            out.push(Box::new(decal));
        }

        for mut fire in self.fires.drain(..) {
            out.extend(fire.take_spawned());

            out.push(Box::new(fire));
        }

        out
    }

    fn take_sounds(&mut self) -> Vec<i64> {
        std::mem::take(&mut self.sounds)
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}
