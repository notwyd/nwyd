use super::{actors::Actor, effects::Billboard, field::FieldScene, rng::Lcg, world::World};
use formats::net::attack::Attack;
use glam::Vec3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Pending {
    pub stamp: u32,
    pub damage: i32,
}

pub fn delay(model: u32, part1: i16, skill: i16, action: u8) -> u32 {
    let action = action.min(9);

    if action < 4 {
        return 0;
    }

    if skill == 151 {
        1000
    } else if model == 11 {
        if action < 7 { 700 } else { 1000 }
    } else if model == 20 && part1 == 0 && action >= 7 {
        1000
    } else {
        0
    }
}

pub fn schedule_hit_reactions(
    scene: &mut FieldScene,
    opcode: u16,
    attack: &Attack,
    attacker: usize,
    now: u32,
) {
    if scene.player == Some(attacker) && attack.echo {
        return;
    }

    let skill = if attack.skill_id == 0x68 {
        0x27
    } else {
        attack.skill_id
    };

    let row = usize::try_from(skill)
        .ok()
        .and_then(|id| scene.statics.skills.as_ref()?.get(id));

    if row.is_some_and(|s| matches!(s.target_shape, 3..=6)) {
        return;
    }

    if (0..104).contains(&skill) && skill != 29 && row.is_some_and(|s| s.damage_kind == 8) {
        return;
    }

    let caster = &scene.world.actors.actors[attacker];

    let wait = delay(caster.model_id, caster.parts[1].mesh, skill, attack.action);

    let viewer = scene.player.map(|i| scene.world.actors.actors[i].position);

    for hit in attack
        .landed()
        .iter()
        .take(formats::net::attack::hits_for(opcode))
    {
        let Some(target) = scene.world.actors.entity(hit.entity) else {
            continue;
        };

        let actor = &mut scene.world.actors.actors[target];

        if viewer.is_some_and(|at| {
            formats::world::path::tile_distance(
                (at.x as i32, at.z as i32),
                (actor.position.x as i32, actor.position.z as i32),
            ) > 20
        }) {
            break;
        }

        actor.pending_hit = Some(Pending {
            stamp: now.wrapping_add(wait),
            damage: hit.damage / super::attack::DAMAGE_SPLIT,
        });
    }
}

pub fn selector(actor: &Actor) -> usize {
    if matches!(actor.class, 44 | 45) {
        56
    } else if (actor.class == 25
        && ((actor.parts[0].mesh == 3 && actor.parts[0].tex == 8) || actor.parts[0].mesh == 12))
        || (actor.class == 28 && actor.parts[0].mesh == 2)
        || (actor.class == 16 && actor.parts[0].mesh == 6)
        || matches!(actor.model_id, 26 | 35 | 36 | 11)
    {
        89
    } else {
        0
    }
}

fn ratio(damage: i32, hp: u32) -> f32 {
    if hp > 0 {
        damage as f32 / hp as f32
    } else {
        damage as f32
    }
}

fn flinch(actor: &Actor, ratio: f32, local: bool, now: u32, rng: &mut Lcg) -> bool {
    let (threshold, modulo) = if actor.class == 66 {
        (0.8, 30)
    } else {
        (0.2, 10)
    };

    let admitted = ratio > threshold || (rng.next() % modulo == 0 && ratio.trunc() > 0.0);

    if !admitted || actor.status.hp == 0 {
        return false;
    }

    if local {
        if (4..10).contains(&actor.action) && rng.next() % 5 != 0 {
            return false;
        }

        if now.wrapping_sub(actor.anim.start_ms) <= 600 {
            return false;
        }
    }

    true
}

fn sprite(actor: &Actor) -> Option<Billboard> {
    let &(_, height) = super::pick::PICK_VOLUME.get(actor.model_id as usize)?;

    let mut sprite = Billboard::new(
        119,
        500,
        Vec3::splat((height * actor.scale + 1.01) * 1.5),
        0.002,
        1,
        80,
    );

    sprite.position = actor.position + Vec3::Y;

    sprite.pass = if selector(actor) == 56 {
        render::billboard::BillboardPass::Additive
    } else {
        render::billboard::BillboardPass::Blended
    };

    Some(sprite)
}

pub fn update(world: &mut World, now: u32) {
    let reach = world.pose_reach();

    let viewer = world.actors.viewer;

    for actor in &mut world.actors.actors {
        let Some(pending) = actor.pending_hit else {
            continue;
        };

        if !reach.admits(actor.position) && actor.removal_mode == 0 {
            continue;
        }

        let elapsed = now.wrapping_sub(pending.stamp);

        if elapsed <= 200 || elapsed >= 0x80000000 {
            continue;
        }

        actor.pending_hit = None;

        if matches!(actor.action, 10 | 11) {
            continue;
        }

        let fraction = ratio(pending.damage, actor.status.hp);

        if flinch(
            actor,
            fraction,
            viewer == Some(actor.entity),
            now,
            &mut world.actors.effect_rng,
        ) {
            actor.play(10, now, &world.actors.models);
        }

        if fraction as f64 <= 0.1 {
            continue;
        }

        let Some(sprite) = sprite(actor) else {
            tracing::warn!(
                entity = actor.entity,
                model = actor.model_id,
                "hit reaction has no body height"
            );

            continue;
        };

        world.effects.spawn(Box::new(sprite));
    }
}
