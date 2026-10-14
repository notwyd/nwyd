use super::effect_tree::{
    Base, Class, Cull, Effect, MeshSubmission, Pass, Sink, Submit, TexturedMesh, Tick,
};
use super::freeze_blade::FreezeBlade;
use super::lod::DetailGate;
use super::rng::Lcg;
use super::shade::{Ground, Shade};
use crate::scene::effects::SECTOR_UNASSIGNED;
use glam::Vec3;

pub const CLASS: Class = Class {
    name: "SkillIceSpear",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

pub const SPAWN_ARGUMENT_COUNT: usize = 9;

pub const OBJECT_SIZE: u16 = 188;

pub const MESH_ID: i32 = 0x2C4;

pub const MESH_TTL_MS: i32 = 180_000;

pub const LIFETIME_PER_UNIT_MS: i32 = 100;

pub const LIFETIME_FLOOR_MS: i32 = 1;

pub const LIFETIME_CEILING_MS: i32 = 5000;

pub const HALFWAY_T: f32 = 0.5;

pub const EXPIRE_T: f32 = 1.0;

pub const DRAW_T: f32 = 0.05;

pub const YAW_OFFSET: f32 = std::f32::consts::FRAC_PI_2;

pub const SHADE_GRID: i32 = 4;

pub const SHADE_TEXTURE: usize = 7;

pub const SHADE_ARGB: u32 = 0x3333_3344;

pub const SOUNDS: [u16; 2] = [0x9A, 0xA0];

pub const IMPACT_GATE: DetailGate = DetailGate::Full;

pub const IMPACT_KIND: i32 = 1;

pub const IMPACT_DROP: f32 = 1.0;

#[derive(Debug)]

pub struct IceSpear {
    pub source: Vec3,
    pub dest: Vec3,
    pub spare: i32,
    pub target: Option<u16>,
    pub position: Vec3,
    pub birth_ms: u32,
    pub lifetime_ms: i32,
    pub t: f32,
    pub yaw: f32,
    pub shade: Option<Shade>,
    pub impact_admitted: bool,
    spawned: Vec<FreezeBlade>,
    gone: bool,
}

impl IceSpear {
    pub fn new(
        source: Vec3,
        dest: Vec3,
        spare: i32,
        target: Option<u16>,
        now_ms: u32,
        detail: i32,
    ) -> IceSpear {
        let span = (dest - source).length().trunc() as i32;

        let lifetime_ms = match span.saturating_mul(LIFETIME_PER_UNIT_MS) {
            0 => LIFETIME_FLOOR_MS,
            ms => ms.min(LIFETIME_CEILING_MS),
        };

        IceSpear {
            source,
            dest,
            spare,
            target,
            position: source,
            birth_ms: now_ms,
            lifetime_ms,
            t: 0.0,
            yaw: 0.0,
            shade: Some(Shade::new(
                SHADE_GRID,
                SHADE_TEXTURE,
                SHADE_ARGB,
                source,
                SECTOR_UNASSIGNED,
            )),
            impact_admitted: IMPACT_GATE.admits(detail),
            spawned: Vec::new(),
            gone: false,
        }
    }

    pub fn tick_with(
        &mut self,
        now_ms: u32,
        at: impl Fn(u16) -> Option<Vec3>,
        ground: Option<&Ground<'_>>,
    ) {
        if self.gone {
            return;
        }

        let elapsed = now_ms.wrapping_sub(self.birth_ms) as f32;

        self.t = elapsed / self.lifetime_ms.max(LIFETIME_FLOOR_MS) as f32;

        if self.t > HALFWAY_T {
            self.target = None;
        }

        if let Some(live) = self.target.and_then(&at) {
            self.dest = Vec3::new(live.x, live.y + 1.0, live.z);
        }

        let delta = self.dest - self.source;

        self.yaw = delta.x.atan2(delta.z) - YAW_OFFSET;

        if self.t >= EXPIRE_T {
            self.expire(now_ms);

            return;
        }

        let inv = 1.0 - self.t;

        self.position = Vec3::new(
            inv * self.source.x + self.t * self.dest.x,
            inv * self.source.y + self.dest.y * self.t,
            inv * self.source.z + self.t * self.dest.z,
        );

        if let (Some(shade), Some(g)) = (self.shade.as_mut(), ground) {
            shade.centre = Vec3::new(self.position.x, 0.0, self.position.z);

            shade.place(g);
        }
    }

    fn expire(&mut self, now_ms: u32) {
        if self.gone {
            return;
        }

        self.gone = true;

        self.shade = None;

        if !self.impact_admitted {
            return;
        }

        let at = Vec3::new(
            self.position.x,
            self.position.y - IMPACT_DROP,
            self.position.z,
        );

        let mut unused = Lcg::new(self.birth_ms);

        self.spawned.push(FreezeBlade::new(
            at,
            IMPACT_KIND,
            false,
            false,
            &mut unused,
            now_ms,
        ));
    }

    pub fn mesh_draw(&self) -> Option<TexturedMesh> {
        if self.gone || self.t < DRAW_T {
            return None;
        }

        Some(TexturedMesh {
            uv_offset: [0.0; 2],
            blend: 1,
            mesh_id: 708,
            world: super::mesh_fx::rotate_lit_world(self.position, self.yaw, 1.0),
            texture: 19,
            tint: [1.0; 4],
        })
    }
}

impl Effect for IceSpear {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn update(&mut self, tick: &Tick) {
        self.tick_with(tick.now_ms, |_| None, None);
    }

    fn needs_ground(&self) -> bool {
        true
    }

    fn update_grounded(&mut self, tick: &Tick, ground: &Ground<'_>) {
        self.tick_with(tick.now_ms, |_| None, Some(ground));
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        self.impact_admitted = IMPACT_GATE.admits(live.detail);

        self.tick_with(tick.now_ms, |_| None, live.ground);
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn removed(
        &mut self,
        reason: super::effect_tree::Removal,
        cx: &super::effect_tree::RemovalContext<'_>,
    ) {
        if reason == super::effect_tree::Removal::SceneTeardown {
            self.gone = true;

            self.shade = None;

            self.spawned.clear();

            return;
        }

        self.impact_admitted = IMPACT_GATE.admits(cx.detail);

        self.expire(cx.now_ms);
    }

    fn submit(&self, _cx: &Submit<'_>, out: &mut Sink<'_>) {
        if let Some(draw) = self.mesh_draw() {
            out.mesh.push(MeshSubmission::Textured(draw));
        }
    }

    fn submit_effect_children(&self, cx: &Submit<'_>, out: &mut Sink<'_>) {
        if self.mesh_draw().is_none() {
            return;
        }

        if let Some(shade) = &self.shade {
            shade.submit(cx, out);
        }
    }

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        std::mem::take(&mut self.spawned)
            .into_iter()
            .map(|blade| Box::new(blade) as Box<dyn Effect>)
            .collect()
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }
}
