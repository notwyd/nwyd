use super::effects::{BillboardScene, Frame};
use super::lights::Glow;
use glam::Vec3;
use render::camera::Matrix4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeshSubmission {
    Glow(Glow),
    RotateLit(RotateLit),
    Textured(TexturedMesh),
    Authored(AuthoredMesh),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AuthoredMesh {
    pub mesh_id: u32,
    pub world: Matrix4,
    pub material: MeshMaterial,
    pub texture: Option<&'static str>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeshMaterial {
    Opaque,
    Additive,
    FreezeStone,
    Waterfall,
    Effect {
        blend: u32,
        tint: [f32; 4],
        uv_offset: [f32; 2],
    },
    Arrow {
        stage1: Option<usize>,
    },
    Swing {
        tint: [f32; 4],
        texture: Option<usize>,
    },
}

impl AuthoredMesh {
    pub fn states(
        &self,
        vertex_color: bool,
    ) -> (
        render::mesh::MeshPass,
        render::mesh::Cull,
        render::mesh::DrawParams,
    ) {
        use render::mesh::{Cull, DrawParams, MeshPass};

        if let MeshMaterial::Effect { blend, tint, .. } = self.material {
            return render::mesh::effect_states(blend, tint);
        }

        if let MeshMaterial::Swing { tint, .. } = self.material {
            return render::mesh::effect_states(1, tint);
        }

        if let MeshMaterial::Arrow { stage1 } = self.material {
            let (pass, cull, mut params) = render::mesh::effect_states(1, [1.0; 4]);

            params.stage1 = stage1.is_some();

            return (pass, cull, params);
        }

        if self.material == MeshMaterial::Opaque {
            (
                MeshPass::Opaque,
                Cull::Ccw,
                DrawParams::static_object(vertex_color, true),
            )
        } else if self.material == MeshMaterial::Waterfall {
            let mut params = DrawParams::static_object(vertex_color, false);

            params.stage1 = true;

            params.material.diffuse = [0.7, 0.7, 0.7, 1.0];

            params.material.ambient = [0.0; 4];

            params.material.emissive = [0.3, 0.3, 0.3, 0.0];

            (MeshPass::SolidEffect, Cull::Ccw, params)
        } else if self.material == MeshMaterial::FreezeStone {
            (
                MeshPass::SolidEffect,
                Cull::Ccw,
                DrawParams::static_object(vertex_color, false),
            )
        } else {
            render::mesh::effect_states(render::mesh::EFFECT_ADDITIVE, [1.0; 4])
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TexturedMesh {
    pub mesh_id: u32,
    pub world: Matrix4,
    pub texture: usize,
    pub tint: [f32; 4],
    pub blend: u32,
    pub uv_offset: [f32; 2],
}

impl TexturedMesh {
    pub fn states(
        &self,
    ) -> (
        render::mesh::MeshPass,
        render::mesh::Cull,
        render::mesh::DrawParams,
    ) {
        render::mesh::effect_states(self.blend, self.tint)
    }
}

impl MeshSubmission {
    pub fn into_glow(self) -> Option<Glow> {
        match self {
            MeshSubmission::Glow(glow) => Some(glow),
            MeshSubmission::RotateLit(_)
            | MeshSubmission::Textured(_)
            | MeshSubmission::Authored(_) => None,
        }
    }
}

impl From<Glow> for MeshSubmission {
    fn from(value: Glow) -> Self {
        MeshSubmission::Glow(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotateLit {
    pub mesh_id: i32,
    pub position: Vec3,
    pub world: Matrix4,
    pub owner: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnedGroup {
    HumanRotors(u16),
    HumanAffect(super::effect_context::ActorKey, u16),
    HandAffect(
        super::effect_context::ActorKey,
        u32,
        formats::mesh::appearance::Hand,
    ),
}

impl OwnedGroup {
    pub(crate) fn uses_old_skill_vfx(self) -> bool {
        matches!(
            self,
            Self::HandAffect(..)
                | Self::HumanAffect(
                    _,
                    0x80C
                        | 0x810
                        | 0x818
                        | 0x820
                        | 0x824
                        | 0x830
                        | 0x834
                        | 0x850
                        | 0x854
                        | 0x858
                        | 0x85C
                        | 0x860
                        | 0x8C4
                        | 0x8C8
                        | 0x8CC
                        | 0x8D0
                )
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removal {
    Expired,
    Replaced,
    Forced,
    SceneTeardown,
}

pub struct RemovalContext<'a> {
    pub now_ms: u32,
    pub detail: i32,
    pub ground: Option<&'a super::shade::Ground<'a>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pass {
    Effect,
    Mesh,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Base {
    VisualEffect,
    SceneNode,
}

pub const SCENE_NODE_METHOD_COUNT: usize = 14;

pub const VISUAL_EFFECT_METHOD_COUNT: usize = 15;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cull {
    Kept,
    Rejected,
    NotCullable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Class {
    pub name: &'static str,
    pub method_count: usize,
    pub base: Base,
    pub pass: Pass,
}

impl Class {
    pub const fn consistent(&self) -> bool {
        match self.base {
            Base::VisualEffect => self.method_count >= VISUAL_EFFECT_METHOD_COUNT,
            Base::SceneNode => self.method_count == SCENE_NODE_METHOD_COUNT,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tick {
    pub now_ms: u32,
    pub yaw: f32,
    pub pitch: f32,
}

pub struct Submit<'a> {
    pub scene: &'a BillboardScene,
    pub eye: Vec3,
    pub now_ms: u32,
}

pub struct Sink<'a> {
    pub effect: &'a mut Frame,
    pub mesh: &'a mut Vec<MeshSubmission>,
}

pub trait Effect: std::fmt::Debug {
    fn class(&self) -> &'static Class;

    fn update(&mut self, tick: &Tick);

    fn needs_ground(&self) -> bool {
        false
    }

    fn update_grounded(&mut self, tick: &Tick, _ground: &super::shade::Ground<'_>) {
        self.update(tick);
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        match live.ground {
            Some(ground) if self.needs_ground() => self.update_grounded(tick, ground),
            _ => self.update(tick),
        }
    }

    fn expired(&self) -> bool;

    fn removed(&mut self, _reason: Removal, _cx: &RemovalContext<'_>) {}

    fn removed_live(
        &mut self,
        reason: Removal,
        now_ms: u32,
        live: &mut super::effect_context::Live<'_>,
    ) {
        self.removed(
            reason,
            &RemovalContext {
                now_ms,
                detail: live.detail,
                ground: live.ground,
            },
        );
    }

    fn bind_meteor_clock(&mut self, _clock: std::rc::Rc<std::cell::Cell<u32>>) {}

    fn bind_freeze_clock(&mut self, _clock: std::rc::Rc<std::cell::Cell<u32>>) {}

    fn bind_arrow_clock(&mut self, _clock: std::rc::Rc<std::cell::Cell<u32>>) {}

    fn submit(&self, cx: &Submit<'_>, out: &mut Sink<'_>);

    fn submit_effect_children(&self, _cx: &Submit<'_>, _out: &mut Sink<'_>) {}

    fn tracked_entity(&self) -> Option<u16> {
        None
    }

    fn resolve_tracked_entity(&mut self, _position: Option<Vec3>) {}

    fn owned_group(&self) -> Option<OwnedGroup> {
        None
    }

    fn published_position(&self) -> Option<Vec3> {
        None
    }

    fn human_owner_hidden(&mut self) {}

    fn human_draw_visibility(&mut self, _visible: bool) {}

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        Vec::new()
    }

    fn take_sounds(&mut self) -> Vec<i64> {
        Vec::new()
    }

    fn take_hits(&mut self) -> Vec<(u16, u32)> {
        Vec::new()
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::NotCullable
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Refused {
    CullOnSceneNode,
    MethodCountDisagreesWithBase,
}

const PROBE_EYES: [Vec3; 3] = [
    Vec3::ZERO,
    Vec3::new(1.0, 1.0, 1.0),
    Vec3::new(100_000.0, 100_000.0, 100_000.0),
];

#[derive(Debug, Default)]
pub struct Tree {
    nodes: Vec<Box<dyn Effect>>,
    human_controls: human::Controls,
    meteor_clock: std::rc::Rc<std::cell::Cell<u32>>,
    freeze_clock: std::rc::Rc<std::cell::Cell<u32>>,
    arrow_clock: std::rc::Rc<std::cell::Cell<u32>>,
    pub refused: Vec<(&'static str, Refused)>,
    sounds: Vec<i64>,
    hits: Vec<(u16, u32)>,
}

mod human;

pub(crate) use human::Control as HumanControl;

impl Tree {
    pub(crate) fn control_humans(
        &mut self,
        controls: impl IntoIterator<Item = (super::effect_context::ActorKey, HumanControl)>,
    ) {
        self.human_controls.fill(controls);

        self.human_controls.apply(&mut self.nodes);
    }

    pub fn spawn(&mut self, mut node: Box<dyn Effect>) {
        node.bind_meteor_clock(self.meteor_clock.clone());

        node.bind_freeze_clock(self.freeze_clock.clone());

        node.bind_arrow_clock(self.arrow_clock.clone());

        let class = node.class();

        if !class.consistent() {
            self.refused
                .push((class.name, Refused::MethodCountDisagreesWithBase));

            return;
        }

        if class.base == Base::SceneNode {
            let culls = PROBE_EYES
                .iter()
                .flat_map(|&eye| [(eye, true), (eye, false)])
                .any(|(eye, field)| node.cull(eye, field) != Cull::NotCullable);

            if culls {
                self.refused.push((class.name, Refused::CullOnSceneNode));

                return;
            }
        }

        self.nodes.push(node);
    }

    pub fn refused_count(&self, why: Refused) -> usize {
        self.refused.iter().filter(|(_, w)| *w == why).count()
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = &dyn Effect> {
        self.nodes.iter().map(|n| &**n)
    }

    pub fn count_of(&self, name: &str) -> usize {
        self.nodes.iter().filter(|n| n.class().name == name).count()
    }

    pub fn remove_owned(&mut self, group: OwnedGroup) -> usize {
        self.remove_where(
            Removal::Replaced,
            &RemovalContext {
                now_ms: 0,
                detail: 0,
                ground: None,
            },
            |node| node.owned_group() == Some(group),
        )
    }

    pub fn remove_where(
        &mut self,
        reason: Removal,
        cx: &RemovalContext<'_>,
        mut remove: impl FnMut(&dyn Effect) -> bool,
    ) -> usize {
        let mut children = Vec::new();

        let mut removed = 0;

        self.nodes.retain_mut(|node| {
            if !remove(&**node) {
                return true;
            }

            node.removed(reason, cx);

            if reason != Removal::SceneTeardown {
                children.extend(node.take_spawned());

                self.sounds.extend(node.take_sounds());
            }

            removed += 1;

            false
        });

        for child in children {
            self.spawn(child);
        }

        removed
    }

    pub fn needs_ground(&self) -> bool {
        self.nodes.iter().any(|node| node.needs_ground())
    }

    pub fn resolve_tracked_entities(&mut self, mut position_of: impl FnMut(u16) -> Option<Vec3>) {
        for node in &mut self.nodes {
            if let Some(entity) = node.tracked_entity() {
                node.resolve_tracked_entity(position_of(entity));
            }
        }
    }

    pub fn detach_entity(&mut self, entity: u16) {
        for node in &mut self.nodes {
            if node.tracked_entity() == Some(entity) {
                node.resolve_tracked_entity(None);
            }
        }
    }

    pub fn update(&mut self, tick: &Tick) -> usize {
        self.update_inner(tick, None, None)
    }

    pub fn update_grounded(&mut self, tick: &Tick, ground: &super::shade::Ground<'_>) -> usize {
        self.update_inner(tick, Some(ground), None)
    }

    pub fn update_live(
        &mut self,
        tick: &Tick,
        live: &mut super::effect_context::Live<'_>,
    ) -> usize {
        self.update_inner(tick, None, Some(live))
    }

    fn update_inner(
        &mut self,
        tick: &Tick,
        ground: Option<&super::shade::Ground<'_>>,
        mut live: Option<&mut super::effect_context::Live<'_>>,
    ) -> usize {
        let before = self.nodes.len();

        let mut spawned: Vec<Box<dyn Effect>> = Vec::new();

        for node in &mut self.nodes {
            match live.as_deref_mut() {
                Some(live) => node.update_live(tick, live),
                None => match ground {
                    Some(ground) if node.needs_ground() => node.update_grounded(tick, ground),
                    _ => node.update(tick),
                },
            }

            if node.expired() {
                if let Some(live) = live.as_deref_mut() {
                    node.removed_live(Removal::Expired, tick.now_ms, live);
                } else {
                    node.removed(
                        Removal::Expired,
                        &RemovalContext {
                            now_ms: tick.now_ms,
                            detail: 0,
                            ground,
                        },
                    );
                }
            }

            spawned.append(&mut node.take_spawned());

            self.sounds.append(&mut node.take_sounds());

            self.hits.append(&mut node.take_hits());
        }

        self.nodes.retain(|n| !n.expired());

        let removed = before - self.nodes.len();

        for child in spawned {
            self.spawn(child);
        }

        removed
    }

    pub fn take_sounds(&mut self) -> Vec<i64> {
        std::mem::take(&mut self.sounds)
    }

    pub fn take_hits(&mut self) -> Vec<(u16, u32)> {
        std::mem::take(&mut self.hits)
    }

    pub fn submit(&self, pass: Pass, cx: &Submit<'_>, out: &mut Sink<'_>) {
        for node in &self.nodes {
            if !cx.scene.skill_vfx_visible(node.owned_group()) {
                continue;
            }

            if node.class().pass == pass {
                node.submit(cx, out);
            } else if pass == Pass::Effect {
                node.submit_effect_children(cx, out);
            }
        }
    }
}
