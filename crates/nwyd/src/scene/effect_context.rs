use super::actors::{Actor, Anim, Model};
use super::rng::Lcg;
use super::shade::Ground;
use formats::mesh::appearance::{self, PartRef};
use formats::vfx::Body;
use glam::Vec3;

mod owners;

pub(crate) use owners::OwnerIndex;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ActorKey {
    pub entity: u16,
    incarnation: u64,
}

pub(crate) fn next_incarnation() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT: AtomicU64 = AtomicU64::new(1);

    NEXT.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
        .expect("actor allocation identity exhausted")
}

impl ActorKey {
    #[cfg(test)]
    pub(crate) fn synthetic(entity: u16) -> Self {
        Self {
            entity,
            incarnation: next_incarnation(),
        }
    }

    pub(crate) fn of(actor: &Actor) -> Self {
        Self {
            entity: actor.entity,
            incarnation: actor.incarnation,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Appearance {
    pub model_id: u32,
    pub parts: [PartRef; appearance::PARTS],
    pub scale: f32,
    pub mounted: bool,
    pub mount_model: Option<u32>,
    pub mount_scale: f32,
    pub animation: Anim,
    pub afterimage_clip: Option<usize>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Snapshot {
    pub key: ActorKey,
    pub position: Vec3,
    pub yaw: f32,
    pub class: i32,
    pub model_id: u32,
    pub scale: f32,
    pub part0_mesh: i16,
    pub movement_origin: [f32; 2],
    pub destination: (i32, i32),
    pub appearance: Option<Appearance>,
    pub dimensions: Option<(f32, f32)>,
    pub skill_anchor: Option<Vec3>,
    pub critical: bool,
    pub skill_child: Option<u16>,
    pub primary_tier: u8,
    pub affect_29: bool,
    pub effects_hidden: bool,
    pub appearance_epoch: u32,
    pub hand_glow_points: [Option<Vec3>; 2],
}

pub fn snapshot(actor: &Actor, models: &[Model]) -> Snapshot {
    Snapshot {
        key: ActorKey::of(actor),
        position: actor.position,
        yaw: actor.yaw,
        class: actor.class,
        movement_origin: actor.mover.turn_origin,
        model_id: actor.model_id,
        scale: actor.scale,
        part0_mesh: actor.parts[0].mesh,
        destination: actor.destination,
        skill_anchor: actor.skill_anchor,
        critical: actor.skill_critical,
        skill_child: actor.skill_child,
        primary_tier: actor.primary_tier,
        affect_29: actor.flags.get(formats::net::affect::Flags::LAST) == 1,
        effects_hidden: actor.flags.get(0xB55) != 0 || actor.flags.get(0xB3E) == 1,
        appearance_epoch: actor.appearance_epoch,
        hand_glow_points: if actor.flags.get(0xB41) != 0 {
            [actor.body_points.points[6], actor.body_points.points[7]]
        } else {
            [None; 2]
        },
        dimensions: models.get(actor.model).and_then(|model| {
            super::pick::PICK_VOLUME
                .get(model.model_id as usize)
                .copied()
        }),
        appearance: models.get(actor.model).map(|model| Appearance {
            model_id: model.model_id,
            parts: actor.parts,
            scale: actor.scale,
            mounted: actor.mounted,
            mount_model: actor.mount_model,
            mount_scale: actor.mount_scale,
            animation: actor.anim,
            afterimage_clip: None,
        }),
    }
}

#[derive(Debug, Clone)]
pub struct Construction {
    pub caster: Snapshot,
    pub target: Option<Snapshot>,
    pub bystander: Option<Snapshot>,
    pub victims: Vec<Option<Snapshot>>,
    pub viewer: Option<Snapshot>,
    pub camera: super::world::Pose,
    pub scene: u32,
    pub world: u64,
    pub zone: (u8, u8),
    pub poses: Vec<PosedBody>,
    pub producer: Option<(formats::vfx::Chain, u8)>,
    pub cannon: Option<Cannon>,
}

#[derive(Debug, Clone)]
pub struct PosedBody {
    pub key: ActorKey,
    pub body: super::skinfx::BodySnapshot,
    pub appearance: Appearance,
    pub mount_parts: Option<[PartRef; appearance::MOUNT_PARTS]>,
    pub mount_position: Option<Vec3>,
    pub mount_animation: Option<Anim>,
    pub mount_run_clip: Option<usize>,
    pub held: Vec<HeldInput>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HeldInput {
    pub hand: appearance::Hand,
    pub mesh_id: u32,
    pub bone: i32,
    pub world: Option<render::camera::Matrix4>,
}

impl PosedBody {
    pub fn of(actor: &Actor, models: &[Model]) -> Option<Self> {
        let appearance = snapshot(actor, models).appearance?;

        let mount = actor
            .mount
            .as_ref()
            .filter(|mount| actor.mounted && models.get(mount.model).is_some());

        let held = actor
            .held
            .iter()
            .map(|held| {
                let world = usize::try_from(held.bone).ok().and_then(|bone| {
                    actor.tree.slot(bone)?;

                    let world = held.world(&render::camera::Matrix4(actor.tree.combined(bone)));

                    world.0.iter().all(|v| v.is_finite()).then_some(world)
                });

                HeldInput {
                    hand: held.hand,
                    mesh_id: held.mesh_id,
                    bone: held.bone,
                    world,
                }
            })
            .collect();

        let mut body = super::skinfx::BodySnapshot::of(actor);

        if mount.is_none() {
            body.mount = None;
        }

        Some(Self {
            key: ActorKey::of(actor),
            body,
            appearance,
            mount_parts: mount.map(|mount| mount.parts),
            mount_position: mount.map(|_| actor.position),
            mount_animation: mount.map(|mount| mount.anim),
            mount_run_clip: mount.and(actor.clips.mount_run).map(|clip| clip.slot),
            held,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cannon {
    pub entity: u16,
    pub position: Vec3,
    pub yaw: f32,
    pub mesh: Option<i32>,
}

impl Construction {
    pub fn body(&self, body: Body) -> Option<&Snapshot> {
        match body {
            Body::Caster => Some(&self.caster),
            Body::Target => self.target.as_ref(),
            Body::Bystander => self.bystander.as_ref(),
        }
    }

    pub fn pose(&self, key: ActorKey) -> Option<&PosedBody> {
        self.poses.iter().find(|pose| pose.key == key)
    }

    pub fn capture(
        scene: &super::field::FieldScene,
        caster: usize,
        target: Option<u16>,
        victims: &[formats::net::attack::Hit],
    ) -> Self {
        let actors = &scene.world.actors;

        let capture_body = |actor: &Actor| {
            let mut body = snapshot(actor, &actors.models);

            if let Some(look) = body.appearance.as_mut() {
                look.afterimage_clip = actors
                    .anim_slots(actor.model_id)
                    .and_then(|slots| {
                        Model::from_action(
                            &actors.registries.sounds,
                            &slots,
                            actor.model_id,
                            actor.group,
                            actor.action,
                        )
                    })
                    .map(|(slot, _)| slot);
            }

            body
        };

        let resolve = |id| actors.entity(id).map(|i| capture_body(&actors.actors[i]));

        let caster = capture_body(&actors.actors[caster]);

        let target = target.and_then(resolve);

        let viewer = actors.viewer.and_then(resolve);

        let victims: Vec<_> = victims.iter().map(|hit| resolve(hit.entity)).collect();

        let mut poses = Vec::new();

        for body in std::iter::once(Some(caster))
            .chain([target, viewer])
            .chain(victims.iter().copied())
            .flatten()
        {
            if !poses.iter().any(|pose: &PosedBody| pose.key == body.key)
                && let Some(actor) = actor(&actors.actors, body.key)
                && let Some(mut pose) = PosedBody::of(actor, &actors.models)
            {
                pose.appearance = body
                    .appearance
                    .expect("PosedBody requires a loaded appearance");

                poses.push(pose);
            }
        }

        let cannon = cannon_at(scene, caster.position);

        Self {
            caster,
            target,
            bystander: viewer.filter(|body| body.key != caster.key),
            victims,
            viewer,
            camera: scene.world.pose,
            scene: scene.world.scene,
            world: scene.world.identity,
            zone: actors.zone,
            poses,
            producer: None,
            cannon,
        }
    }

    pub fn projectile_source(
        &self,
        muzzle: Option<Vec3>,
        mode: i32,
        suppressed: bool,
    ) -> Option<Vec3> {
        if suppressed {
            return None;
        }

        if mode == 0 {
            let look = self.caster.appearance?;

            if !(look.model_id == 0x14 && matches!(look.parts[1].mesh, 0 | 2))
                && self.caster.class != 0x23
            {
                return None;
            }
        } else if mode != 1 {
            return None;
        }

        let source = match muzzle {
            Some(point) if point != Vec3::ZERO => point,
            _ => self.caster.position + Vec3::Y * 1.5,
        };

        source.is_finite().then_some(source)
    }
}

pub fn cannon_at(scene: &super::field::FieldScene, source: Vec3) -> Option<Cannon> {
    (0x3A99..0x3AFD).find_map(|entity| {
        let drop = scene.drops.get(entity)?;

        (drop.item.index == 0x2EA
            && drop.position.x.is_finite()
            && drop.position.z.is_finite()
            && drop.position.x.trunc() == source.x.trunc()
            && drop.position.z.trunc() == source.z.trunc())
        .then_some(Cannon {
            entity,
            position: drop.position,
            yaw: drop.yaw,
            mesh: drop.mesh,
        })
    })
}

pub struct Live<'a> {
    pub mesh: Option<&'a dyn Fn(i32) -> bool>,
    pub body: &'a dyn Fn(ActorKey) -> Option<Snapshot>,
    pub bone: &'a dyn Fn(ActorKey, usize) -> Option<render::camera::Matrix4>,
    pub height: &'a dyn Fn(f32, f32) -> Option<f32>,
    pub ground: Option<&'a Ground<'a>>,
    pub camera: super::world::Pose,
    pub scene: u32,
    pub world: u64,
    pub zone: (u8, u8),
    pub viewer: Option<Snapshot>,
    pub detail: i32,
    pub rate: f32,
    pub rng: &'a mut Lcg,
}

pub fn actor(actors: &[Actor], key: ActorKey) -> Option<&Actor> {
    actors.iter().find(|actor| ActorKey::of(actor) == key)
}

pub fn skill_anchor(actor: &Actor, models: &[Model]) -> Option<Vec3> {
    models.get(actor.model)?;

    let point = match actor.mount.as_ref().filter(|_| actor.mounted) {
        None => actor.position,
        Some(mount) => {
            models.get(mount.model)?;

            if !actor.mount_scale.is_finite() || actor.mount_scale == 0.0 {
                return None;
            }

            let anchor = mount.anchor()?;

            let [x, y] = mount.seat.offset;

            anchor.transform_coord(Vec3::new(x / actor.mount_scale, y / actor.mount_scale, 0.0))
                - Vec3::Y * actor.scale * 0.4
        }
    };

    point.is_finite().then_some(point)
}

pub fn skin_owner_points(actor: &Actor, models: &[Model]) -> Option<(Vec3, Vec3)> {
    models.get(actor.model)?;

    let (a, b) = match actor.model_id {
        0 | 1 | 54 | 56 | 57 => (7, 29),
        20 => (9, 13),
        8 => (34, 35),
        2 => (27, 32),
        4 => (25, 31),
        7 => (7, 12),
        38 => (16, 11),
        31 => (19, 24),
        _ => return None,
    };

    let point = |bone| {
        actor.tree.slot(bone)?;

        let local = if actor.model_id == 31 {
            Vec3::new(0.3, 0.1, 0.0)
        } else {
            Vec3::ZERO
        };

        let point = render::camera::Matrix4(actor.tree.combined(bone)).transform_coord(local);

        point.is_finite().then_some(point)
    };

    Some((point(a)?, point(b)?))
}

pub fn projectile_muzzle(actor: &Actor, models: &[Model]) -> Option<Vec3> {
    models.get(actor.model)?;

    let (bone, local) = match actor.model_id {
        0 | 1 | 54 | 56 | 57 => (8, Vec3::new(0.4, 0.2, 0.0)),
        20 => (8, Vec3::ZERO),
        8 | 38 => (33, Vec3::ZERO),
        2 => (8, Vec3::new(0.0, 0.2, 0.0)),
        4 => (8, Vec3::new(-0.5, 0.0, 0.0)),
        29 => (8, Vec3::ZERO),
        26 => (14, Vec3::new(0.25, 0.15, 0.0)),
        7 | 25 => (6, Vec3::ZERO),
        28 => (10, Vec3::ZERO),
        22 => (25, Vec3::ZERO),
        6 => (18, Vec3::new(0.0, 0.05, 0.05)),
        31 => (14, Vec3::new(0.3, 0.1, 0.0)),
        _ => return None,
    };

    actor.tree.slot(bone)?;

    let point = render::camera::Matrix4(actor.tree.combined(bone)).transform_coord(local);

    point.is_finite().then_some(point)
}

pub fn bone(
    actors: &[Actor],
    models: &[Model],
    key: ActorKey,
    bone: usize,
) -> Option<render::camera::Matrix4> {
    actor_bone(actor(actors, key)?, models, bone)
}

fn actor_bone(actor: &Actor, models: &[Model], bone: usize) -> Option<render::camera::Matrix4> {
    models.get(actor.model)?;

    actor.tree.slot(bone)?;

    let matrix = render::camera::Matrix4(actor.tree.combined(bone));

    matrix
        .0
        .iter()
        .all(|value| value.is_finite())
        .then_some(matrix)
}

pub fn height(window: &[i8], zone: (u8, u8), x: f32, z: f32) -> Option<f32> {
    if window.len() != 256 * 256 || !x.is_finite() || !z.is_finite() {
        return None;
    }

    let (ox, oz) = (f32::from(zone.0) * 128.0, f32::from(zone.1) * 128.0);

    Some(
        formats::world::path::HeightField::window(window, zone).ground(
            x.clamp(ox, ox + 255.0) as i32,
            z.clamp(oz, oz + 255.0) as i32,
        ),
    )
}
