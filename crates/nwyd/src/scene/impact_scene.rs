use super::{effect_context::Construction, field::FieldScene};
use glam::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Blind {
    pub birth_ms: u32,
    pub lifetime_ms: u32,
}

impl Blind {
    pub fn expired(self, now: u32) -> bool {
        now.wrapping_sub(self.birth_ms) >= self.lifetime_ms
    }

    pub fn colour(self, now: u32) -> u32 {
        let phase = now.wrapping_sub(self.birth_ms) as f32 / self.lifetime_ms.max(1) as f32;

        if (0.1..0.9).contains(&phase) {
            0xDD000000
        } else {
            0
        }
    }
}

pub fn duration(caster_level: u16, viewer_level: u16, mastery: u16) -> u32 {
    let ratio = if viewer_level == 0 {
        0
    } else {
        caster_level / viewer_level
    };

    (f32::from(mastery) * f32::from(ratio) * 4000.0 / 100.0).clamp(2000.0, 4000.0) as u32
}

pub fn raise(
    scene: &mut FieldScene,
    attack: &formats::net::attack::Attack,
    context: &Construction,
    now: u32,
) -> usize {
    if !matches!(attack.skill_id, 0x1A | 0x76) {
        return 0;
    }

    let local = context.viewer.is_some_and(|v| v.key == context.caster.key);

    if attack.skill_id == 0x76 {
        if local && !attack.echo && attack.action != 0xFE {
            return 0;
        }

        if super::impact_particles::alternate_spark(context, attack.skill_id, attack.action) {
            return 0;
        }
    } else {
        if local && !attack.echo {
            return 0;
        }

        if !scene
            .statics
            .skills
            .as_ref()
            .and_then(|s| s.get(0x1A))
            .is_some_and(|s| matches!(s.target_shape, 3..=6))
        {
            return 0;
        }
    }

    let ground = super::shade::Ground::new(
        &scene.world.terrain,
        &scene.world.actors.heights,
        scene.world.streamed.iter().map(|s| &s.terrain).collect(),
    );

    let mut out = Vec::new();

    super::skill::flash::spawn_with_ground(
        context.caster.position + Vec3::Y * 0.5,
        0,
        now,
        &ground,
        &mut out,
    );

    let count = out.len();

    for effect in out {
        scene.world.effects.spawn(effect);
    }

    let Some(viewer) = scene.player else {
        return count;
    };

    let Some(caster) = scene
        .world
        .actors
        .actors
        .iter()
        .position(|a| super::effect_context::ActorKey::of(a) == context.caster.key)
    else {
        return count;
    };

    let me = &scene.world.actors.actors[viewer];

    let actor = &scene.world.actors.actors[caster];

    if caster == viewer
        || actor.tracked
        || super::attack::guild_shields(scene, caster, viewer)
        || super::attack::safe_here(
            scene.world.actors.attributes(),
            (me.position.x as i32, me.position.z as i32),
        )
        || !attack.hits.iter().any(|hit| hit.entity == me.entity)
    {
        return count;
    }

    scene.blind = Some(Blind {
        birth_ms: now,
        lifetime_ms: duration(actor.status.level, me.status.level, actor.status.mastery[1]),
    });

    count
}

pub fn tick(scene: &mut FieldScene, now: u32) {
    if let Some(blind) = scene.blind {
        scene.attack.auto_target = None;

        if blind.expired(now) {
            scene.blind = None;
        }
    }
}
