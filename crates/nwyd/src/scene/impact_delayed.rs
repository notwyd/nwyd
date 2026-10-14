use super::{
    actors::Actor,
    effects::Billboard,
    rng::Lcg,
    shade::{Ground, Shade},
};
use glam::Vec3;

#[derive(Default)]

struct Burst {
    mark: Option<Shade>,
    sprites: Vec<Billboard>,
    sound: Option<i64>,
    shake: bool,
}

fn take(
    actor: &mut Actor,
    now: u32,
    sprites_supported: bool,
    ground: &Ground<'_>,
    rng: &mut Lcg,
) -> Burst {
    let mut out = Burst::default();

    if actor.impact_grid_ms == 0 || now.wrapping_sub(actor.impact_grid_ms) <= 1000 {
        return out;
    }

    actor.impact_grid_ms = 0;

    if !matches!(actor.class, 32 | 56) {
        return out;
    }

    out.shake = true;

    if actor.class != 32 {
        return out;
    }

    let (sin, cos) = (actor.yaw - std::f32::consts::PI).sin_cos();

    let at = actor.position + Vec3::new(cos * 1.5, 0.0, -sin * 1.5);

    let water = ground.water_above(at.x, at.z, actor.position.y).is_some();

    out.sound = Some(if water { 10 } else { 142 });

    if !water {
        let mut mark = Shade::new(2, 117, 0xAAAAAAAA, at, -1).with_lifetime(5000);

        mark.pass = render::billboard::BillboardPass::Blended;

        mark.cosine_fade = true;

        mark.birth_ms = Some(now);

        mark.place(ground);

        out.mark = Some(mark);
    }

    if sprites_supported {
        for i in 0..3 {
            let mut b = Billboard::new(
                if water { 151 } else { 0 },
                2000,
                Vec3::splat(0.8),
                0.001,
                1,
                80,
            );

            let x = ((rng.next() % 3) as i32 - 1) * i;

            let z = ((rng.next() % 3) as i32 - 1) * i;

            b.position = at + Vec3::new(x as f32 * 0.4, 0.0, z as f32 * 0.4);

            b.lift = true;

            b.birth_ms = Some(now);

            out.sprites.push(b);

            actor.movement_dust.run_ms = now;
        }
    }

    out
}

pub(crate) fn tick(scene: &mut super::field::FieldScene, now: u32) {
    let world = &mut scene.world;

    let ground = Ground::new(
        &world.terrain,
        &world.actors.heights,
        world.streamed.iter().map(|s| &s.terrain).collect(),
    )
    .with_water(&world.sea.seas);

    let bursts = world
        .actors
        .actors
        .iter_mut()
        .map(|actor| take(actor, now, true, &ground, &mut world.actors.effect_rng))
        .collect::<Vec<_>>();

    for burst in bursts {
        if burst.shake {
            scene.camera.start_shake(1, now);
        }

        if let Some(slot) = burst.sound {
            world.play_sound(slot);
        }

        if let Some(mark) = burst.mark {
            world.effects.spawn(Box::new(mark));
        }

        for sprite in burst.sprites {
            world.effects.spawn(Box::new(sprite));
        }
    }
}
