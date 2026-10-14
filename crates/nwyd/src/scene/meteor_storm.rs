use super::effect_tree::{
    AuthoredMesh, Base, Class, Cull, Effect, MeshSubmission, Pass, Removal, RemovalContext, Sink,
    Submit, TexturedMesh, Tick,
};
use super::effects::Billboard;
use super::rng::Lcg;
use glam::Vec3;
use render::billboard::BillboardPass;
use render::camera::Matrix4;
use std::cell::Cell;
use std::rc::Rc;

pub const CLASS: Class = Class {
    name: "SkillMeteorStorm",
    method_count: 15,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

pub fn cannon_source(source: Vec3, cannon: Option<super::effect_context::Cannon>) -> Vec3 {
    let Some(cannon) = cannon else {
        return source;
    };

    let length = match cannon.mesh {
        Some(405) => 1.0,
        Some(1607) => 1.2,
        _ => return source,
    };

    let angle = cannon.yaw - std::f32::consts::FRAC_PI_2;

    source + Vec3::new(angle.cos() * length, 0.0, -angle.sin() * length)
}

pub fn launches(
    attack: &formats::net::attack::Attack,
    opcode: u16,
    context: &super::effect_context::Construction,
    cell: Vec3,
    area: bool,
    now_ms: u32,
    rng: &mut Lcg,
) -> Vec<MeteorStorm> {
    let skill = attack.skill_id;

    let local = context
        .viewer
        .is_some_and(|viewer| viewer.key == context.caster.key);

    let action_gate = !local || attack.echo || attack.action == 0xFE;

    let mut out = Vec::new();

    if super::impact_particles::alternate_spark(context, skill, attack.action)
        && matches!(skill, 0x73 | 0x74)
    {
        return out;
    }

    if skill == 0x24 && action_gate {
        let target = context
            .target
            .map_or(cell + Vec3::new(0.5, 0.0, 0.5), |target| target.position);

        for n in 0..6 {
            out.push(MeteorStorm::new(
                Vec3::ZERO,
                target + Vec3::new((n % 3) as f32 * 0.3, 0.0, ((n + 3) % 5) as f32 * 0.3),
                1,
                now_ms,
            ));
        }
    } else if matches!(skill, 0x6E | 0x73 | 0x74) && action_gate {
        let count = if skill != 0x6E {
            13
        } else {
            match opcode {
                0x39D => 1,
                0x39E => 2,
                _ => 13,
            }
        };

        let mut previous: Option<(Vec3, Vec3)> = None;

        for victim in context.victims.iter().take(count) {
            let scatter = if skill == 0x6E {
                Vec3::new(
                    (rng.next() % 18) as f32 - 9.0,
                    0.0,
                    (rng.next() % 18) as f32 - 9.0,
                )
            } else {
                Vec3::ZERO
            };

            let pair = if let Some(victim) = victim {
                let dest = victim.position - Vec3::Y * 5.0;

                let source = Vec3::new(dest.x, if skill == 0x6E { 1.0 } else { 10.0 }, dest.z);

                previous = Some((source, dest));

                Some((source, dest))
            } else if skill == 0x6E {
                previous
                    .filter(|(_, dest)| dest.to_array().iter().all(|axis| *axis != 0.0))
                    .map(|(source, dest)| (source + scatter, dest + scatter))
            } else {
                None
            };

            if let Some((source, dest)) = pair {
                let mut meteor = MeteorStorm::new(source, dest, 3, now_ms);

                if skill == 0x6E {
                    meteor.birth_ms = meteor.birth_ms.wrapping_add(rng.next() % 1000);
                }

                out.push(meteor);
            }
        }
    } else if area && (!local || attack.echo) {
        return area_launches(
            skill,
            cell,
            context.caster.position,
            context.cannon,
            context.viewer.map(|v| v.position),
            now_ms,
        );
    }

    out
}

pub fn area_launches(
    skill: i16,
    cell: Vec3,
    source: Vec3,
    cannon: Option<super::effect_context::Cannon>,
    viewer: Option<Vec3>,
    now_ms: u32,
) -> Vec<MeteorStorm> {
    let mut out = Vec::new();

    match skill {
        0x23 => out.push(MeteorStorm::new(Vec3::ZERO, cell + Vec3::Y, 0, now_ms)),
        0x27 | 0x68 => {
            let instant = skill == 0x68;

            let step = if instant { 0.9 } else { 1.8 };

            for n in 0..5 {
                let offset = if n == 4 {
                    Vec3::ZERO
                } else {
                    Vec3::new(
                        (n % 2) as f32 * step * 2.0 - step,
                        0.0,
                        (n / 2) as f32 * step * 2.0 - step,
                    )
                };

                let dest = cell
                    + offset
                    + Vec3::Y
                        * if instant {
                            if n == 4 { 2.3 } else { 2.0 }
                        } else {
                            1.0
                        };

                let mode = if instant {
                    if n == 4 { 10 } else { 9 }
                } else {
                    4
                };

                let birth = now_ms.wrapping_add(if instant {
                    0
                } else if n == 4 {
                    270
                } else {
                    n * 200
                });

                out.push(MeteorStorm::new(Vec3::ZERO, dest, mode, birth));
            }
        }
        0x61 => {
            let source = cannon_source(source + Vec3::Y, cannon);

            let mut meteor = MeteorStorm::new(source, cell + Vec3::Y, 2, now_ms);

            meteor.viewer_distance = viewer.map_or(f32::INFINITY, |viewer| {
                Vec3::new(cell.x, 0.0, cell.z).distance(Vec3::new(viewer.x, 0.0, viewer.z))
            });

            out.push(meteor);
        }
        _ => {}
    }

    out
}

pub fn raise(
    scene: &mut super::field::FieldScene,
    attack: &formats::net::attack::Attack,
    opcode: u16,
    context: &super::effect_context::Construction,
    now_ms: u32,
) -> usize {
    if !matches!(
        attack.skill_id,
        0x24 | 0x23 | 0x27 | 0x61 | 0x68 | 0x6E | 0x73 | 0x74
    ) {
        return 0;
    }

    let (x, z) = (f32::from(attack.to.0), f32::from(attack.to.1));

    let needs_cell = match attack.skill_id {
        0x24 => context.target.is_none(),
        0x6E | 0x73 | 0x74 => false,
        _ => true,
    };

    let y = if needs_cell {
        let Some(y) = super::effect_context::height(
            &scene.world.actors.window,
            scene.world.actors.zone,
            x,
            z,
        ) else {
            tracing::warn!(
                skill = attack.skill_id,
                x,
                z,
                "meteor destination requires loaded terrain"
            );

            return 0;
        };

        y
    } else {
        0.0
    };

    let row = if attack.skill_id == 0x68 {
        0x27
    } else {
        attack.skill_id as usize
    };

    let area = scene
        .statics
        .skills
        .as_ref()
        .and_then(|skills| skills.get(row))
        .is_some_and(|skill| (3..=6).contains(&skill.target_shape));

    let meteors = launches(
        attack,
        opcode,
        context,
        Vec3::new(x, y, z),
        area,
        now_ms,
        &mut scene.world.actors.effect_rng,
    );

    let count = meteors.len();

    if meteors.iter().any(|meteor| meteor.mode == 2) {
        super::cannon::arm(scene, context.cannon);
    }

    for meteor in meteors {
        scene.world.effects.spawn(Box::new(meteor));
    }

    count
}

#[derive(Debug)]
pub struct MeteorStorm {
    pub source: Vec3,
    pub dest: Vec3,
    pub position: Vec3,
    pub mode: i32,
    pub birth_ms: u32,
    pub lifetime_ms: u32,
    pub travel: f32,
    pub viewer_distance: f32,
    pub t: f32,
    pub yaw: f32,
    clock: Rc<Cell<u32>>,
    rng: Lcg,
    spawned: Vec<Box<dyn Effect>>,
    explosions: Vec<(usize, super::fire_ring::FireRing)>,
    sounds: Vec<i64>,
    ice_sound: bool,
    gone: bool,
    removed: bool,
}

impl MeteorStorm {
    pub fn new(mut source: Vec3, dest: Vec3, mode: i32, now_ms: u32) -> Self {
        if source.to_array().iter().all(|axis| axis.trunc() == 0.0) {
            source = dest + Vec3::new(3.0, 5.0, -3.0);
        }

        if mode == 9 {
            source = dest;
        }

        let travel = source.distance(dest);

        let factor = match mode {
            1 => 30,
            3 => 50,
            _ => 100,
        };

        let lifetime_ms = if matches!(mode, 9 | 10) {
            1
        } else {
            (travel as u32).saturating_mul(factor).clamp(1, 5000)
        };

        Self {
            source,
            dest,
            position: source,
            mode,
            birth_ms: now_ms,
            lifetime_ms,
            travel,
            viewer_distance: f32::INFINITY,
            t: 0.0,
            yaw: 0.0,
            clock: Rc::new(Cell::new(0)),
            rng: Lcg::new(now_ms),
            spawned: Vec::new(),
            explosions: Vec::new(),
            sounds: if matches!(mode, 0 | 4) {
                vec![160]
            } else {
                Vec::new()
            },
            ice_sound: false,
            gone: false,
            removed: false,
        }
    }

    pub fn tick_with(&mut self, now_ms: u32, rng: &mut Lcg) {
        if self.gone || (now_ms.wrapping_sub(self.birth_ms) as i32) < 0 {
            return;
        }

        self.t = now_ms.wrapping_sub(self.birth_ms) as f32 / self.lifetime_ms as f32;

        if self.mode == 2 && self.travel > 33.0 && self.viewer_distance < 33.0 {
            self.t += 0.8;
        }

        if self.t >= 1.0 {
            self.gone = true;

            return;
        }

        if matches!(self.mode, 9 | 10) {
            self.gone = true;
        }

        let delta = self.dest - self.source;

        self.yaw = delta.x.atan2(delta.z) - std::f32::consts::FRAC_PI_2;

        self.position = self.source.lerp(self.dest, self.t);

        if self.mode == 2 {
            self.position.y += self.travel * 0.2 * (self.t * std::f32::consts::PI).sin();
        }

        if self.mode == 3 {
            self.yaw = (now_ms % 250) as f32 * std::f32::consts::PI / 125.0;
        }

        if self.mode == 1 && self.t > 0.1 && !self.ice_sound {
            self.ice_sound = true;

            self.sounds.push(161);
        }

        let elapsed = now_ms.wrapping_sub(self.clock.get());

        let count = if elapsed < 20 {
            1
        } else if elapsed < 30 {
            2
        } else {
            4
        };

        for i in 0..count {
            let roll = (rng.next() % 5) as f32;

            let scale = roll * 0.1 + 0.4;

            let mut spark = Billboard::new(0, 1000, Vec3::splat(scale), 0.001, 1, 80);

            spark.position = self.position + Vec3::splat(roll * 0.01) - Vec3::Y * 0.7;

            spark.pass = BillboardPass::Additive;

            spark.lift = true;

            spark.set_color(match self.mode {
                0 | 4 => 0xFFFF7711,
                1 => 0xFF3333FF,
                2 => 0xFFFFEE55,
                3 => 0xFF555555,
                _ => 0xFFFFFFFF,
            });

            self.spawned.push(Box::new(spark));

            if matches!(self.mode, 0 | 4) || (self.mode == 2 && i == 3) {
                let mut smoke = Billboard::new(59, 800, Vec3::splat(scale), 0.001, 1, 80);

                smoke.position =
                    self.position - Vec3::new(0.01, if self.mode == 2 { 0.7 } else { 0.5 }, 0.01);

                smoke.lift = true;

                if self.mode != 2 {
                    smoke.pass = BillboardPass::from_additive(i != 3);

                    smoke.set_color(if i == 3 { 0xFFFFFF28 } else { 0xFFFF7711 });
                }

                self.spawned.push(Box::new(smoke));
            }
        }

        self.clock.set(now_ms);
    }

    fn world(&self, yaw: f32, pitch: f32, scale: f32) -> Matrix4 {
        let rotation =
            Matrix4::rotation_yaw_pitch_roll(yaw, pitch - std::f32::consts::FRAC_PI_2, 0.0);

        let world = Matrix4::multiply(&super::objects::DEVICE_WORLD, &rotation);

        let world = Matrix4::multiply(&world, &Matrix4::scaling(scale, scale, scale));

        Matrix4::multiply(
            &world,
            &Matrix4::translation(self.position.x, self.position.y, self.position.z),
        )
    }

    fn impact(&mut self, cx: &RemovalContext<'_>) {
        if self.mode == 2 {
            for (i, (x, z)) in [
                (0.0, 0.0),
                (-3.0, -3.0),
                (-3.0, 3.0),
                (3.0, -3.0),
                (3.0, 3.0),
            ]
            .into_iter()
            .enumerate()
            {
                let at = self.dest + Vec3::new(x, 0.0, z);

                let recipe = super::burst::Recipe {
                    at,
                    kind: 4,
                    count: 12,
                    spread: if i == 0 { 3.0 } else { 0.1 },
                    argb: 0xFFFFAA00,
                    random_colour: false,
                    texture: 56,
                    scatter: 1.0,
                    additive: true,
                    axis: Vec3::ZERO,
                    lifetime_ms: 1000,
                };

                let (emitter, children) = super::burst::build(&recipe, cx.now_ms, &mut self.rng);

                self.spawned
                    .extend(super::skill::particle::Placed { emitter, children }.into_effects());

                let explosion = super::fire_ring::FireRing::new(
                    at, 0, 3.0, 210, 0x44444444, cx.now_ms, cx.ground,
                );

                self.explosions.push((self.spawned.len(), explosion));

                let mut smoke = Billboard::new(59, 2500, Vec3::splat(1.2), 0.003, 1, 80);

                smoke.position = Vec3::new(at.x, at.y + 1.0, self.position.z + z);

                self.spawned.push(Box::new(smoke));
            }
        } else {
            let flash = match self.mode {
                0 | 9 | 10 => Some((33, 999, Vec3::new(3.5, -1.5, 3.5), 0.005, 9)),
                1 => Some((71, 888, Vec3::splat(0.5), 0.0, 8)),
                4 => Some((33, 999, Vec3::splat(1.5), 0.005, 9)),
                _ => None,
            };

            if let Some((texture, life, size, growth, frames)) = flash {
                let mut flash = Billboard::new(texture, life, size, growth, frames, 111);

                flash.position = self.dest - Vec3::Y * 1.5;

                flash.colour_mode = 0;

                flash.pass = BillboardPass::Additive;

                self.spawned.push(Box::new(flash));
            }

            if matches!(self.mode, 0 | 4 | 9 | 10) {
                self.spawned.push(Box::new(super::timers::TimedQuad::new(
                    super::quads::GroundQuad {
                        owner: 0,
                        slot: super::quads::Slot::Placed,
                        texture: 8,
                        pass: BillboardPass::Additive,
                        anchor: self.dest - Vec3::Y * 0.6,
                        base_scale: Vec3::splat(if self.mode == 10 { 1.0 } else { 0.01 }),
                        grow: 0.01,
                        spin_period: 0,
                        ordinal: 0,
                        argb: if self.mode == 4 {
                            0xFFFF7711
                        } else {
                            0xFFFFFFFF
                        },
                    },
                    500,
                )));
            }

            let mut shade = super::shade::Shade::new(
                if matches!(self.mode, 1 | 3) { 2 } else { 4 },
                7,
                match self.mode {
                    1 => 0xFF3333FF,
                    3 => 0xFFFF7722,
                    _ => 0xFFFF7711,
                },
                self.dest,
                super::effects::SECTOR_UNASSIGNED,
            )
            .with_lifetime(1500);

            if let Some(ground) = cx.ground {
                shade.place(ground);
            }

            self.spawned.push(Box::new(shade));
        }

        self.sounds.push(match self.mode {
            1 => 154,
            2 => 309,
            _ => 152,
        });
    }
}

impl Effect for MeteorStorm {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn needs_ground(&self) -> bool {
        true
    }

    fn bind_meteor_clock(&mut self, clock: Rc<Cell<u32>>) {
        self.clock = clock;
    }

    fn update(&mut self, tick: &Tick) {
        let mut rng = self.rng;

        self.tick_with(tick.now_ms, &mut rng);

        self.rng = rng;
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        self.tick_with(tick.now_ms, live.rng);

        self.rng = *live.rng;
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn removed(&mut self, reason: Removal, cx: &RemovalContext<'_>) {
        if self.removed {
            return;
        }

        self.removed = true;

        self.gone = true;

        if reason != Removal::SceneTeardown {
            self.impact(cx);
        }
    }

    fn removed_live(
        &mut self,
        reason: Removal,
        now_ms: u32,
        live: &mut super::effect_context::Live<'_>,
    ) {
        self.rng = *live.rng;

        self.removed(
            reason,
            &RemovalContext {
                now_ms,
                detail: live.detail,
                ground: live.ground,
            },
        );

        *live.rng = self.rng;
    }

    fn submit(&self, _: &Submit<'_>, out: &mut Sink<'_>) {
        if self.gone || self.t < 0.05 {
            return;
        }

        match self.mode {
            1 => out.mesh.push(MeshSubmission::Textured(TexturedMesh {
                mesh_id: 708,
                texture: 19,
                blend: 1,
                uv_offset: [0.0; 2],
                tint: [1.0; 4],
                world: self.world(
                    self.yaw + std::f32::consts::FRAC_PI_2,
                    std::f32::consts::FRAC_PI_4,
                    1.7,
                ),
            })),
            2 => out.mesh.push(MeshSubmission::Authored(AuthoredMesh {
                mesh_id: 1608,
                world: self.world(0.0, 0.0, 3.0),
                material: super::effect_tree::MeshMaterial::Opaque,
                texture: None,
            })),
            3 => {
                for mesh_id in [2816, 2817] {
                    out.mesh.push(MeshSubmission::Authored(AuthoredMesh {
                        mesh_id,
                        world: self.world(self.yaw, 0.0, 1.0),
                        material: super::effect_tree::MeshMaterial::Additive,
                        texture: None,
                    }));
                }
            }
            _ => {}
        }
    }

    fn take_spawned(&mut self) -> Vec<Box<dyn Effect>> {
        let mut nodes = std::mem::take(&mut self.spawned);

        let mut inserted = 0;

        for (index, mut explosion) in self.explosions.drain(..) {
            let mut children = explosion.take_spawned();

            children.push(Box::new(explosion));

            let count = children.len();

            nodes.splice(index + inserted..index + inserted, children);

            inserted += count;
        }

        nodes
    }

    fn take_sounds(&mut self) -> Vec<i64> {
        std::mem::take(&mut self.sounds)
    }

    fn cull(&self, _: Vec3, _: bool) -> Cull {
        Cull::Kept
    }
}
