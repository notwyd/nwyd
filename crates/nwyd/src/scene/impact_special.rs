use super::{effect_context::Construction, effect_tree::Effect, field::FieldScene};
use glam::Vec3;

pub fn raise(
    scene: &mut FieldScene,
    attack: &formats::net::attack::Attack,
    context: &Construction,
    now: u32,
) -> usize {
    if !matches!(attack.skill_id, 0x34 | 0x63 | 0x64 | 0x71..=0x75 | 0x78) {
        return 0;
    }

    let local = context
        .viewer
        .is_some_and(|body| body.key == context.caster.key);

    if local && !attack.echo && attack.action != 0xFE {
        return 0;
    }

    if super::impact_particles::alternate_spark(context, attack.skill_id, attack.action) {
        return 0;
    }

    if matches!(attack.skill_id, 0x71..=0x74)
        && (attack.skill_id != 0x73 || context.caster.class != 0x49)
    {
        if let Some(index) = scene
            .world
            .actors
            .actors
            .iter()
            .position(|a| super::effect_context::ActorKey::of(a) == context.caster.key)
        {
            scene.world.actors.play_at(index, 7, now);
        }
    }

    if matches!(attack.skill_id, 0x73 | 0x74) {
        return 0;
    }

    let refreshed = if attack.skill_id == 0x34 && attack.action == 0xFE {
        context.target.map_or(0, |body| {
            super::magic_shield::refresh(&mut scene.world.effects, &body, 3, now)
        })
    } else {
        0
    };

    let ground = super::shade::Ground::new(
        &scene.world.terrain,
        &scene.world.actors.heights,
        scene
            .world
            .streamed
            .iter()
            .map(|zone| &zone.terrain)
            .collect(),
    );

    let mut out: Vec<Box<dyn Effect>> = Vec::new();

    let mut sounds = Vec::new();

    let mut skins = Vec::new();

    let detail = i32::from(!scene.world.actors.effects_detail.effects);

    match attack.skill_id {
        0x64 => sounds.push(156),
        0x63 => {
            let Some(target) = context.target else {
                return 0;
            };

            match super::skin_inputs::impact_sink(context) {
                Ok(skin) => skins.push(skin),
                Err(error) => {
                    tracing::warn!(
                        skill = 0x63,
                        ?error,
                        "copied impact body lacks construction inputs"
                    )
                }
            }

            for i in -1..2 {
                let mut mesh = super::effect_mesh::EffectMesh::new(506, 0xAAAAAAAA, 0.0, 0, now);

                mesh.blend = 1;

                mesh.scale_xz = 1.5;

                mesh.scale_y = 3.0;

                mesh.pulse = true;

                mesh.lifetime_ms = 3000;

                mesh.period_ms = 3000;

                mesh.position = target.position + Vec3::new(i as f32 * 0.2, -0.5, i as f32 * 0.2);

                out.push(Box::new(mesh));
            }

            let mut shade = super::shade::Shade::new(
                4,
                118,
                0xAAAAAAAA,
                target.position,
                super::effects::SECTOR_UNASSIGNED,
            )
            .with_lifetime(8000);

            shade.place(&ground);

            out.push(Box::new(shade));

            sounds.push(1);
        }
        0x34 => {
            let source = context.caster.position + Vec3::Y;

            if attack.action == 0xFE {
                let Some(target) = context.target else {
                    return 0;
                };

                let mut slash = super::slow_slash::SlowSlash::new(
                    source,
                    target.position,
                    2,
                    Some(target.key.entity),
                    now,
                );

                slash.target_key = Some(target.key);

                out.push(Box::new(slash));

                let built = super::skill::level_up::build(
                    &super::skill::level_up::Order {
                        at: formats::vfx::PositionRef::Target,
                        kind: 2,
                    },
                    target.position,
                    now,
                    &mut scene.world.actors.effect_rng,
                    detail,
                    Some(&ground),
                );

                out.extend(built.into_effects());

                sounds.push(156);
            } else {
                let dest = if let Some(target) = context.target {
                    target.position
                } else {
                    let (x, z) = (attack.to.0 as f32 + 0.5, attack.to.1 as f32 + 0.5);

                    let Some(y) = super::effect_context::height(
                        &scene.world.actors.window,
                        scene.world.actors.zone,
                        x,
                        z,
                    ) else {
                        tracing::warn!(
                            skill = 0x34,
                            x,
                            z,
                            "spike chain requires a loaded height window"
                        );

                        return 0;
                    };

                    Vec3::new(x, y, z)
                };

                let step = (dest - source) / 7.0;

                let mut blade = super::freeze_blade::FreezeBlade::new(
                    source + step,
                    2,
                    false,
                    false,
                    &mut scene.world.actors.effect_rng,
                    now,
                );

                blade.step = (step.x, step.z);

                out.extend(blade.take_spawned());

                out.push(Box::new(blade));
            }
        }
        0x78 => {
            let Some(target) = context.target else {
                return 0;
            };

            let caster = context.caster.position;

            out.push(Box::new(super::spark::Spark::new(
                caster + Vec3::Y,
                None,
                target.position + Vec3::Y,
                0xFF5555FF,
                0xFF222299,
                60000,
                1.0,
                0.0,
                now,
                now,
            )));

            for _ in 0..4 {
                out.push(Box::new(quad(caster + Vec3::Y * 1.1, 56, 1000, 0.01, 10)));
            }

            for (at, life) in [
                (caster, 1000),
                (target.position, 2000),
                (target.position, 60000),
            ] {
                let mut shade = super::shade::Shade::new(
                    4,
                    7,
                    0xFFFFFFFF,
                    at,
                    super::effects::SECTOR_UNASSIGNED,
                )
                .with_lifetime(life);

                shade.place(&ground);

                out.push(Box::new(shade));
            }

            let mut bolt = super::skill::thunderbolt::build_at(target.position, 0, now);

            bolt.place(&ground);

            let child = &bolt.poison;

            let poison = super::skill::poison::build(
                &super::skill::poison::Recipe {
                    at: child.at,
                    argb: child.argb.unwrap_or(0xFFFFFFFF),
                    count: child.count,
                    play_sound: child.play_sound,
                    style: child.style,
                },
                now,
                &mut scene.world.actors.effect_rng,
                detail,
            );

            sounds.extend(poison.sound);

            let mut children = Vec::new();

            super::skill::poison::push(poison, &mut children);

            out.extend(bolt.into_nodes(children));

            sounds.push(i64::from(super::skill::thunderbolt::SOUND));

            out.push(Box::new(quad(caster + Vec3::Y * 0.3, 2, 1500, 3.0, 2)));
        }
        0x75 => {
            let Some(target) = context.target else {
                return 0;
            };

            let [x, z] = target.movement_origin;

            let mut boom = super::fire_ring::FireRing::new(
                Vec3::new(x, 0.0, z),
                0,
                1.5,
                210,
                0xFFFFFFFF,
                now,
                Some(&ground),
            );

            out.extend(boom.take_spawned());

            out.push(Box::new(boom));
        }
        0x71 => {
            let mut shade = super::shade::Shade::new(
                70,
                118,
                0xAA660000,
                context.caster.position,
                super::effects::SECTOR_UNASSIGNED,
            )
            .with_lifetime(3200);

            shade.place(&ground);

            out.push(Box::new(shade));

            for victim in context.victims.iter().flatten() {
                let at = victim.position + Vec3::Y;

                out.push(Box::new(super::freeze_blade::FreezeBlade::new(
                    at,
                    8,
                    true,
                    false,
                    &mut scene.world.actors.effect_rng,
                    now,
                )));

                let built = super::skill::poison::build(
                    &super::skill::poison::Recipe {
                        at,
                        argb: 0xFF33FF66,
                        count: 10,
                        play_sound: 1,
                        style: 0,
                    },
                    now,
                    &mut scene.world.actors.effect_rng,
                    detail,
                );

                sounds.extend(built.sound);

                super::skill::poison::push(built, &mut out);
            }
        }
        0x72 => {
            for victim in context.victims.iter().flatten() {
                let at = victim.position;

                let built =
                    super::skill::judgement::build_at(Vec3::new(at.x, 1.0, at.z), 2, 0.1, now);

                for child in built.orphans {
                    out.push(Box::new(child));
                }

                if let Some(mesh) = built.mesh {
                    out.push(Box::new(mesh));
                }

                out.push(Box::new(built.token));

                for i in 0..4 {
                    let at =
                        at + Vec3::new(((i & 1) * 2) as f32 - 1.0, -5.0, (i & !1) as f32 - 1.0);

                    let mut blade = super::freeze_blade::FreezeBlade::new(
                        at,
                        1,
                        false,
                        false,
                        &mut scene.world.actors.effect_rng,
                        now,
                    );

                    out.extend(blade.take_spawned());

                    out.push(Box::new(blade));
                }

                let dest = at + Vec3::Y * 0.5;

                let mut arrow = super::arrow::Arrow::new(
                    dest + Vec3::new(3.0, 5.0, -3.0),
                    dest,
                    0,
                    10001,
                    0,
                    0,
                    0,
                    now,
                );

                arrow.variant = 0xFFFFFF00u32 as i32;

                out.push(Box::new(arrow));
            }
        }
        _ => unreachable!(),
    }

    let mut count = out.len() + refreshed;

    for effect in out {
        scene.world.effects.spawn(effect);
    }

    for mut skin in skins {
        match scene.world.actors.hydrate_skin(&mut skin, now) {
            Ok(()) => {
                scene.world.actors.skinfx.spawn(skin);

                count += 1;
            }
            Err(error) => {
                tracing::warn!(
                    skill = attack.skill_id,% error, "impact rig hydration failed"
                )
            }
        }
    }

    for sound in sounds {
        scene.world.play_sound(sound);
    }

    count
}

fn quad(at: Vec3, texture: usize, life: u32, scale: f32, spin: u32) -> super::timers::TimedQuad {
    super::timers::TimedQuad::new(
        super::quads::GroundQuad {
            owner: 0,
            slot: super::quads::Slot::Placed,
            texture,
            pass: render::billboard::BillboardPass::Additive,
            anchor: at,
            base_scale: Vec3::splat(scale),
            grow: 0.03,
            spin_period: spin,
            ordinal: 0,
            argb: 0xFFFFFFFF,
        },
        life,
    )
}
