use super::{
    effects::Billboard,
    field::FieldScene,
    glow::Glow,
    skill::{poison, thunderbolt},
};
use glam::Vec3;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct State {
    pub stamp: u32,
    pub count: u8,
}

impl State {
    pub fn due(self, now: u32) -> bool {
        self.count > 0 && now.wrapping_sub(self.stamp) as i32 > 300
    }

    fn advance(&mut self, draw: u32) -> bool {
        self.count += 1;

        self.stamp = self.stamp.wrapping_add((draw % 10 + 4) * 100);

        if self.count < 10 {
            return false;
        }

        *self = Self::default();

        true
    }
}

fn cached(at: Vec3) -> bool {
    at.x == 2366.5 && at.z == 3926.5
}

fn pad(scene: &FieldScene, kind: usize) -> Option<usize> {
    scene
        .world
        .effects
        .billboards
        .iter()
        .position(|b| b.kind == kind && b.lifetime_ms == 0 && cached(b.position))
}

pub fn death(scene: &mut FieldScene, index: usize, now: u32) -> bool {
    let actor = &scene.world.actors.actors[index];

    if actor.class != 56 || actor.parts[0].mesh != 0 {
        return false;
    }

    let at = actor.position;

    let Some(slot) = pad(scene, 423) else {
        return false;
    };

    let anchor = scene.world.effects.billboards[slot].position;

    scene.world.effects.billboards[slot].glow = Glow::FlareOnce;

    for (dx, y, dz) in [(-0.8, -4.73, -0.3), (0.8, -4.73, 0.3), (0.0, -3.93, 0.0)] {
        let mut b = Billboard::new(423, 3800, Vec3::new(4.0, 4.0, 2.7), 0.0, 1, 80);

        b.position = Vec3::new(anchor.x + dx, y, anchor.z + dz);

        b.birth_ms = Some(now);

        b.colour_mode = 1;

        b.set_color(0x88FFFFFF);

        b.pass = render::billboard::BillboardPass::Additive;

        b.glow = Glow::FlareLoop;

        scene.world.effects.spawn(Box::new(b));
    }

    burst(scene, at + Vec3::Y * 1.5, 5, now);

    if (at.x as i32 & !127) == 2304 && (at.z as i32 & !127) == 3840 {
        scene.ground_wave = Some(super::field::GroundWave {
            centre: at,
            birth_ms: now.wrapping_sub(100),
        });
    }

    let built = super::skill::judgement::build_at(at - Vec3::Y * 0.5, 5, 0.1, now);

    for q in built.orphans {
        scene.world.effects.spawn(Box::new(q));
    }

    if let Some(mesh) = built.mesh {
        scene.world.effects.spawn(Box::new(mesh));
    }

    scene.world.effects.spawn(Box::new(built.token));

    scene.siege_effects = State {
        stamp: now,
        count: 1,
    };

    true
}

pub fn update(scene: &mut FieldScene, now: u32) {
    if scene.world.actors.zone != (18, 30) {
        return;
    }

    let shown = scene
        .player
        .is_some_and(|i| scene.world.actors.actors[i].position.z >= 3918.0)
        && scene.khepra.is_none();

    visibility(scene, shown);

    if !scene.siege_effects.due(now) {
        return;
    }

    let rng = &mut scene.world.actors.effect_rng;

    let at = Vec3::new(
        (rng.next() % 9) as f32 + 2362.0,
        -6.8,
        (rng.next() % 9) as f32 + 3927.0,
    );

    burst(scene, at, 3, now);

    if scene
        .siege_effects
        .advance(scene.world.actors.effect_rng.next())
    {
        register_poison(
            scene,
            &poison::Recipe {
                at: Vec3::new(2365.0, -9.8, 3930.0),
                argb: 0xFFCC6666,
                count: 25,
                play_sound: 1,
                style: 0,
            },
            now,
        );
    }
}

fn visibility(scene: &mut FieldScene, shown: bool) {
    let Some(first) = pad(scene, 423) else {
        return;
    };

    let Some(second) = pad(scene, 424) else {
        return;
    };

    let objects = &mut scene.world.objects;

    let Some(index) = objects
        .instances
        .iter()
        .position(|i| i.kind == 2035 && cached(i.position))
    else {
        return;
    };

    let instance = objects.instances[index];

    let placement = &mut objects.placements[index];

    let y = if shown { -8.3 } else { -20.0 };

    if placement.position.y != y {
        placement.position.y = y;

        placement.world = super::objects::world_at(placement.position, instance.yaw, 0.0, 0.0);

        placement.bounds = objects.meshes[instance.mesh]
            .bounds
            .transformed(&placement.world);
    }

    scene.world.effects.billboards[first].position.y = if shown { -4.73 } else { -20.0 };

    scene.world.effects.billboards[second].position.y = if shown { -7.8 } else { -20.0 };
}

fn register_poison(scene: &mut FieldScene, recipe: &poison::Recipe, now: u32) {
    let mut built = poison::build(
        recipe,
        now,
        &mut scene.world.actors.effect_rng,
        i32::from(!scene.world.actors.effects_detail.effects),
    );

    for b in &mut built.billboards {
        b.birth_ms = Some(now);
    }

    if let Some(sound) = built.sound {
        scene.world.play_sound(sound);
    }

    let mut nodes = Vec::new();

    poison::push(built, &mut nodes);

    for node in nodes {
        scene.world.effects.spawn(node);
    }
}

fn burst(scene: &mut FieldScene, at: Vec3, mode: i32, now: u32) {
    let mut bolt = thunderbolt::build_at(at, mode, now);

    bolt.shade.birth_ms = Some(now);

    for b in &mut bolt.bolts {
        b.birth_ms = Some(now);
    }

    let p = bolt.poison;

    let mut poison = poison::build(
        &poison::Recipe {
            at: p.at,
            argb: p.argb.unwrap_or(0xFFFFFFFF),
            count: p.count,
            play_sound: p.play_sound,
            style: p.style,
        },
        now,
        &mut scene.world.actors.effect_rng,
        i32::from(!scene.world.actors.effects_detail.effects),
    );

    for b in &mut poison.billboards {
        b.birth_ms = Some(now);
    }

    if let Some(sound) = poison.sound {
        scene.world.play_sound(sound);
    }

    let mut children = Vec::new();

    poison::push(poison, &mut children);

    let ground = super::shade::Ground::new(
        &scene.world.terrain,
        &scene.world.actors.heights,
        scene.world.streamed.iter().map(|s| &s.terrain).collect(),
    );

    bolt.place(&ground);

    for node in bolt.into_nodes(children) {
        scene.world.effects.spawn(node);
    }

    scene.world.play_sound(i64::from(thunderbolt::SOUND));

    for delta in [0.0, -0.3, 0.3] {
        let built = super::dust::build(
            at + Vec3::new(delta, 0.0, delta),
            50.0,
            0,
            now,
            &mut scene.world.actors.effect_rng,
        );

        for mut b in built.children {
            b.birth_ms = Some(now);

            scene.world.effects.spawn(Box::new(b));
        }

        scene.world.effects.spawn(Box::new(built.dust));
    }
}
