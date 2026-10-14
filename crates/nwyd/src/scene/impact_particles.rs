use super::effect_context::Construction;
use super::effect_tree::Effect;
use super::rng::Lcg;
use glam::Vec3;

pub fn member_limit(opcode: u16) -> usize {
    match opcode {
        0x39D => 1,
        0x39E => 2,
        _ => 13,
    }
}

pub fn immediate_spark(
    context: &Construction,
    skill: i16,
    now: u32,
) -> Option<super::spark::Spark> {
    let (argb, shade, lift) = match skill {
        6 => (0xFF5555FF, 0xFF222299, 1.0),
        0x6B | 0x6D => {
            context.target?;

            let (_, height) = context.caster.dimensions?;

            let extra = matches!((skill, context.caster.model_id), (0x6B, 8) | (0x6D, 42));

            let lift = height * context.caster.scale * 0.7
                + if extra { context.caster.scale } else { 0.0 };

            if skill == 0x6B {
                (0xFFFFAA00, 0xFF554411, lift)
            } else {
                (0xFF550000, 0xAA663311, lift)
            }
        }
        _ => return None,
    };

    let mut spark = super::spark::Spark::new(
        context.caster.position + Vec3::Y * lift,
        context.target.map(|body| super::spark::TrackedBody {
            entity: body.key.entity,
            position: body.position,
        }),
        Vec3::ZERO,
        argb,
        shade,
        1000,
        1.0,
        0.0,
        now,
        now,
    );

    spark.target_key = context.target.map(|body| body.key);

    Some(spark)
}

pub fn raise_immediate(
    scene: &mut super::field::FieldScene,
    attack: &formats::net::attack::Attack,
    context: &Construction,
    now: u32,
) -> usize {
    if !matches!(
        attack.skill_id,
        3 | 6 | 0x17 | 0x1E | 0x20 | 0x22 | 0x24 | 0x2D | 0x4C | 0x4D | 0x57 | 0x6B | 0x6D
    ) {
        return 0;
    }

    let local = context
        .viewer
        .is_some_and(|viewer| viewer.key == context.caster.key);

    if local && !attack.echo && attack.action != 0xFE {
        return 0;
    }

    if matches!(attack.skill_id, 0x20 | 0x22 | 0x24) {
        if attack.action != 0xFE {
            return 0;
        }

        let Some(pet) = scene.world.actors.skinfx.objects.iter().find(|effect| {
            effect.owner == Some(context.caster.key) && effect.held_slot == Some(0x864)
        }) else {
            return 0;
        };

        let at = pet.position + Vec3::Y * 0.1;

        let mut particle = super::effects::Billboard::new(1, 700, Vec3::splat(0.1), 0.002, 1, 80);

        particle.position = at;

        particle.origin = at;

        particle.pass = render::billboard::BillboardPass::Additive;

        particle.base = if attack.skill_id == 0x20 {
            [255, 255, 170, 0]
        } else {
            [255, 170, 238, 255]
        };

        scene.world.effects.spawn(Box::new(particle));

        return 1;
    }

    if matches!(attack.skill_id, 0x4C | 0x4D) {
        scene
            .world
            .play_sound(if attack.skill_id == 0x4C { 34 } else { 36 });

        return 0;
    }

    if attack.skill_id == 0x57 {
        if !scene.world.actors.effects_detail.effects {
            return 0;
        }

        for particle in circles(
            context.caster.position,
            now,
            &mut scene.world.actors.effect_rng,
        ) {
            scene.world.effects.spawn(Box::new(particle));
        }

        return 10;
    }

    if matches!(attack.skill_id, 0x17 | 0x1E) {
        let (at, mode) = if attack.skill_id == 0x1E {
            (context.caster.position + Vec3::Y * 0.5, 0)
        } else {
            (
                context.target.map_or(
                    Vec3::new(attack.to.0 as f32 + 0.5, 2.0, attack.to.1 as f32 + 0.5),
                    |body| body.position + Vec3::Y * 0.5,
                ),
                2,
            )
        };

        let built = super::skill::judgement::build_at(at, mode, 0.1, now);

        let count = built.nodes();

        if let Some(sound) = built.sound {
            scene.world.play_sound(i64::from(sound));
        }

        for child in built.orphans {
            scene.world.effects.spawn(Box::new(child));
        }

        if let Some(mesh) = built.mesh {
            scene.world.effects.spawn(Box::new(mesh));
        }

        scene.world.effects.spawn(Box::new(built.token));

        return count;
    }

    if attack.skill_id == 0x2D {
        let at = if let Some(body) = context.target {
            body.position + Vec3::Y
        } else {
            let (x, z) = (attack.to.0 as f32 + 0.5, attack.to.1 as f32 + 0.5);

            let Some(y) = super::effect_context::height(
                &scene.world.actors.window,
                scene.world.actors.zone,
                x,
                z,
            ) else {
                tracing::warn!(
                    skill = 0x2D,
                    x,
                    z,
                    "impact particles require a loaded height window"
                );

                return 0;
            };

            Vec3::new(x, y, z)
        };

        let recipe = super::burst::Recipe {
            at,
            kind: 0,
            count: 20,
            spread: 0.1,
            argb: 0,
            random_colour: true,
            texture: 56,
            scatter: 1.0,
            additive: true,
            axis: Vec3::ZERO,
            lifetime_ms: 1000,
        };

        let count = super::burst::spawn(
            &mut scene.world.effects,
            &recipe,
            now,
            &mut scene.world.actors.effect_rng,
        );

        scene.world.play_sound(158);

        return count;
    }

    if attack.skill_id == 3 {
        let built = super::skill::holy_touch::build(
            context.caster.position + Vec3::Y * 0.5,
            1,
            now,
            &mut scene.world.actors.effect_rng,
        );

        let sound = built.sound;

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

        let mut out = Vec::new();

        super::skill::holy_touch::push(built, Some(&ground), &mut out);

        let count = out.len();

        for effect in out {
            scene.world.effects.spawn(effect);
        }

        scene.world.play_sound(sound);

        count
    } else if let Some(mut spark) = immediate_spark(context, attack.skill_id, now) {
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

        spark.place_constructor_shades(&ground);

        scene.world.effects.spawn(Box::new(spark));

        if context.target.is_some() {
            scene.world.play_sound(super::spark::SOUND);
        }

        1
    } else {
        if context.target.is_some() {
            tracing::warn!(
                skill = attack.skill_id,
                model = context.caster.model_id,
                "immediate Spark lacks the native model height"
            );
        }

        0
    }
}

pub fn circles(at: Vec3, now: u32, rng: &mut Lcg) -> Vec<super::effects::Billboard> {
    (0..10)
        .map(|i| {
            let scatter = (rng.next() % 5) as f32 * 0.1 - 0.3;

            let mut b = super::effects::Billboard::new(
                56,
                1000 + i * 150,
                Vec3::splat((i & 1) as f32 * 0.1 + 0.2),
                0.0,
                1,
                80,
            );

            b.birth_ms = Some(now);

            b.motion = super::effects::Motion::Circle;

            b.amp_h = (i % 3) as f32 * 0.15 + 1.1;

            b.rise = -2.0;

            b.position = at + Vec3::splat(scatter + 0.5) + Vec3::Y * 2.0;

            b.origin = b.position;

            b.set_color(0xFF4444FF);

            b
        })
        .collect()
}

pub fn raise_area(
    scene: &mut super::field::FieldScene,
    attack: &formats::net::attack::Attack,
    opcode: u16,
    context: &Construction,
    now: u32,
) -> usize {
    if !matches!(attack.skill_id, 0 | 1 | 7) {
        return 0;
    }

    let local = context
        .viewer
        .is_some_and(|viewer| viewer.key == context.caster.key);

    if local && !attack.echo {
        return 0;
    }

    let victims: Vec<_> = context
        .victims
        .iter()
        .map(|v| v.map(|v| v.position))
        .collect();

    raise_area_at(
        scene,
        attack,
        opcode,
        context.caster.position,
        context.caster.position.y,
        &victims,
        now,
    )
}

pub fn raise_area_at(
    scene: &mut super::field::FieldScene,
    attack: &formats::net::attack::Attack,
    opcode: u16,
    source: Vec3,
    arrow_y: f32,
    victims: &[Option<Vec3>],
    now: u32,
) -> usize {
    if !scene
        .statics
        .skills
        .as_ref()
        .and_then(|s| s.get(attack.skill_id as usize))
        .is_some_and(|s| matches!(s.target_shape, 3..=6))
    {
        return 0;
    }

    let mut out: Vec<Box<dyn Effect>> = Vec::new();

    match attack.skill_id {
        0 => {
            let (x, z) = (attack.to.0 as f32, attack.to.1 as f32);

            let Some(y) = super::effect_context::height(
                &scene.world.actors.window,
                scene.world.actors.zone,
                x,
                z,
            ) else {
                tracing::warn!(
                    skill = 0,
                    x,
                    z,
                    "HeavensDust requires a loaded height window"
                );

                return 0;
            };

            out = super::skill::heavens_dust::build(
                Vec3::new(x, y, z),
                now,
                scene.world.actors.render_lod.rate,
                &mut scene.world.actors.effect_rng,
            )
            .into_effects();
        }
        1 => {
            let built = super::skill::holy_touch::build(
                source + Vec3::Y * 0.5,
                0,
                now,
                &mut scene.world.actors.effect_rng,
            );

            let sound = built.sound;

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

            super::skill::holy_touch::push(built, Some(&ground), &mut out);

            scene.world.play_sound(sound);
        }
        7 => {
            for victim in victims.iter().take(member_limit(opcode)).flatten() {
                let dest = Vec3::new(victim.x, arrow_y + 0.5, victim.z);

                out.push(Box::new(super::arrow::Arrow::new(
                    dest + Vec3::new(3.0, 5.0, -3.0),
                    dest,
                    0,
                    10001,
                    0,
                    0,
                    0,
                    now,
                )));
            }
        }
        _ => return 0,
    }

    let count = out.len();

    for effect in out {
        scene.world.effects.spawn(effect);
    }

    count
}

pub fn raise_projectile(
    scene: &mut super::field::FieldScene,
    attack: &formats::net::attack::Attack,
    context: &Construction,
    now: u32,
) -> usize {
    if !matches!(attack.skill_id, 0x6A | 0x6C) {
        return 0;
    }

    let local = context
        .viewer
        .is_some_and(|viewer| viewer.key == context.caster.key);

    if local && !attack.echo && attack.action != 0xFE {
        return 0;
    }

    let Some(target) = context.target else {
        return 0;
    };

    let Some(actor) = scene
        .world
        .actors
        .actors
        .iter_mut()
        .find(|a| super::effect_context::ActorKey::of(a) == context.caster.key)
    else {
        return 0;
    };

    if actor.projectile_muzzle == Vec3::ZERO {
        actor.projectile_muzzle = context.caster.position + Vec3::Y * 1.5;

        actor.body_points.points[0] = Some(actor.projectile_muzzle);
    }

    let mode = i32::from(attack.skill_id == 0x6C);

    let source = context.projectile_source(Some(actor.projectile_muzzle), mode, false);

    let mut count = 0;

    if attack.skill_id == 0x6C && context.caster.class == 0x3E && context.caster.part0_mesh == 2 {
        scene
            .world
            .effects
            .spawn(Box::new(super::magic_arrow::MagicArrow::new(
                context.caster.position + Vec3::Y * 2.0,
                target.position + Vec3::Y,
                5,
                None,
                now,
            )));

        count += 1;
    }

    if let Some(source) = source {
        if mode == 0 {
            let fire = super::timers::Fire::new(
                source,
                target.position,
                42,
                now,
                scene.world.actors.effects_detail.effects,
            );

            scene.world.effects.spawn(Box::new(fire));
        } else {
            let mut spark = super::spark::Spark::new(
                source,
                Some(super::spark::TrackedBody {
                    entity: target.key.entity,
                    position: target.position,
                }),
                target.position + Vec3::Y,
                0xFFFF3300,
                0xFF551100,
                1000,
                1.0,
                0.0,
                now,
                now,
            )
            .with_jitter(0.5);

            spark.target_key = Some(target.key);

            scene.world.effects.spawn(Box::new(spark));
        }

        count += 1;
    }

    count
}

pub fn alternate_spark(context: &Construction, skill: i16, action: u8) -> bool {
    context.caster.class == 0x38
        && context.caster.part0_mesh == 0
        && !matches!(
            skill,
            2 | 3
                | 5
                | 6
                | 0x17
                | 0x1D
                | 0x1E
                | 0x21
                | 0x24
                | 0x29
                | 0x2D
                | 0x34
                | 0x37
                | 0x4C
                | 0x4D
                | 0x56
                | 0x57
                | 0x63
                | 0x64
                | 0x6A..=0x6F
        )
        && !(action == 0xFE && matches!(skill, 0x20 | 0x22))
}

pub fn sparks(
    context: &Construction,
    sources: [Vec3; 2],
    opcode: u16,
    now: u32,
    rng: &mut Lcg,
) -> Vec<super::spark::Spark> {
    let mut previous: Option<Vec3> = None;

    let mut out = Vec::new();

    for (slot, victim) in context
        .victims
        .iter()
        .take(member_limit(opcode))
        .enumerate()
    {
        let offset = Vec3::new(
            (rng.next() % 18) as f32 - 9.0,
            0.0,
            (rng.next() % 18) as f32 - 9.0,
        );

        let (target, dest, jitter) = if let Some(victim) = victim {
            previous = Some(victim.position + Vec3::Y);

            (
                Some(super::spark::TrackedBody {
                    entity: victim.key.entity,
                    position: victim.position,
                }),
                previous.unwrap(),
                0.2,
            )
        } else if let Some(point) = previous.filter(|p| p.to_array().iter().all(|v| *v != 0.0)) {
            (None, point + offset, 0.4)
        } else {
            continue;
        };

        let mut spark = super::spark::Spark::new(
            sources[slot % 2],
            target,
            dest,
            0xFFFF0000,
            0xFF222299,
            1000,
            1.0,
            0.0,
            now,
            now.wrapping_add(slot as u32),
        )
        .with_jitter(jitter);

        spark.target_key = victim.map(|body| body.key);

        out.push(spark);
    }

    out
}

pub fn raise_sparks(
    scene: &mut super::field::FieldScene,
    attack: &formats::net::attack::Attack,
    opcode: u16,
    context: &Construction,
    now: u32,
) -> usize {
    if attack.skill_id != 0x79 && !alternate_spark(context, attack.skill_id, attack.action) {
        return 0;
    }

    let local = context
        .viewer
        .is_some_and(|viewer| viewer.key == context.caster.key);

    if local && !attack.echo && attack.action != 0xFE {
        return 0;
    }

    let sources = scene
        .world
        .actors
        .actors
        .iter()
        .find(|a| super::effect_context::ActorKey::of(a) == context.caster.key)
        .and_then(|actor| {
            super::effect_context::skin_owner_points(actor, &scene.world.actors.models)
        });

    let Some((a, b)) = sources else {
        tracing::warn!(
            skill = attack.skill_id,
            model = context.caster.model_id,
            "impact Spark producer lacks the native paired BODY pose publisher"
        );

        return 0;
    };

    let effects = sparks(
        context,
        [a, b],
        opcode,
        now,
        &mut scene.world.actors.effect_rng,
    );

    let count = effects.len();

    for effect in effects {
        scene.world.effects.spawn(Box::new(effect));
    }

    count
}

pub fn admit_grid(skill: i16, now: u32, stamp: &mut u32, rng: &mut Lcg) -> bool {
    let (interval, backdate) = match skill {
        0x6E => (1700, 1000),
        0x73 => {
            if rng.next() % 10 <= 5 {
                return false;
            }

            (4000, 2000)
        }
        0x74 => return true,
        _ => return false,
    };

    let elapsed = now.wrapping_sub(*stamp);

    if elapsed <= interval || elapsed > i32::MAX as u32 {
        return false;
    }

    *stamp = now.wrapping_sub(backdate);

    true
}

pub fn grid(source: Vec3, now: u32) -> Vec<super::effects::Billboard> {
    let mut out = Vec::with_capacity(36);

    for x in -3i32..3 {
        for z in -3..3 {
            let mut particle =
                super::effects::Billboard::new(193, 4000, Vec3::splat(2.0), 0.001, 1, 80);

            particle.position = source + Vec3::new(x as f32 * 2.0, 0.0, z as f32 * 2.0);

            particle.birth_ms = Some(now);

            particle.lift = x % 2 == 1;

            particle.set_color(0xFF777799);

            out.push(particle);
        }
    }

    out
}

pub fn raise_grid(
    scene: &mut super::field::FieldScene,
    attack: &formats::net::attack::Attack,
    context: &Construction,
    now: u32,
) -> usize {
    if !matches!(attack.skill_id, 0x6E | 0x73 | 0x74) {
        return 0;
    }

    if alternate_spark(context, attack.skill_id, attack.action) {
        return 0;
    }

    let local = context
        .viewer
        .is_some_and(|viewer| viewer.key == context.caster.key);

    if local && !attack.echo && attack.action != 0xFE {
        return 0;
    }

    let Some(actor) = scene
        .world
        .actors
        .actors
        .iter_mut()
        .find(|a| super::effect_context::ActorKey::of(a) == context.caster.key)
    else {
        return 0;
    };

    if !admit_grid(
        attack.skill_id,
        now,
        &mut actor.impact_grid_ms,
        &mut scene.world.actors.effect_rng,
    ) {
        return 0;
    }

    for particle in grid(context.caster.position, now) {
        scene.world.effects.spawn(Box::new(particle));
    }

    if attack.skill_id != 0x74 {
        scene.ground_wave = Some(super::field::GroundWave {
            centre: context.caster.position,
            birth_ms: now,
        });
    }

    if attack.skill_id == 0x73 {
        scene.camera.start_shake(11, now);

        scene.world.play_sound(160);
    }

    36
}

pub fn fan(context: &Construction, cell: Vec3, opcode: u16) -> Vec<Vec3> {
    let source = context.caster.position;

    let mut targets: Vec<Vec3> = context
        .victims
        .iter()
        .take(member_limit(opcode))
        .flatten()
        .map(|body| Vec3::new(body.position.x, source.y + 0.5, body.position.z))
        .collect();

    let centre = if targets.is_empty() {
        cell + Vec3::new(0.5, 0.0, 0.5)
    } else {
        targets.iter().copied().sum::<Vec3>() / targets.len() as f32
    };

    let angle = (f64::from(centre.x - source.x).atan2(f64::from(source.z - centre.z))
        - std::f64::consts::FRAC_PI_2)
        .rem_euclid(std::f64::consts::TAU);

    let sector = (angle * 12.0 / std::f64::consts::TAU).trunc() as i32;

    let start = (sector - 3).rem_euclid(12) as usize;

    let end = (sector + 3) as usize;

    let sectors: Vec<usize> = if start < end && end <= 12 {
        (start..end).collect()
    } else {
        (0..end % 12).chain(start..12).collect()
    };

    for i in sectors {
        let angle = i as f32 * std::f32::consts::TAU / 12.0;

        targets.push(Vec3::new(
            centre.x + angle.cos() * 10.0,
            source.y + 0.5,
            centre.z + angle.sin() * 10.0,
        ));
    }

    targets
}

pub fn ice_trails(
    context: &Construction,
    opcode: u16,
    now: u32,
    rng: &mut Lcg,
) -> Vec<super::freeze_blade::FreezeBlade> {
    let mut previous = None;

    let mut out = Vec::new();

    for victim in context.victims.iter().take(member_limit(opcode)) {
        let offset = Vec3::new(
            (rng.next() % 18) as f32 - 9.0,
            0.0,
            (rng.next() % 18) as f32 - 9.0,
        );

        let at = if let Some(victim) = victim {
            previous = Some(victim.position);

            previous
        } else {
            previous
                .filter(|p: &Vec3| p.to_array().iter().all(|axis| *axis != 0.0))
                .map(|p| p + offset)
        };

        if let Some(at) = at {
            out.push(super::freeze_blade::FreezeBlade::new(
                at, 8, true, false, rng, now,
            ));
        }
    }

    out
}

pub fn raise(
    scene: &mut super::field::FieldScene,
    attack: &formats::net::attack::Attack,
    opcode: u16,
    context: &Construction,
    now: u32,
) -> usize {
    if !matches!(attack.skill_id, 0x56 | 0x6F) {
        return 0;
    }

    let local = context
        .viewer
        .is_some_and(|viewer| viewer.key == context.caster.key);

    if local && !attack.echo && attack.action != 0xFE {
        return 0;
    }

    let mut out: Vec<Box<dyn Effect>> = Vec::new();

    if attack.skill_id == 0x56 {
        let cell = Vec3::new(attack.to.0 as f32, 0.0, attack.to.1 as f32);

        for dest in fan(context, cell, opcode) {
            out.push(Box::new(super::arrow::Arrow::new(
                context.caster.position + Vec3::Y * 0.5,
                dest,
                2,
                0x98,
                0,
                0,
                0,
                now,
            )));
        }
    } else {
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

        out.extend(
            ice_trails(context, opcode, now, &mut scene.world.actors.effect_rng)
                .into_iter()
                .map(|effect| Box::new(effect) as Box<dyn Effect>),
        );
    }

    let count = out.len();

    for effect in out {
        scene.world.effects.spawn(effect);
    }

    count
}
