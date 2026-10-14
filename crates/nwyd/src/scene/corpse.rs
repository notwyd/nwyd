use super::actors::{Actor, Model};
use super::field::FieldScene;
use super::pick::PICK_VOLUME;
use formats::mesh::appearance;

pub const MOB_ID_FLOOR: u16 = 999;

pub const SINK_MS: u32 = 10_000;

pub const SIEGE_SINK_MS: u32 = 600_000;

pub const SIEGE_CLASS: i32 = 0x38;

pub const BURST_CLASS: i32 = 0x2C;

pub const BURST_SINK: f32 = 4.0;

pub const ANCHORED_CLASS: i32 = 0x44;

pub const CLIP_TAIL_FRAMES: usize = 2;

pub const SUB_STEPS_PER_FRAME: u32 = 4;

pub const MODE_NONE: i32 = -1;

pub const MODE_ARMED: i32 = 0;

pub const MODE_DEAD: i32 = 1;

pub const MODE_VANISHING: i32 = 2;

pub const MODE_SCORE_DEATH: i32 = 4;

pub const REAPING_MODES: [i32; 2] = [MODE_DEAD, MODE_VANISHING];

pub fn life_ms(actor: &Actor) -> u32 {
    match siege(actor) {
        true => SIEGE_SINK_MS,
        false => SINK_MS,
    }
}

pub fn siege(actor: &Actor) -> bool {
    actor.class == SIEGE_CLASS && actor.parts[0].mesh == 0
}

pub fn sink_scale(actor: &Actor) -> f32 {
    if siege(actor) || actor.class == ANCHORED_CLASS {
        return 0.0;
    }

    match actor.class == BURST_CLASS {
        true => BURST_SINK,
        false => 1.0,
    }
}

pub fn sink(actor: &Actor, now_ms: u32) -> f32 {
    if !REAPING_MODES.contains(&actor.removal_mode) || !actor.is_dead {
        return 0.0;
    }

    let Some(since) = actor.corpse_ms else {
        return 0.0;
    };

    let scale = sink_scale(actor);

    if scale == 0.0 {
        return 0.0;
    }

    let height = PICK_VOLUME
        .get(actor.model_id as usize)
        .map_or(0.0, |v| v.1);

    let elapsed = ((now_ms.wrapping_sub(since) as i32) as f32 / SINK_MS as f32).clamp(0.0, 1.0);

    -elapsed * actor.scale * height * scale
}

pub fn expired(actor: &Actor, now_ms: u32) -> bool {
    REAPING_MODES.contains(&actor.removal_mode)
        && actor
            .corpse_ms
            .is_some_and(|since| (now_ms.wrapping_sub(since) as i32) > life_ms(actor) as i32)
}

pub fn clip_over(actor: &Actor, model: &Model, now_ms: u32) -> bool {
    let Some(frames) = model.animation.frames(actor.anim.slot) else {
        return false;
    };

    let frames = match frames > CLIP_TAIL_FRAMES {
        true => frames - CLIP_TAIL_FRAMES,
        false => frames,
    };

    let span = (frames as u32)
        .saturating_mul(actor.anim.sub_frame_ms)
        .saturating_mul(SUB_STEPS_PER_FRAME);

    now_ms.wrapping_sub(actor.anim.start_ms) > span
}

pub fn settle(actor: &mut Actor, models: &[Model], now_ms: u32, viewer: Option<u16>) -> bool {
    if actor.action != appearance::ACTION_DIE {
        return false;
    }

    let Some(model) = models.get(actor.model) else {
        return false;
    };

    if !clip_over(actor, model, now_ms) {
        return false;
    }

    actor.play(appearance::ACTION_DEAD, now_ms, models);

    let mine = viewer.is_some_and(|id| id == actor.entity);

    if !mine && actor.entity > MOB_ID_FLOOR {
        if actor.corpse_ms.is_none() {
            actor.corpse_ms = Some(now_ms);
        }

        actor.removal_mode = MODE_DEAD;
    }

    true
}

pub(crate) fn completion_burst(actor: &mut Actor) -> Option<super::burst::Recipe> {
    if actor.class != 64 || actor.body != 397 {
        return None;
    }

    actor.flags.set(0xB55, 1);

    Some(super::burst::Recipe {
        at: actor.position,
        kind: 0,
        count: 8,
        spread: 10.0,
        argb: 0xFFFF3333,
        random_colour: true,
        texture: 56,
        scatter: 1.0,
        additive: true,
        axis: glam::Vec3::ZERO,
        lifetime_ms: 1000,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Reaped {
    pub entity: u16,
    pub lay_ms: u32,
    pub dropped_target: bool,
}

pub fn reap(scene: &mut FieldScene) -> Vec<Reaped> {
    let now_ms = scene.time.now_ms;

    let due: Vec<(u16, u32)> = scene
        .world
        .actors
        .actors
        .iter()
        .filter(|actor| expired(actor, now_ms))
        .map(|actor| {
            (
                actor.entity,
                now_ms.wrapping_sub(actor.corpse_ms.unwrap_or(now_ms)),
            )
        })
        .collect();

    due.into_iter()
        .map(|(entity, lay_ms)| {
            let dropped_target = scene.attack.auto_target == Some(entity);

            if dropped_target {
                scene.attack.auto_target = None;
            }

            super::action::remove(scene, entity);

            Reaped {
                entity,
                lay_ms,
                dropped_target,
            }
        })
        .collect()
}

pub fn describe(reaped: &Reaped) -> String {
    let mut line = format!(
        "NWYD: entity {} sank and was removed after {} ms (local expiration)",
        reaped.entity, reaped.lay_ms,
    );

    if reaped.dropped_target {
        line.push_str(" — and the auto-attack let go");
    }

    line
}
