use super::{
    effects::{Billboard, Motion},
    rng::Lcg,
};
use glam::Vec3;
use render::billboard::BillboardPass;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct State {
    pub armed: bool,
    last_ms: u32,
    pub recoil: Vec3,
    barrel_mesh: i32,
}

impl State {
    pub(crate) fn place(&mut self, mesh: Option<i32>) {
        if let Some(mesh @ (405 | 1607)) = mesh {
            self.barrel_mesh = mesh;
        }
    }

    fn tick(&mut self, now: u32, at: Vec3, yaw: f32) -> bool {
        let elapsed = now.wrapping_sub(self.last_ms);

        let angle = yaw - std::f32::consts::FRAC_PI_2;

        let kick = if elapsed < 500 {
            (elapsed as f32 / 500.0 * std::f32::consts::PI).sin() * 0.3
        } else {
            0.0
        };

        self.recoil = Vec3::new(-angle.cos() * kick, 0.0, angle.sin() * kick);

        let armed = std::mem::take(&mut self.armed);

        let interval = ((at.z as i64 as u32 % 3) & 1) * 500;

        if armed && elapsed > interval {
            self.last_ms = now;

            true
        } else {
            false
        }
    }
}

pub fn arm(scene: &mut super::field::FieldScene, cannon: Option<super::effect_context::Cannon>) {
    let Some(cannon) = cannon else {
        return;
    };

    if let Some(drop) = scene
        .drops
        .items
        .iter_mut()
        .find(|drop| drop.entity == cannon.entity)
    {
        if let Some(state) = &mut drop.cannon {
            state.armed = true;
        }
    }
}

fn sprites(at: Vec3, yaw: f32, mesh: i32, detail: bool, rng: &mut Lcg) -> Vec<Billboard> {
    let (height, length) = if mesh == 1607 { (1.0, 1.2) } else { (1.9, 1.0) };

    let angle = yaw - std::f32::consts::FRAC_PI_2;

    let muzzle = at + Vec3::new(angle.cos() * length, height, -angle.sin() * length);

    let mut flash = Billboard::new(56, 100, Vec3::ONE, 0.0, 1, 80);

    flash.position = muzzle;

    flash.pass = BillboardPass::Additive;

    flash.set_color(0xFFFFEE55);

    let mut out = vec![flash];

    if !detail {
        return out;
    }

    for i in 0..4 {
        let n = rng.next_mod(5) as f32;

        let mut smoke = Billboard::new(0, (5 - i) * 400, Vec3::splat(n * 0.3 + 0.2), 0.001, 1, 80);

        smoke.position = muzzle + Vec3::new(n * 0.1, n * 0.01, n * 0.1);

        smoke.origin = smoke.position;

        smoke.motion = Motion::Creep;

        smoke.rise = 0.01;

        smoke.amp_h = 1.1;

        smoke.pass = BillboardPass::Additive;

        smoke.set_color(0xFFFFEE55);

        out.push(smoke);

        if i == 3 {
            let mut white = Billboard::new(59, 800, Vec3::splat(n * 0.3 + 0.2), 0.001, 1, 80);

            white.position = muzzle + Vec3::new(-0.01, n * 0.01, -0.01);

            white.origin = white.position;

            white.motion = Motion::Creep;

            white.rise = 0.01;

            white.amp_h = 1.1;

            white.pass = BillboardPass::Blended;

            white.set_color(0xFFFFFFFF);

            out.push(white);
        }
    }

    out
}

pub fn tick(scene: &mut super::field::FieldScene, now: u32) {
    let mut fired = Vec::new();

    for drop in &mut scene.drops.items {
        let Some(state) = &mut drop.cannon else {
            continue;
        };

        if state.tick(now, drop.position, drop.yaw) {
            fired.push((drop.position + state.recoil, drop.yaw, state.barrel_mesh));
        }
    }

    if fired.is_empty() {
        return;
    }

    let ground = super::shade::Ground::new(
        &scene.world.terrain,
        &scene.world.actors.heights,
        scene
            .world
            .streamed
            .iter()
            .map(|zone| &zone.terrain)
            .collect(),
    )
    .with_water(&scene.world.sea.seas);

    let reach = scene.world.pose_reach();

    let mut audible = 0;

    for (at, yaw, mesh) in fired {
        for sprite in sprites(
            at,
            yaw,
            mesh,
            scene.world.actors.effects_detail.effects,
            &mut scene.world.actors.effect_rng,
        ) {
            scene.world.effects.spawn(Box::new(sprite));
        }

        let mut mark =
            super::shade::Shade::new(4, 7, 0x33443300, at, super::effects::SECTOR_UNASSIGNED)
                .with_lifetime(1000);

        mark.place(&ground);

        scene.world.effects.spawn(Box::new(mark));

        if reach.admits(at) {
            audible += 1;
        }
    }

    for _ in 0..audible {
        scene.world.play_sound(307);
    }
}
