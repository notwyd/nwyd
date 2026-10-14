use super::{
    effect_tree::Effect,
    effects::{Billboard, Motion},
    world::World,
};
use glam::Vec3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]

pub struct State {
    pub variant: u8,
    pub started: Option<u32>,
}

fn family(body: i32) -> Option<u8> {
    match body % 10 {
        6 => Some(6),
        7 => Some(7),
        8 => Some(8),
        _ if matches!(body, 22..=25 | 32) => Some(8),
        9 => Some(9),
        _ => None,
    }
}

pub fn update(world: &mut World, now: u32) {
    for index in 0..world.actors.actors.len() {
        let actor = &mut world.actors.actors[index];

        if actor.transformation.started.is_none()
            && let Some(pending) = actor.pending.filter(|p| p.skill == 0x66)
        {
            actor.transformation.started = Some(now);

            build(world, index, pending, now);
        }

        let actor = &mut world.actors.actors[index];

        if actor.flags.get(0xB5A) == 1
            && actor
                .transformation
                .started
                .is_some_and(|start| now.wrapping_sub(start) > 1250)
        {
            actor.transformation.started = None;

            let at = actor.position + Vec3::Y * 2.0;

            for i in 0..3 {
                let n = i as f32;

                let mut b = particle(
                    56,
                    700,
                    Vec3::new(3.0 + 1.5 * n, 3.0 + 1.5 * n, 3.0 + 0.5 * n),
                    0.001,
                    at,
                    0xFFFFFFFF,
                );

                b.colour_mode = 1;

                world.effects.spawn(Box::new(b));
            }
        }
    }
}

fn particle(texture: usize, life: u32, scale: Vec3, grow: f32, at: Vec3, argb: u32) -> Billboard {
    let mut b = Billboard::new(texture, life, scale, grow, 1, 80);

    b.position = at;

    b.origin = at;

    b.pass = render::billboard::BillboardPass::Additive;

    b.set_color(argb);

    b
}

fn moving(mut b: Billboard, motion: Motion, speed: f32, lift: bool) -> Billboard {
    b.motion = motion;

    b.rise = speed;

    b.amp_h = speed;

    b.lift = lift;

    b
}

fn judgement(world: &mut World, at: Vec3, mode: i32, scale: f32, now: u32) {
    let built = super::skill::judgement::build_at(at, mode, scale, now);

    for quad in built.orphans {
        world.effects.spawn(Box::new(quad));
    }

    if let Some(mesh) = built.mesh {
        world.effects.spawn(Box::new(mesh));
    }

    world.effects.spawn(Box::new(built.token));
}

fn ghost(world: &mut World, index: usize, n: u32, many: bool, now: u32) {
    use super::skinfx::{Blend, ColourAnim, SkinConstruction, SkinInput, SkinMeshEffect};

    let actor = &world.actors.actors[index];

    let at = actor.position
        + Vec3::Y
            * if actor.mounted && actor.mount.is_some() {
                0.5
            } else {
                0.0
            };

    let mut skin = SkinMeshEffect::new(actor.model_id as i32, at, at, 0);

    skin.place(at);

    skin.colour_a = [if many { 0.3 + 0.8 * n as f32 } else { 0.3 }; 3];

    skin.colour_anim = ColourAnim::Cosine;

    skin.blend = Blend::Additive;

    skin.lifetime_ms = if many { 100 + 300 * n } else { 1400 };

    skin.yaw = actor.yaw + if many { 1.5 * n as f32 } else { 0.0 };

    skin.spin = 1.0;

    let divisor = actor
        .anim
        .sub_frame_ms
        .wrapping_sub(if many { 4 * n } else { 0 });

    let clip = world
        .actors
        .registries
        .sounds
        .class(actor.model_id as usize)
        .and_then(|table| table.frame.get(if many { 4 } else { 14 }))
        .and_then(|&clip| usize::try_from(clip).ok());

    skin.construction = Some(SkinConstruction {
        primary: SkinInput {
            model_id: actor.model_id as i32,
            parts: actor.parts,
            pose: None,
            animation: None,
            clip_action: None,
            scale: if many { 1.9 } else { 2.0 },
        },
        secondary: None,
        cape: matches!(actor.class, 4 | 8),
        body: None,
    });

    match world.actors.hydrate_skin(&mut skin, now) {
        Ok(()) => {
            if let Some(rig) = skin.rig.as_mut() {
                rig.anim_speed_ms = divisor;

                if let Some(clip) = clip {
                    rig.animation
                        .set_clip(clip, now, &world.actors.models[rig.model]);
                }
            }

            world.actors.skinfx.spawn(skin);
        }
        Err(error) => {
            tracing::warn!(
                skill = 0x66,
                index,
                ?error,
                "transformation ghost could not be hydrated"
            )
        }
    }
}

fn build(world: &mut World, index: usize, pending: super::skill_fx::Pending, now: u32) {
    let actor = &world.actors.actors[index];

    let Some(family) = family(actor.body) else {
        return;
    };

    let variant = actor.transformation.variant;

    let at = actor.position;

    let yaw = actor.yaw;

    let key = super::effect_context::ActorKey::of(actor);

    let mounted = actor.mounted;

    let anchor = actor.skill_anchor.unwrap_or(at);

    let height = super::pick::PICK_VOLUME
        .get(actor.model_id as usize)
        .map_or(0.0, |(_, h)| *h)
        * actor.scale;

    match (family, variant) {
        (6, 1) | (9, 3) => {
            let wide = family == 9;

            let count = if wide { 7 } else { 4 };

            for i in 0..count {
                let point = at
                    + Vec3::Y
                        * if wide {
                            0.5 + 0.3 * i as f32
                        } else {
                            0.5 * i as f32
                        };

                let mut arrow = super::arrow::Arrow::new(
                    point,
                    point,
                    if wide { pending.variant as i32 } else { 0 },
                    if wide { 10003 } else { 10002 },
                    0,
                    0,
                    0,
                    now,
                );

                arrow.birth_ms = now.wrapping_add(i * if wide { 100 } else { 150 });

                world.effects.spawn(Box::new(arrow));
            }
        }
        (6, 2) => {
            for i in 0..16 {
                let mut mesh = super::effect_mesh::EffectMesh::new(2838, 0x33555555, yaw, 3, now);

                mesh.texture_bias = 413;

                mesh.lifetime_ms = 1000 + 30 * i;

                mesh.period_ms = 1000 - 30 * i;

                mesh.position = if mounted {
                    anchor + Vec3::Y * (height * 0.5 - 0.3)
                } else {
                    at + Vec3::Y * (height * 0.5 + 0.3)
                };

                mesh.blend = 1;

                mesh.pulse = true;

                mesh.scale_xz = 1.0 + 0.3 * i as f32;

                mesh.scale_y = mesh.scale_xz;

                world.effects.spawn(Box::new(mesh));
            }
        }
        (6, 3) => {
            for life in [1000, 1500, 2000, 1500] {
                let mut blade = super::freeze_blade::FreezeBlade::new(
                    pending.to,
                    1,
                    false,
                    true,
                    &mut world.actors.effect_rng,
                    now,
                );

                blade.duration_ms = life;

                for child in blade.take_spawned() {
                    world.effects.spawn(child);
                }

                world.effects.spawn(Box::new(blade));
            }
        }
        (7, 1) => {
            ghost(world, index, 0, false, now);

            let b = particle(0, 1300, Vec3::splat(7.0), 0.0001, at + Vec3::Y, 0xFFFFFFFF);

            world
                .effects
                .spawn(Box::new(moving(b, Motion::SwayX, 0.05, true)));
        }
        (7, 2) => {
            for i in 0..3 {
                let n = i as f32;

                let mut b = particle(
                    33,
                    1000,
                    Vec3::splat(2.0 + 1.5 * n),
                    0.0,
                    pending.to + Vec3::Y * 2.0,
                    0xFFFFFFFF,
                );

                b.frames = 9;

                b.frame_ms = 110;

                b.colour_mode = 0;

                world.effects.spawn(Box::new(b));

                let b = particle(
                    0,
                    1500,
                    Vec3::new(4.0 + n, 4.0 + 2.0 * n, 4.0 + n),
                    0.0001,
                    at + Vec3::Y * 2.0,
                    0xFFFF5555,
                );

                world
                    .effects
                    .spawn(Box::new(moving(b, Motion::Rise, 0.08, true)));

                world.effects.spawn(Box::new(super::timers::TimedQuad::new(
                    super::quads::GroundQuad {
                        owner: 0,
                        slot: super::quads::Slot::Placed,
                        texture: 8,
                        pass: render::billboard::BillboardPass::Additive,
                        anchor: pending.to + Vec3::Y * (0.5 + 0.1 * n),
                        base_scale: Vec3::splat(0.0005),
                        grow: 0.005,
                        spin_period: 0,
                        ordinal: 0,
                        argb: 0xFFFF5555,
                    },
                    1500,
                )));
            }
        }
        (7, 3) => {
            for i in 0..8 {
                let n = i as f32;

                let b = particle(
                    56,
                    1000 + 100 * i,
                    Vec3::new(1.5 + 0.5 * n, 1.5 + 0.4 * n, 1.5 + 0.5 * n),
                    0.0,
                    at + Vec3::Y * (5.0 - 0.9 * n),
                    0xFFFF3300,
                );

                world
                    .effects
                    .spawn(Box::new(moving(b, Motion::Follow, 1.0 + 2.0 * n, true)));

                let b = particle(
                    0,
                    1000,
                    Vec3::splat(5.0),
                    0.0001,
                    at + Vec3::Y * 2.0,
                    0xFFFF0000,
                );

                world
                    .effects
                    .spawn(Box::new(moving(b, Motion::Rise, 5.0, false)));
            }
        }
        (8, 1) => {
            for i in 0..10 {
                let n = i as f32;

                judgement(world, at + Vec3::Y * (-2.0 + 0.5 * n), 7, 0.1 * n, now);
            }
        }
        (8, 2) => {
            for i in 0..5 {
                let n = i as f32;

                judgement(world, at + Vec3::Y * (0.5 - 0.1 * n), 8, 0.4 * n, now);

                let b = particle(
                    56,
                    2300,
                    Vec3::new(0.3 + 1.7 * n, 0.3 + 0.5 * n, 0.3 + 1.7 * n),
                    0.0,
                    at + Vec3::Y * (-0.5 + 0.7 * n),
                    0x33555555,
                );

                world
                    .effects
                    .spawn(Box::new(moving(b, Motion::Follow, 1.0 + n, true)));
            }
        }
        (8, 3) => {
            for i in 0..6 {
                ghost(world, index, i, true, now);
            }
        }
        (9, 1) => {
            for i in 0..2 {
                let mut rotor = super::mesh_fx::MeshRotate::new(
                    at - Vec3::Y * (3.0 + 1.5 * i as f32),
                    2,
                    true,
                    false,
                    now,
                );

                rotor.owner_key = Some(key);

                rotor.position = if mounted {
                    Vec3::new(anchor.x, at.y - 3.0, anchor.z)
                } else {
                    at - Vec3::Y * 3.0
                };

                rotor.scale = 6.5;

                rotor.life_ms = 1000 - 100 * i;

                rotor.additive = true;

                rotor.set_color(0xFF5555AA);

                world.effects.spawn(Box::new(rotor));
            }
        }
        (9, 2) => {
            let ground = super::shade::Ground::new(
                &world.terrain,
                &world.actors.heights,
                world.streamed.iter().map(|zone| &zone.terrain).collect(),
            );

            for i in 0..3 {
                let mut built =
                    super::sp_change::build(at + Vec3::Y * (0.7 * i as f32), 1, yaw, now);

                built.emitter.owner = Some(key);

                built.emitter.lifetime_ms = 700 + 300 * i;

                built.emitter.place(&ground);

                world.effects.spawn(Box::new(built.emitter));
            }
        }
        _ => {}
    }
}
