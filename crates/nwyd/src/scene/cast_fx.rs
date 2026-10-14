use super::effect_tree::Effect;
use super::field::FieldScene;
use super::skill_fx::Dispatched;
use super::vfx::{self, Ambient, Anchors, Cx, Declined, Skips};
use formats::vfx::dispatch::Class;
use formats::vfx::{Chain, DispatchRow, row, state};
use formats::world::path::HeightField;
use glam::Vec3;

#[derive(Debug, Default, PartialEq)]

pub struct Raised {
    pub casts: usize,
    pub objects: usize,
    pub skins: usize,
    pub sounds: Vec<u16>,
    pub skips: Vec<(Class, Declined)>,
}

pub fn body_state(scene: &FieldScene, cast: &Dispatched) -> u32 {
    let actor = &scene.world.actors.actors[cast.actor];

    let target = cast
        .pending
        .target
        .and_then(|id| scene.world.actors.entity(id))
        .map(|index| &scene.world.actors.actors[index]);

    let mut bits = 0;

    if actor.volley.armed {
        bits |= state::VOLLEY_ARMED;
    }

    if super::cast::hidden(actor) {
        bits |= state::HIDDEN;
    }

    if actor.flags.get(0xB59) == 1 {
        bits |= state::AFFECT_37;
    }

    if let Some(target) = target {
        bits |= state::TARGET;

        if target.class == super::attack::MIDPOINT_CLASS && target.parts[0].mesh == 0 {
            bits |= state::MIDPOINT;
        }
    }

    if scene.player == Some(cast.actor) {
        bits |= state::SELF;
    }

    bits
}

pub fn selected_row(scene: &FieldScene, cast: &Dispatched) -> Option<&'static DispatchRow> {
    row(
        Chain::Caster,
        u8::try_from(cast.recipe.skill()).ok()?,
        u8::try_from(cast.pending.variant).ok()?,
        body_state(scene, cast),
    )
}

pub fn anchors(scene: &FieldScene, actor: usize, to: Vec3) -> Anchors {
    let caster = scene.world.actors.actors[actor].position;

    let field = HeightField::window(&scene.world.actors.window, scene.world.actors.zone);

    let ground = field.ground(to.x as i32, to.z as i32);

    Anchors {
        caster,
        target: Some(to),
        target_cell: Vec3::new(to.x, ground, to.z),
    }
}

pub fn build(scene: &mut FieldScene, fired: &[Dispatched]) -> Raised {
    let mut raised = Raised::default();

    let now_ms = scene.time.now_ms;

    let detail = match scene.world.actors.effects_detail.effects {
        true => 0,
        false => 1,
    };

    for cast in fired {
        raised.casts += 1;

        if cast.pending.skill == 0x10 {
            let actor = &scene.world.actors.actors[cast.actor];

            if let Some(target) = cast
                .pending
                .target
                .and_then(|id| scene.world.actors.entity(id))
                .map(|index| &scene.world.actors.actors[index])
                && target.position.x == actor.position.x
                && target.position.z == actor.position.z
            {
                let at = actor.position + Vec3::Y;

                let mut slash =
                    super::slow_slash::SlowSlash::new(at, at, 0, Some(target.entity), now_ms);

                slash.target_key = Some(super::effect_context::ActorKey::of(target));

                scene.world.effects.spawn(Box::new(slash));

                raised.objects += 1;
            }

            continue;
        }

        if cast.pending.skill == 0x13 {
            for hand in &mut scene.world.actors.actors[cast.actor].swings {
                hand.emitters.aura_mode = super::aura::AURA_MODE_SWING;
            }
        }

        if cast.pending.skill == 0x11 {
            let actor = &mut scene.world.actors.actors[cast.actor];

            let active = actor.active_hands;

            let any = active.into_iter().any(|a| a);

            for hand in &mut actor.swings {
                if !any || active[usize::from(hand.hand == formats::mesh::appearance::Hand::Main)] {
                    hand.emitters.skill = true;
                }
            }
        }

        if super::skill::bow::admits(cast.pending.skill) {
            let actor = &scene.world.actors.actors[cast.actor];

            let key = super::effect_context::ActorKey::of(actor);

            let retained =
                (actor.flags.get(0xB59) == 1 && !actor.is_dead && actor.flags.get(0xB3E) == 0)
                    .then(|| {
                        scene
                            .world
                            .effects
                            .tree
                            .iter()
                            .find(|node| {
                                node.owned_group()
                                    == Some(super::effect_tree::OwnedGroup::HumanAffect(key, 0x858))
                            })
                            .and_then(|node| node.published_position())
                    })
                    .flatten();

            let context = super::effect_context::Construction::capture(
                scene,
                cast.actor,
                cast.pending.target,
                &[],
            );

            if let Some(built) = super::skill::bow::build(
                &context,
                cast.pending.skill,
                cast.pending.variant,
                retained,
                scene.entered.equipment.items[6].index,
                now_ms,
            ) {
                if let Some(pending) = &mut scene.world.actors.actors[cast.actor].pending {
                    pending.variant = built.variant;
                }

                let out = built.into_effects();

                raised.objects += out.len();

                for effect in out {
                    scene.world.effects.spawn(effect);
                }
            } else {
                raised.skips.push((Class::Arrow, Declined::MissingContext));
            }

            continue;
        }

        let Some(row) = selected_row(scene, cast) else {
            continue;
        };

        if scene.player == Some(cast.actor) {
            match row.skill {
                8 | 10 | 0x12 => scene.camera.start_shake(2, now_ms),
                0x48 | 0x50 => scene.camera.start_shake(1, now_ms),
                _ => {}
            }
        }

        let at = anchors(scene, cast.actor, cast.pending.to);

        let construction = super::effect_context::Construction::capture(
            scene,
            cast.actor,
            cast.pending.target,
            &[],
        );

        let cx = Cx {
            at: &at,
            rate: scene.world.actors.render_lod.rate,
            detail,
            now_ms,
        };

        let heights = &scene.world.actors.heights;

        let mut zones: Vec<&formats::world::trn::Terrain> = vec![&scene.world.terrain];

        zones.extend(scene.world.streamed.iter().map(|s| &s.terrain));

        let ground = super::shade::Ground::new(&scene.world.terrain, heights, zones);

        let mut out: Vec<Box<dyn Effect>> = Vec::new();

        let mut skips = Skips::default();

        let mut env = Ambient::new(&mut scene.world.actors.effect_rng)
            .on(&ground)
            .with_construction(construction);

        if row.skill == 0x25 {
            for sprite in super::skill::billboard::caster_pairs(
                &env.construction.as_ref().unwrap().caster,
                now_ms,
                env.rng,
            ) {
                out.push(Box::new(sprite));
            }

            raised.sounds.extend(row.sounds.iter().copied());
        } else if row.skill == 0x4F {
            let mastery = scene.world.actors.actors[cast.actor].status.mastery[3];

            let members = super::skill::volley::plan(
                env.construction.as_ref().unwrap(),
                cast.pending.to,
                mastery,
            );

            super::skill::volley::build(
                &members,
                i32::from(cast.pending.variant),
                now_ms,
                env.rng,
                &mut out,
            );

            raised.sounds.extend(row.sounds.iter().copied());
        } else {
            let built = vfx::interpret(row, &cx, &mut env, &mut out, &mut skips);

            raised.sounds.extend(built.sounds.iter().copied());
        }

        let skins = std::mem::take(&mut env.skins);

        raised.skips.extend(skips.iter());

        raised.objects += out.len();

        for node in out {
            scene.world.effects.spawn(node);
        }

        for mut skin in skins {
            match scene.world.actors.hydrate_skin(&mut skin, now_ms) {
                Ok(()) => {
                    scene.world.actors.skinfx.spawn(skin);

                    raised.skins += 1;
                }
                Err(error) => {
                    tracing::warn!(
                        model = skin.mesh_id, skill = row.skill, % error,
                        "effect rig hydration failed"
                    );

                    raised
                        .skips
                        .push((Class::EffectSkinMesh, Declined::MissingAssets));
                }
            }
        }
    }

    for slot in raised.sounds.clone() {
        scene.world.play_sound(i64::from(slot));
    }

    raised
}

pub fn describe(raised: &Raised) -> Option<String> {
    if raised.casts == 0 {
        return None;
    }

    let mut line = format!(
        "NWYD: cast — {} cast(s) built {} object(s), {} skin effect(s), {} sound(s)",
        raised.casts,
        raised.objects,
        raised.skins,
        raised.sounds.len(),
    );

    if !raised.skips.is_empty() {
        let mut by_class: std::collections::BTreeMap<&'static str, usize> =
            std::collections::BTreeMap::new();

        for (class, _why) in &raised.skips {
            *by_class.entry(class.name()).or_default() += 1;
        }

        let named: Vec<String> = by_class
            .iter()
            .map(|(class, n)| format!("{class} x{n}"))
            .collect();

        line.push_str(&format!(
            ", {} spawn(s) declined: {}",
            raised.skips.len(),
            named.join(", ")
        ));
    }

    Some(line)
}
