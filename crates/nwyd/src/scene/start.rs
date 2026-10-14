use super::effect_context::ActorKey;
use super::effect_tree::Effect;
use super::mesh_fx::Start;
use glam::Vec3;

fn ring(position: Vec3, mode: i32, owner: Option<ActorKey>, now: u32) -> Start {
    let mut ring = Start::new(position, mode, None, now);

    ring.owner_key = owner;

    ring
}

pub fn spawn(
    position: Vec3,
    mode: i32,
    owner: Option<ActorKey>,
    now: u32,
    out: &mut Vec<Box<dyn Effect>>,
) {
    if mode == 0 {
        out.push(Box::new(child(position + Vec3::Y * 0.05)));
    }

    out.push(Box::new(ring(position, mode, owner, now)));
}

fn child(position: Vec3) -> super::timers::TimedQuad {
    super::timers::TimedQuad::new(
        super::quads::GroundQuad {
            owner: 0,
            slot: super::quads::Slot::Placed,
            texture: 1,
            pass: render::billboard::BillboardPass::Additive,
            anchor: position,
            base_scale: Vec3::splat(0.5),
            grow: 0.002,
            spin_period: 0,
            ordinal: 0,
            argb: 0xFFFFFFFF,
        },
        2000,
    )
}

pub fn vanishing(scene: &mut super::field::FieldScene, index: usize, now: u32, instant: bool) {
    let actor = &scene.world.actors.actors[index];

    if actor.flags.get(0xB55) != 0 {
        return;
    }

    if instant {
        scene
            .world
            .effects
            .spawn(Box::new(super::town_portal::TownPortal::new(
                actor.position,
                if actor.entity < 1000 { 1 } else { 3 },
                now,
            )));

        return;
    }

    let Some(y) = super::effect_context::height(
        &scene.world.actors.window,
        scene.world.actors.zone,
        actor.position.x,
        actor.position.z,
    ) else {
        return;
    };

    let at = Vec3::new(actor.position.x, y + 0.05, actor.position.z);

    let mut out = Vec::new();

    if matches!(actor.class, 1 | 2 | 4 | 8 | 0x1A) {
        spawn(at, 0, None, now, &mut out);

        out.push(Box::new(child(at)));
    } else if matches!(actor.model_id, 2 | 3 | 4 | 20..=24) {
        spawn(at, 1, None, now, &mut out);
    }

    for effect in out {
        scene.world.effects.spawn(effect);
    }
}

pub fn spawned(scene: &mut super::field::FieldScene, index: usize, raw_kind: u16, now: u32) {
    let kind = raw_kind & 0x7FFF;

    let actor = &scene.world.actors.actors[index];

    if actor.flags.get(0xB55) != 0 {
        return;
    }

    let (model, class, part, entity, body) = (
        actor.model_id,
        actor.class,
        actor.parts[0].mesh,
        actor.entity,
        actor.position,
    );

    let player_model = matches!(model, 0 | 1);

    if matches!(kind, 2 | 3) {
        let at = body + Vec3::Y * 0.05;

        let style = if player_model {
            Some(0)
        } else if kind != 3 && !matches!(model, 35 | 36) {
            Some(1)
        } else {
            None
        };

        if let Some(style) = style {
            let mut out = Vec::new();

            spawn(at, style, None, now, &mut out);

            for effect in out {
                scene.world.effects.spawn(effect);
            }

            if style == 1 && scene.player == Some(index) {
                scene.world.play_sound(151);
            }
        }

        if kind == 3 {
            if class == 0x3E && part == 2 {
                scene.world.actors.actors[index].position.y = body.y - 2.0;

                let built = super::skill::judgement::build_at(body + Vec3::Y * 0.2, 4, 0.1, now);

                for orphan in built.orphans {
                    scene.world.effects.spawn(Box::new(orphan));
                }

                if let Some(mesh) = built.mesh {
                    scene.world.effects.spawn(Box::new(mesh));
                }

                scene.world.effects.spawn(Box::new(built.token));
            } else {
                scene
                    .world
                    .effects
                    .spawn(Box::new(super::town_portal::TownPortal::new(
                        at,
                        if entity < 1000 { 1 } else { 3 },
                        now,
                    )));
            }
        }

        scene.world.actors.play_at(index, 14, now);

        if matches!(model, 35 | 36) {
            scene.world.play_sound(303);

            if scene.world.actors.effects_detail.effects {
                for i in -3i32..3 {
                    for sign in [1.0, -1.0] {
                        let mut particle =
                            super::effects::Billboard::new(193, 4000, Vec3::ONE, 0.001, 1, 80);

                        particle.birth_ms = Some(now);

                        particle.lift = i % 2 == 1;

                        particle.position =
                            body + Vec3::new(i as f32 * 0.5 * sign, 0.0, i as f32 * 0.5 * sign);

                        scene.world.effects.spawn(Box::new(particle));
                    }
                }
            }
        } else {
            super::timers::spawn(scene, at, 0);
        }
    }

    if player_model {
        match raw_kind & 0xF0 {
            0x10 => scene.world.actors.play_at(index, 26, now),
            0x20 => scene.world.actors.play_at(index, 24, now),
            _ => {}
        }
    }
}

pub fn raise(
    scene: &mut super::field::FieldScene,
    attack: &formats::net::attack::Attack,
    opcode: u16,
    context: &super::effect_context::Construction,
    now: u32,
) -> usize {
    if !matches!(attack.skill_id, 5 | 0x37) {
        return 0;
    }

    let local = context.viewer.is_some_and(|v| v.key == context.caster.key);

    if local && !attack.echo && attack.action != 0xFE {
        return 0;
    }

    if attack.skill_id == 5 {
        scene.world.effects.spawn(Box::new(Start::new(
            context.caster.position + Vec3::Y * 0.5,
            2,
            None,
            now,
        )));

        scene.world.play_sound(151);

        return 1;
    }

    let mut count = 0;

    if let Some(target) = context.target {
        let built = super::skill::judgement::build_at(target.position + Vec3::Y * 0.2, 3, 0.1, now);

        count += built.nodes();

        for orphan in built.orphans {
            scene.world.effects.spawn(Box::new(orphan));
        }

        if let Some(mesh) = built.mesh {
            scene.world.effects.spawn(Box::new(mesh));
        }

        scene.world.effects.spawn(Box::new(built.token));
    }

    for victim in context
        .victims
        .iter()
        .take(super::impact_particles::member_limit(opcode))
        .flatten()
    {
        let mut ring = ring(victim.position + Vec3::Y * 0.5, 5, Some(victim.key), now);

        ring.life_ms = 2000;

        scene.world.effects.spawn(Box::new(ring));

        count += 1;
    }

    count
}
