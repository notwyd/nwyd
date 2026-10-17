use super::actors::ActorScene;
use super::effect_context::ActorKey;
use super::effect_tree::{
    Class, Cull, Effect, OwnedGroup, Removal, RemovalContext, Sink, Submit, Tick,
};
use super::effects::{Billboard, BillboardScene};
use glam::Vec3;

pub(crate) fn relocated(actor: &mut super::actors::Actor, effects: &mut BillboardScene, now: u32) {
    let key = ActorKey::of(actor);

    actor.skill_anchor = None;

    actor.body_points.points.fill(None);

    for hand in &mut actor.swings {
        hand.owner_visible = false;
    }

    effects.billboards.retain(|b| {
        !b.body_owned
            .is_some_and(|(owner, slot)| owner == key && matches!(slot, 0x820 | 0x824))
    });

    effects.tree.remove_where(
        Removal::Forced,
        &RemovalContext {
            now_ms: now,
            detail: 0,
            ground: None,
        },
        |node| match node.owned_group() {
            Some(OwnedGroup::HandAffect(owner, ..)) => owner == key,
            Some(OwnedGroup::HumanAffect(owner, 0x8D4 | 0x8D8)) => owner == key,
            _ => false,
        },
    );
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct MotionState {
    pub water_ms: u32,
    pub previous_body: Vec3,
}

pub(crate) fn attack_body_trail(actor: &mut super::actors::Actor) -> Option<Billboard> {
    if actor.model_id != 20
        || !matches!(actor.parts[1].mesh, 0 | 2)
        || actor.action != 5
        || !actor.anim.frame.is_multiple_of(5)
    {
        return None;
    }

    let at = actor.body_points.points[0]?;

    if !at.is_finite() || at == actor.motion_effects.previous_body {
        return None;
    }

    actor.motion_effects.previous_body = at;

    let mut b = Billboard::new(44, 2000, Vec3::splat(0.5), 0.0005, 1, 80);

    b.position = at;

    b.pass = render::billboard::BillboardPass::Additive;

    Some(b)
}

fn submerged_surface(
    ground: &[&formats::world::trn::Terrain],
    seas: &[super::sea::Sea],
    at: Vec3,
) -> Option<f32> {
    if !at.is_finite() {
        return None;
    }

    let owner = ground.iter().find(|terrain| terrain.contains(at.x, at.z))?;

    let sea = seas
        .iter()
        .filter(|s| s.zone == (owner.zone_x, owner.zone_y))
        .take(10)
        .find(|s| s.contains(at.x, at.z))?;

    (sea.position.y > at.y).then_some(sea.position.y)
}

pub(crate) fn water_ripples(world: &mut super::world::World, now: u32) {
    let reach = world.pose_reach();

    let ground: Vec<_> = std::iter::once(&world.terrain)
        .chain(world.streamed.iter().map(|s| &s.terrain))
        .collect();

    for actor in &mut world.actors.actors {
        if actor.class == 45
            || !matches!(actor.action, 2 | 3)
            || !reach.admits(actor.position)
            || world.actors.models.get(actor.model).is_none()
        {
            continue;
        }

        let Some(y) = submerged_surface(&ground, &world.sea.seas, actor.position) else {
            continue;
        };

        let period = if actor.action == 2 { 120 } else { 80 };

        if now.wrapping_sub(actor.motion_effects.water_ms) <= period {
            continue;
        }

        let Some(&(radius, _)) = super::pick::PICK_VOLUME.get(actor.model_id as usize) else {
            continue;
        };

        if !actor.scale.is_finite() {
            continue;
        }

        actor.motion_effects.water_ms = now;

        let quad = super::timers::TimedQuad::new(
            super::quads::GroundQuad {
                owner: 0,
                slot: super::quads::Slot::Placed,
                texture: 10,
                pass: render::billboard::BillboardPass::Additive,
                anchor: Vec3::new(actor.position.x, y + 0.25, actor.position.z),
                base_scale: Vec3::splat(0.5),
                grow: radius * actor.scale * 0.002,
                spin_period: 0,
                ordinal: 0,
                argb: 0xFFFFFFFF,
            },
            700,
        );

        world.effects.spawn(Box::new(quad));
    }
}

pub(crate) fn body_cloud(
    actor: &mut super::actors::Actor,
    now: u32,
    rng: &mut super::rng::Lcg,
) -> Option<([Billboard; 2], super::shade::Shade)> {
    let part = actor.parts[0];

    if actor.class != 25
        || !(part.mesh == 12 || part.mesh == 3 && part.tex == 8)
        || now.wrapping_sub(actor.body_cloud_ms) <= 1000
    {
        return None;
    }

    actor.body_cloud_ms = now;

    let particles = std::array::from_fn(|i| {
        let random = (rng.next() % 3) as f32;

        let size = actor.scale * 1.2 + random * 0.1;

        let mut b = Billboard::new(
            89,
            2400 + i as u32 * 600,
            Vec3::new(size, actor.scale + 0.3, size),
            0.0005,
            1,
            80,
        );

        let offset = i as f32 * 0.3 * random;

        b.position = actor.position + Vec3::new(offset, actor.scale * 1.6 - offset, offset);

        b.origin = b.position;

        b.motion = super::effects::Motion::Rise;

        b.rise = -2.0;

        b.birth_ms = Some(now);

        b
    });

    let random = (rng.next() % 3) as f32;

    let (sin, cos) = (actor.yaw - std::f32::consts::PI).sin_cos();

    let at = actor.position + Vec3::new(cos * 0.5 + random * 0.2, 0.0, -sin * 0.5 + random * 0.2);

    let mut mark = super::shade::Shade::new(2, 89, 0xCCCCCCCC, at, -1).with_lifetime(3000);

    mark.pass = render::billboard::BillboardPass::Blended;

    mark.cosine_fade = true;

    mark.rotation = random * std::f32::consts::PI / 6.0;

    mark.birth_ms = Some(now);

    Some((particles, mark))
}

#[derive(Debug)]
struct HandSprite {
    group: OwnedGroup,
    billboard: Billboard,
}

impl HandSprite {
    fn follow(&mut self, body: Option<super::effect_context::Snapshot>) {
        if let OwnedGroup::HandAffect(key, epoch, hand) = self.group
            && let Some(body) = body
            && body.key == key
            && body.appearance_epoch == epoch
            && let Some(point) = body.hand_glow_points[hand.part() - 6]
        {
            self.billboard.position = point;
        }
    }
}

impl Effect for HandSprite {
    fn class(&self) -> &'static Class {
        &super::effects::CLASS
    }

    fn owned_group(&self) -> Option<OwnedGroup> {
        Some(self.group)
    }

    fn update(&mut self, tick: &Tick) {
        Effect::update(&mut self.billboard, tick);
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        if let OwnedGroup::HandAffect(key, ..) = self.group {
            self.follow((live.body)(key));
        }

        self.update(tick);
    }

    fn expired(&self) -> bool {
        false
    }

    fn cull(&self, eye: Vec3, field: bool) -> Cull {
        self.billboard.cull(eye, field)
    }

    fn submit(&self, cx: &Submit<'_>, sink: &mut Sink<'_>) {
        self.billboard.submit(cx, sink);
    }
}

pub(crate) fn hand_sprites(
    actors: &ActorScene,
    effects: &mut BillboardScene,
    now: u32,
    create: bool,
) {
    effects.tree.remove_where(
        Removal::Forced,
        &RemovalContext {
            now_ms: now,
            detail: 0,
            ground: None,
        },
        |node| {
            let Some(OwnedGroup::HandAffect(key, epoch, hand)) = node.owned_group() else {
                return false;
            };

            !actors.actors.iter().any(|a| {
                ActorKey::of(a) == key
                    && a.appearance_epoch == epoch
                    && a.swings
                        .iter()
                        .any(|s| s.hand == hand && s.emitters.weapon_effect != 0)
            })
        },
    );

    if !create {
        return;
    }

    for actor in &actors.actors {
        for hand in actor
            .swings
            .iter()
            .filter(|s| s.owner_visible && s.emitters.weapon_effect != 0)
        {
            let group =
                OwnedGroup::HandAffect(ActorKey::of(actor), actor.appearance_epoch, hand.hand);

            if effects
                .tree
                .iter()
                .any(|node| node.owned_group() == Some(group))
            {
                continue;
            }

            let mut billboard = Billboard::new(56, 0, Vec3::ONE, 0.0, 1, 80);

            billboard.pass = render::billboard::BillboardPass::Additive;

            billboard.position = hand.position + Vec3::Y * 0.2;

            billboard.set_color(0xFF55AAFF);

            billboard.colour_mode = 0;

            effects.spawn(Box::new(HandSprite { group, billboard }));
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct Pet {
    model: i32,
    part: i16,
    texture: i16,
    variant: i32,
    scale: f32,
    additive: bool,
    low: bool,
}

fn pet(item: u16) -> Option<Pet> {
    let (model, part, texture, variant, scale, additive, low) = match item {
        0xF4F | 0xF50 => (20, 2, 0, 9, 0.2, false, true),
        0x2F1 => (20, 2, 0, 3, 0.09, false, true),
        0x301 => (32, 1, 0, 4, 1.2, true, false),
        0x6BE => (32, 2, 0, 5, 1.0, false, false),
        0xF3C | 0xF3F | 0xF42 | 0xF47..=0xF49 => (32, 3, 0, 6, 1.0, true, false),
        0xF3D | 0xF40 | 0xF43 | 0xFDC => (32, 3, 1, 6, 1.0, true, false),
        0xF3E | 0xF41 | 0xF44 => (32, 3, 2, 6, 1.0, true, false),
        0xF4A => (32, 3, 3, 6, 1.0, true, false),
        0xF4B | 0xF4C => (32, 3, 4, 6, 1.0, true, false),
        _ => return None,
    };

    Some(Pet {
        model,
        part,
        texture,
        variant,
        scale,
        additive,
        low,
    })
}

pub(crate) fn held_skins(actors: &mut ActorScene, now: u32) {
    actors.skinfx.remove_where(|effect| {
        let Some(slot) = effect.held_slot else {
            return false;
        };

        let Some(actor) = effect
            .owner
            .and_then(|key| super::effect_context::actor(&actors.actors, key))
        else {
            return true;
        };

        match slot {
            0x860 => actor.flags.get(0xB3D) != 1,
            0x864 => pet(actor.pet_item).is_none() || effect.held_item != Some(actor.pet_item),
            _ => true,
        }
    });

    for index in 0..actors.actors.len() {
        let actor = &actors.actors[index];

        let key = ActorKey::of(actor);

        let hidden = actor.flags.get(0xB55) != 0 || actor.flags.get(0xB3E) == 1;

        let quality = actors.secondary_effects_enabled() && actors.effects_detail.glow;

        for effect in actors
            .skinfx
            .objects
            .iter_mut()
            .filter(|e| e.owner == Some(key) && e.held_slot.is_some())
        {
            effect.visible = !hidden && (effect.held_slot == Some(0x864) || quality);
        }

        if hidden {
            continue;
        }

        let halo = (actor.flags.get(0xB3D) == 1 && actors.effects_detail.glow).then_some(Pet {
            model: 32,
            part: 0,
            texture: 0,
            variant: 3,
            scale: 0.3,
            additive: true,
            low: false,
        });

        let pet_item = actor.pet_item;

        let pet = pet(pet_item);

        let at = actor.position;

        let yaw = actor.yaw;

        let height = super::pick::PICK_VOLUME
            .get(actor.model_id as usize)
            .map_or(0.0, |(_, h)| *h);

        let body_scale = actor.scale;

        for (slot, recipe) in [(0x860, halo), (0x864, pet)] {
            let Some(recipe) = recipe else {
                continue;
            };

            if actors
                .skinfx
                .objects
                .iter()
                .any(|e| e.owner == Some(key) && e.held_slot == Some(slot))
            {
                continue;
            }

            use super::skinfx::{
                Blend, ColourAnim, Motion, SkinConstruction, SkinInput, SkinMeshEffect,
            };
            use formats::mesh::appearance::PartRef;

            let mut parts = [PartRef::EMPTY; 8];

            parts[..2].fill(PartRef {
                mesh: recipe.part,
                tex: recipe.texture,
            });

            let y = if slot == 0x860 {
                at.y + 2.5
            } else {
                (height + 0.3) * body_scale + if recipe.low { 0.0 } else { at.y }
            };

            let source = Vec3::new(at.x, y, at.z);

            let mut effect = SkinMeshEffect::new(recipe.model, source, source, recipe.variant);

            effect.owner = Some(key);

            effect.held_slot = Some(slot);

            effect.held_item = (slot == 0x864).then_some(pet_item);

            effect.lifetime_ms = 0;

            effect.spin = yaw;

            effect.colour_a = [1.0; 3];

            effect.colour_anim = ColourAnim::Cosine;

            effect.blend = if recipe.additive {
                Blend::Additive
            } else {
                Blend::Alpha
            };

            effect.motion = if recipe.low {
                Motion::OwnerHaloLow
            } else {
                Motion::OwnerHalo
            };

            effect.visible = slot == 0x864 || quality;

            effect.construction = Some(SkinConstruction {
                primary: SkinInput {
                    model_id: recipe.model,
                    parts,
                    pose: None,
                    animation: None,
                    clip_action: None,
                    scale: recipe.scale,
                },
                secondary: None,
                cape: false,
                body: None,
            });

            match actors.hydrate_skin(&mut effect, now) {
                Ok(()) => actors.skinfx.spawn(effect),
                Err(error) => {
                    tracing::warn!(
                        entity = key.entity,
                        slot,
                        ?error,
                        "retained skin could not be hydrated"
                    )
                }
            }
        }
    }
}

#[derive(Debug)]
struct OwnedQuad {
    key: ActorKey,
    slot: u16,
    mode: i32,
    lift: f32,
    quad: super::quads::GroundQuad,
    detail: i32,
    gone: bool,
    disabled: bool,
}

#[derive(Debug)]
struct StatusSprite {
    key: ActorKey,
    slot: u16,
    billboard: Billboard,
    gone: bool,
    disabled: bool,
}

impl Effect for StatusSprite {
    fn class(&self) -> &'static Class {
        &super::effects::CLASS
    }

    fn owned_group(&self) -> Option<OwnedGroup> {
        Some(OwnedGroup::HumanAffect(self.key, self.slot))
    }

    fn update(&mut self, tick: &Tick) {
        Effect::update(&mut self.billboard, tick);
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        let Some(body) = (live.body)(self.key) else {
            self.gone = true;

            return;
        };

        if body.effects_hidden {
            self.disabled = true;
        } else {
            let mounted = body.appearance.is_some_and(|a| a.mounted);

            let anchor = if mounted {
                body.skill_anchor.unwrap_or(body.position)
            } else {
                body.position
            };

            let height = body
                .dimensions
                .map_or(0.0, |(_, height)| height * body.scale);

            self.billboard.position = anchor + Vec3::Y * (height - if mounted { 0.3 } else { 0.0 });
        }

        self.update(tick);

        self.billboard.ready = super::effects::visible_in_scene(live.scene, !self.disabled);
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn cull(&self, eye: Vec3, field: bool) -> Cull {
        self.billboard.cull(eye, field)
    }

    fn submit(&self, cx: &Submit<'_>, sink: &mut Sink<'_>) {
        if !self.gone {
            self.billboard.submit(cx, sink);
        }
    }
}

fn status_slot(actor: &super::actors::Actor) -> Option<(u16, usize)> {
    match (actor.flags.get(0xB42) == 1, actor.flags.get(0xB43) == 1) {
        (true, false) => Some((0x83C, 438)),
        (false, true) => Some((0x840, 439)),
        (true, true) => Some((0x844, 440)),
        _ => None,
    }
}

fn status_sprites(actors: &ActorScene, effects: &mut BillboardScene, now: u32) {
    effects.tree.remove_where(
        Removal::Forced,
        &RemovalContext {
            now_ms: now,
            detail: 0,
            ground: None,
        },
        |node| {
            let Some(OwnedGroup::HumanAffect(key, slot @ (0x83C | 0x840 | 0x844))) =
                node.owned_group()
            else {
                return false;
            };

            !actors.actors.iter().any(|a| {
                ActorKey::of(a) == key
                    && (a.flags.get(0xB55) != 0
                        || a.flags.get(0xB3E) == 1
                        || status_slot(a).is_some_and(|(s, _)| s == slot))
            })
        },
    );

    for actor in &actors.actors {
        if actor.flags.get(0xB55) != 0 || actor.flags.get(0xB3E) == 1 {
            continue;
        }

        let Some((slot, texture)) = status_slot(actor) else {
            continue;
        };

        let key = ActorKey::of(actor);

        let group = OwnedGroup::HumanAffect(key, slot);

        if effects
            .tree
            .iter()
            .any(|node| node.owned_group() == Some(group))
        {
            continue;
        }

        let mut billboard = Billboard::new(texture, 0, Vec3::splat(0.4), 0.0, 1, 80);

        billboard.birth_ms = Some(now);

        billboard.colour_mode = 2;

        billboard.pass = render::billboard::BillboardPass::Additive;

        billboard.motion = super::effects::Motion::Halo;

        billboard.rise = 0.1;

        billboard.amp_h = 0.1;

        effects.spawn(Box::new(StatusSprite {
            key,
            slot,
            billboard,
            gone: false,
            disabled: false,
        }));
    }
}

#[derive(Debug)]
struct OwnedMesh {
    key: ActorKey,
    slot: u16,
    index: usize,
    first: bool,
    mesh: super::effect_mesh::EffectMesh,
    gone: bool,
}

impl Effect for OwnedMesh {
    fn class(&self) -> &'static Class {
        &super::effect_mesh::CLASS
    }

    fn owned_group(&self) -> Option<OwnedGroup> {
        Some(OwnedGroup::HumanAffect(self.key, self.slot))
    }

    fn update(&mut self, tick: &Tick) {
        self.mesh.update(tick);
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        let Some(body) = (live.body)(self.key) else {
            self.gone = true;

            return;
        };

        if self.first {
            self.first = false;

            self.update(tick);

            return;
        }

        let phase = ((tick.now_ms % 5000) as f32 / 5000.0 + self.index as f32 / 5.0)
            * std::f32::consts::TAU;

        if self.index % 2 == 0 {
            self.mesh.yaw = phase;
        }

        if self.index % 3 == 1 {
            self.mesh.pitch = phase;
        }

        if body.appearance.is_some_and(|a| a.mounted) {
            if let Some(anchor) = body.skill_anchor {
                self.mesh.position = anchor;
            }

            self.mesh.scale_xz = 1.5;

            self.mesh.scale_y = 1.5;
        } else {
            self.mesh.position = body.position
                + Vec3::Y
                    * body
                        .dimensions
                        .map_or(0.0, |(_, height)| height * body.scale * 0.5);

            self.mesh.scale_xz = 1.0;

            self.mesh.scale_y = 1.0;
        }

        if body.class == 0x2C {
            self.mesh.texture_bias = 45;

            self.mesh.scale_xz = body.scale;

            self.mesh.scale_y = body.scale;

            self.mesh.position.y += body.scale * 5.1;
        }

        self.update(tick);
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn cull(&self, eye: Vec3, field: bool) -> Cull {
        self.mesh.cull(eye, field)
    }

    fn submit(&self, cx: &Submit<'_>, sink: &mut Sink<'_>) {
        if !self.gone {
            self.mesh.submit(cx, sink);
        }
    }
}

fn retained_meshes(actors: &ActorScene, effects: &mut BillboardScene, now: u32) {
    let enabled = |a: &super::actors::Actor| {
        actors.effects_detail.effects && !a.is_dead && (a.flags.get(0xB52) == 1 || a.class == 0x2C)
    };

    effects.tree.remove_where(
        Removal::Forced,
        &RemovalContext {
            now_ms: now,
            detail: 0,
            ground: None,
        },
        |node| {
            let Some(OwnedGroup::HumanAffect(key, 0x8D4 | 0x8D8)) = node.owned_group() else {
                return false;
            };

            !actors
                .actors
                .iter()
                .any(|a| ActorKey::of(a) == key && enabled(a))
        },
    );

    for actor in actors.actors.iter().filter(|a| enabled(a)) {
        let key = ActorKey::of(actor);

        let Some(anchor) = actor.skill_anchor else {
            continue;
        };

        for (index, (slot, colour)) in [(0x8D4, 0xFF99BBFF), (0x8D8, 0xFF00FFAA)]
            .into_iter()
            .enumerate()
        {
            let group = OwnedGroup::HumanAffect(key, slot);

            if effects
                .tree
                .iter()
                .any(|node| node.owned_group() == Some(group))
            {
                continue;
            }

            let mut mesh = super::effect_mesh::EffectMesh::new(12, colour, 0.0, 1, now);

            mesh.texture_bias = 0;

            mesh.position = anchor;

            effects.spawn(Box::new(OwnedMesh {
                key,
                slot,
                index,
                first: true,
                mesh,
                gone: false,
            }));
        }
    }
}

fn retained_rotors(actors: &ActorScene, effects: &mut BillboardScene, now: u32) {
    let enabled =
        |a: &super::actors::Actor| a.flags.get(0xB59) == 1 && !a.is_dead && a.flags.get(0xB3E) == 0;

    let hidden = |a: &super::actors::Actor| a.flags.get(0xB55) != 0 || a.flags.get(0xB3E) == 1;

    effects.tree.remove_where(
        Removal::Forced,
        &RemovalContext {
            now_ms: now,
            detail: 0,
            ground: None,
        },
        |node| {
            let Some(OwnedGroup::HumanAffect(key, 0x858 | 0x85C)) = node.owned_group() else {
                return false;
            };

            !actors
                .actors
                .iter()
                .any(|a| ActorKey::of(a) == key && (hidden(a) || enabled(a)))
        },
    );

    for actor in actors.actors.iter().filter(|a| enabled(a) && !hidden(a)) {
        let key = ActorKey::of(actor);

        for (index, slot) in [0x858, 0x85C].into_iter().enumerate() {
            let group = OwnedGroup::HumanAffect(key, slot);

            if effects
                .tree
                .iter()
                .any(|node| node.owned_group() == Some(group))
            {
                continue;
            }

            let mut rotor = super::mesh_fx::MeshRotate::new(
                actor.position + Vec3::Y * (0.8 + index as f32 * 0.2),
                1,
                true,
                false,
                now.wrapping_sub(index as u32 * 1000),
            )
            .follow(actor.entity);

            rotor.owner_key = Some(key);

            rotor.affect_slot = Some(slot);

            rotor.mesh_id = 2839;

            rotor.additive = true;

            rotor.scale = 1.4;

            rotor.set_color(0xFF999999);

            effects.spawn(Box::new(rotor));
        }
    }
}

impl Effect for OwnedQuad {
    fn class(&self) -> &'static Class {
        &super::quads::CLASS
    }

    fn owned_group(&self) -> Option<OwnedGroup> {
        Some(OwnedGroup::HumanAffect(self.key, self.slot))
    }

    fn update(&mut self, _tick: &Tick) {}

    fn human_owner_hidden(&mut self) {
        self.disabled = true;
    }

    fn update_live(&mut self, _tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        self.detail = live.detail;

        if let Some(body) = (live.body)(self.key) {
            self.quad.anchor = body.position + Vec3::Y * self.lift;
        } else {
            self.gone = true;
        }
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn cull(&self, eye: Vec3, field: bool) -> Cull {
        self.quad.cull(eye, field)
    }

    fn submit(&self, cx: &Submit<'_>, sink: &mut Sink<'_>) {
        if self.gone || self.disabled || (self.detail == 1 && self.mode != 4) {
            return;
        }

        self.quad.submit_coloured(
            cx,
            sink,
            None,
            Some(retained_tint(
                self.quad.argb,
                self.mode,
                self.quad.ordinal,
                cx.now_ms,
            )),
        );
    }
}

fn retained_tint(argb: u32, mode: i32, ordinal: usize, now: u32) -> u32 {
    let phase = now.wrapping_add((ordinal % 100) as u32 * 200) % 5000;

    let pulse = (phase as f32 / 5000.0 * std::f32::consts::TAU).sin().abs();

    let k = if mode == 2 {
        pulse * 0.2 + 0.8
    } else {
        pulse * 0.8 + 0.2
    };

    let byte = |shift: u32| ((((argb >> shift) & 255) as f32 * k).trunc() as u32) << shift;

    byte(24) | byte(16) | byte(8) | byte(0)
}

fn retained_quads(actors: &ActorScene, effects: &mut BillboardScene, now: u32) {
    effects.tree.remove_where(
        Removal::Forced,
        &RemovalContext {
            now_ms: now,
            detail: 0,
            ground: None,
        },
        |node| {
            let Some(OwnedGroup::HumanAffect(key, slot @ (0x830 | 0x834 | 0x838))) =
                node.owned_group()
            else {
                return false;
            };

            let flag = match slot {
                0x830 => 0xB37,
                0x834 => 0xB3A,
                _ => 0xB3E,
            };

            !actors.actors.iter().any(|actor| {
                ActorKey::of(actor) == key
                    && actor.flags.get(flag) == 1
                    && (slot != 0x838 || actors.viewer == Some(actor.entity))
            })
        },
    );

    for actor in &actors.actors {
        let key = ActorKey::of(actor);

        for (flag, slot, texture, mode, lift, size, period, argb) in [
            (0xB37, 0x830, 94, 2, 0.3, 1.5, 3000, 0x33333333),
            (0xB3A, 0x834, 93, 3, 0.4, 1.5, 5000, 0x88888800),
            (0xB3E, 0x838, 94, 4, 0.5, 1.2, 5000, 0xAAAA0000),
        ] {
            if actor.flags.get(flag) != 1 || (slot == 0x838 && actors.viewer != Some(actor.entity))
            {
                continue;
            }

            let group = OwnedGroup::HumanAffect(key, slot);

            if effects
                .tree
                .iter()
                .any(|node| node.owned_group() == Some(group))
            {
                continue;
            }

            effects.spawn(Box::new(OwnedQuad {
                key,
                slot,
                mode,
                lift,
                detail: 0,
                gone: false,
                disabled: false,
                quad: super::quads::GroundQuad {
                    owner: usize::from(actor.entity),
                    slot: super::quads::Slot::Placed,
                    texture,
                    pass: render::billboard::BillboardPass::Additive,
                    anchor: actor.position + Vec3::Y * lift,
                    base_scale: Vec3::splat(size),
                    grow: 0.0,
                    spin_period: period,
                    ordinal: usize::from(actor.entity),
                    argb,
                },
            }));
        }
    }
}

pub fn update(actors: &mut ActorScene, effects: &mut BillboardScene, now: u32) {
    let reduced = actors
        .effect_options
        .is_some_and(|options| !options.secondary)
        || !actors.effects_detail.glow;

    effects.old_skill_vfx = actors
        .effect_options
        .is_none_or(|options| options.old_skill_vfx);

    for actor in &mut actors.actors {
        super::body_sprites::owner_tick(actor, effects.scene_id);

        if actor.flags.get(0xB55) != 0 || actor.flags.invisible() {
            for hand in &mut actor.swings {
                hand.emitters = super::aura::Emitters::default();
            }
        } else if reduced {
            for hand in &mut actor.swings {
                hand.emitters.aura_mode = 0;

                hand.emitters.spark = false;

                hand.emitters.weapon_effect = 0;

                hand.emitters.panel = false;
            }
        }
    }

    effects
        .tree
        .control_humans(actors.actors.iter().filter_map(|actor| {
            use super::effect_tree::HumanControl;

            let control = if actor.flags.get(0xB55) != 0 || actor.flags.get(0xB3E) == 1 {
                HumanControl::OwnerHidden
            } else if reduced {
                HumanControl::QualityHidden
            } else {
                return None;
            };

            Some((ActorKey::of(actor), control))
        }));

    super::magic_shield::periodic(actors, effects, now);

    retained_quads(actors, effects, now);

    retained_meshes(actors, effects, now);

    retained_rotors(actors, effects, now);

    status_sprites(actors, effects, now);

    if actors.secondary_effects_enabled() {
        for actor in &actors.actors {
            if actor.flags.get(0xB55) != 0 || actor.flags.get(0xB3E) == 1 {
                continue;
            }

            if actor.flags.get(0xB53) != 1 {
                continue;
            }

            let height = super::pick::PICK_VOLUME
                .get(actor.model_id as usize)
                .map_or(0.0, |(_, height)| *height)
                * actor.scale;

            for particle in burning_pair(actor.position, height, &mut actors.effect_rng) {
                effects.spawn(Box::new(particle));
            }
        }
    }

    effects.billboards.retain(|billboard| {
        billboard.conceal_with.is_none_or(|key| {
            super::effect_context::actor(&actors.actors, key)
                .is_some_and(|actor| actor.flags.get(0xB55) == 0 && !actor.flags.invisible())
        })
    });

    let detail = actors.effects_detail.effects;

    effects
        .billboards
        .retain_mut(|billboard| match billboard.body_owned {
            Some((key, 0x820 | 0x824)) => actors
                .actors
                .iter()
                .find(|actor| ActorKey::of(actor) == key)
                .is_some_and(|actor| {
                    if reduced {
                        billboard.held_disabled = true;
                    }

                    if actor.flags.get(0xB55) != 0 || actor.flags.get(0xB3E) == 1 {
                        billboard.held_disabled = true;

                        true
                    } else {
                        detail && actor.flags.get(0xB3B) == 1
                    }
                }),
            _ => true,
        });

    if !detail {
        return;
    }

    for actor in &mut actors.actors {
        if actor.flags.get(0xB55) != 0 || actor.flags.get(0xB3E) == 1 {
            continue;
        }

        if actor.flags.get(0xB3B) != 1 {
            continue;
        }

        let key = ActorKey::of(actor);

        let Some(anchor) = actor.skill_anchor else {
            continue;
        };

        let refresh = now.wrapping_sub(actor.affect_pair_ms) > 500;

        let random = if refresh {
            actors.effect_rng.next() % 5
        } else {
            0
        };

        for (index, slot) in [0x820, 0x824].into_iter().enumerate() {
            let existing = effects
                .billboards
                .iter()
                .position(|b| b.body_owned == Some((key, slot)));

            let at = if let Some(at) = existing {
                at
            } else if refresh {
                let size = (random as f32 * 0.2 + 1.0) * if actor.mounted { 1.3 } else { 2.0 }
                    - index as f32 * 0.4;

                let mut billboard = Billboard::new(109, 0, Vec3::splat(size), 0.0, 8, 80);

                billboard.birth_ms = Some(now);

                billboard.colour_mode = 0;

                billboard.pass = render::billboard::BillboardPass::Additive;

                billboard.set_color(0xFFFFDD00);

                billboard.body_owned = Some((key, slot));

                effects.billboards.push(billboard);

                effects.billboards.len() - 1
            } else {
                continue;
            };

            let billboard = &mut effects.billboards[at];

            if refresh {
                billboard.roll = (now.wrapping_sub(billboard.birth_ms.unwrap_or(now)) % 1000)
                    as f32
                    * std::f32::consts::TAU
                    / 1000.0
                    + index as f32;
            }

            billboard.position = anchor + Vec3::Y * 1.2;
        }

        if refresh {
            actor.affect_pair_ms = now;
        }
    }
}

fn burning_pair(at: Vec3, height: f32, rng: &mut super::rng::Lcg) -> [Billboard; 2] {
    let r = rng.next() % 5;

    let n = r as f32;

    let mut ember = Billboard::new(
        56,
        1500,
        Vec3::new(n * 0.02 + 0.02, n * 0.05 + 0.02, n * 0.02 + 0.02),
        0.0,
        1,
        80,
    );

    ember.position = at
        + Vec3::new(
            (rng.next() % 10) as f32 * 0.07 - 0.35,
            height - n * 0.05,
            (rng.next() % 10) as f32 * 0.07 - 0.35,
        );

    ember.origin = ember.position;

    ember.rise = -1.2;

    ember.amp_h = 0.1;

    ember.motion = match r % 3 {
        0 => super::effects::Motion::Still,
        1 => super::effects::Motion::Rise,
        _ => super::effects::Motion::SwayX,
    };

    ember.pass = render::billboard::BillboardPass::Additive;

    ember.set_color(0xFFFF3300);

    let mut flame = Billboard::new(
        0,
        1500,
        Vec3::new(n * 0.1 + 0.1, n * 0.05 + 0.8, n * 0.1 + 0.1),
        0.0001,
        1,
        80,
    );

    flame.position = at
        + Vec3::new(
            (rng.next() % 10) as f32 * 0.1 - 0.5,
            0.0,
            (rng.next() % 10) as f32 * 0.1 - 0.5,
        );

    flame.origin = flame.position;

    flame.rise = 1.5;

    flame.motion = super::effects::Motion::Rise;

    flame.pass = render::billboard::BillboardPass::Additive;

    flame.set_color(0xFFFF0000);

    [ember, flame]
}

pub(crate) fn movement_afterimages(actors: &mut ActorScene, now: u32) {
    if !actors.secondary_effects_enabled() {
        return;
    }

    for index in 0..actors.actors.len() {
        let actor = &actors.actors[index];

        if actor.flags.get(0xB55) != 0
            || actor.flags.get(0xB3E) == 1
            || actor.flags.get(0xB3F) != 1
            || !matches!(actor.action, 2 | 3)
            || now.wrapping_sub(actor.afterimage_ms) <= 80
        {
            continue;
        }

        let Some(mut ghost) = actors.teleport_afterimage(index, now) else {
            continue;
        };

        ghost.start_ms = Some(now);

        ghost.lifetime_ms = 200;

        ghost.colour_a = [0.3; 3];

        ghost.yaw = actor.yaw;

        ghost.spin = actor.yaw;

        ghost.rig2 = None;

        ghost.mesh_id2 = None;

        if let Some(rig) = ghost.rig.as_mut() {
            rig.anim_speed_ms = actor.anim.sub_frame_ms;

            if actor.mounted && actor.mount.is_some() {
                let clip = actors
                    .registries
                    .sounds
                    .class(ghost.mesh_id as usize)
                    .and_then(|table| table.frame.get(3))
                    .and_then(|&clip| usize::try_from(clip).ok());

                if let Some(clip) = clip {
                    rig.animation.set_clip(clip, now, &actors.models[rig.model]);
                }
            }
        }

        actors.skinfx.spawn(ghost);

        actors.actors[index].afterimage_ms = now;
    }
}

pub(crate) fn haste_trails(world: &mut super::world::World, now: u32) {
    if world.scene != 30000 || !world.actors.secondary_effects_enabled() {
        return;
    }

    let reach = world.pose_reach();

    let viewer = world.actors.viewer;

    let hovered = world.actors.hovered;

    for actor in &mut world.actors.actors {
        if !reach.admits(actor.position)
            || (hovered == Some(actor.entity) && viewer != Some(actor.entity))
            || actor.flags.get(0xB2D) != 1
            || actor.action != 3
        {
            continue;
        }

        let Some(interval) = haste_period(actor.speed) else {
            continue;
        };

        if now.wrapping_sub(actor.haste_trail_ms) <= interval {
            continue;
        }

        actor.haste_trail_ms = now;

        world.effects.spawn(Box::new(haste_quad(actor.position)));
    }
}

fn haste_period(speed: f32) -> Option<u32> {
    (speed.is_finite() && speed > 0.0).then(|| (600.0 / speed) as u32)
}

fn haste_quad(position: Vec3) -> super::timers::TimedQuad {
    super::timers::TimedQuad::new(
        super::quads::GroundQuad {
            owner: 0,
            slot: super::quads::Slot::Placed,
            texture: 43,
            pass: render::billboard::BillboardPass::Additive,
            anchor: position + Vec3::Y * 0.2,
            base_scale: Vec3::splat(0.5),
            grow: 0.0005,
            spin_period: 0,
            ordinal: 0,
            argb: 0xFFFFFFFF,
        },
        1000,
    )
}

pub(crate) fn skill_afterimages(actors: &mut ActorScene, now: u32) -> bool {
    let mut sound = false;

    for index in 0..actors.actors.len() {
        let actor = &mut actors.actors[index];

        let Some(start) = actor.skill_afterimage_window else {
            continue;
        };

        if now.wrapping_sub(start) > 1500 || actor.flags.get(0xB30) != 1 {
            actor.flags.set(0xB30, 0);

            actor.skill_afterimage_window = None;

            continue;
        }

        if actor.flags.get(0xB55) != 0
            || actor.flags.get(0xB3E) == 1
            || now.wrapping_sub(actor.skill_afterimage_ms) <= 300
        {
            continue;
        }

        if let Some(ghost) = actors.skill_afterimage(index, now) {
            actors.skinfx.spawn(ghost);

            sound = true;
        }

        actors.actors[index].skill_afterimage_ms = now;
    }

    sound
}
