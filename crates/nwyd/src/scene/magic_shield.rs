use super::effect_context::{ActorKey, Snapshot};
use super::effect_tree::{
    Base, Class, Cull, Effect, OwnedGroup, Pass, Removal, RemovalContext, Sink, Submit, Tick,
};
use glam::Vec3;
use render::camera::Matrix4;

pub static CLASS: Class = Class {
    name: "SkillMagicShield",
    method_count: 16,
    base: Base::VisualEffect,
    pass: Pass::Mesh,
};

#[derive(Debug)]

pub struct MagicShield {
    key: ActorKey,
    epoch: u32,
    mode: u8,
    birth: u32,
    position: Vec3,
    mounted: bool,
    radius: f32,
    lift: f32,
    phase: f32,
    detail: i32,
    gone: bool,
}

fn slot(mode: u8) -> u16 {
    match mode {
        0 => 0x818,
        1 => 0x814,
        2 => 0x80C,
        3 => 0x810,
        _ => 0x81C,
    }
}

impl MagicShield {
    pub fn refreshed(body: &Snapshot, mode: u8, now: u32) -> Self {
        Self {
            key: body.key,
            epoch: body.appearance_epoch,
            mode,
            birth: now,
            position: body.position,
            mounted: body.appearance.is_some_and(|a| a.mounted),
            radius: body.dimensions.map_or(1.0, |(radius, _)| radius) * body.scale,
            lift: 0.0,
            phase: 0.0,
            detail: 0,
            gone: false,
        }
    }

    fn meshes(&self) -> Vec<super::effect_tree::TexturedMesh> {
        if self.gone || self.detail == 1 || self.phase < 0.05 {
            return Vec::new();
        }

        let spin = self.phase * std::f32::consts::TAU;

        let fade = (self.phase * std::f32::consts::PI).sin().abs();

        let tint = if self.mode == 4 { fade * 0.5 } else { fade };

        let tint = (tint * 255.0).trunc() / 255.0;

        let mut out = Vec::new();

        let mut push = |mesh_id, texture, at: Vec3, yaw, pitch, roll, scale| {
            let r =
                Matrix4::rotation_yaw_pitch_roll(yaw, pitch - std::f32::consts::FRAC_PI_2, roll);

            let s = Matrix4::multiply(&r, &Matrix4::scaling(scale, 1.0, scale));

            out.push(super::effect_tree::TexturedMesh {
                mesh_id,
                texture,
                uv_offset: [0.0; 2],
                world: Matrix4::multiply(&s, &Matrix4::translation(at.x, at.y, at.z)),
                tint: [tint, tint, tint, 1.0],
                blend: 1,
            });
        };

        if self.mode == 4 {
            let radius = if self.mounted { 1.0 } else { self.radius };

            let at = self.position + Vec3::Y * ((self.lift + 0.6) * radius);

            let tilt = std::f32::consts::FRAC_PI_6;

            push(
                501,
                202,
                at - Vec3::X * 0.5,
                -tilt,
                -tilt,
                std::f32::consts::PI - spin,
                radius * 1.3,
            );

            push(
                502,
                202,
                at,
                -tilt,
                tilt,
                std::f32::consts::PI - spin,
                radius * 1.3,
            );
        } else {
            let scale = if self.mounted { 1.5 } else { 1.0 };

            let texture = match self.mode {
                0 => 57,
                1 => 45,
                _ => 99,
            };

            for (offset, y, multiplier) in [
                (0.0, 0.0, 1.0),
                (std::f32::consts::FRAC_PI_4, -0.1, 1.5),
                (std::f32::consts::FRAC_PI_2, -0.2, 2.0),
                (-std::f32::consts::FRAC_PI_4, 0.1, 2.0),
                (-std::f32::consts::FRAC_PI_2, 0.2, 1.5),
            ] {
                let at = self.position + Vec3::Y * (y + scale * self.lift);

                for mesh in [704, 705] {
                    push(
                        mesh,
                        texture,
                        at,
                        offset - spin,
                        0.0,
                        std::f32::consts::FRAC_PI_2,
                        scale * multiplier,
                    );
                }
            }
        }

        out
    }
}

impl Effect for MagicShield {
    fn class(&self) -> &'static Class {
        &CLASS
    }

    fn owned_group(&self) -> Option<OwnedGroup> {
        Some(OwnedGroup::HumanAffect(self.key, slot(self.mode)))
    }

    fn update(&mut self, tick: &Tick) {
        self.phase = tick.now_ms.wrapping_sub(self.birth) as f32 / 1000.0;

        if self.phase > 1.0 {
            self.gone = true;
        }
    }

    fn update_live(&mut self, tick: &Tick, live: &mut super::effect_context::Live<'_>) {
        self.detail = live.detail;

        self.update(tick);

        let Some(body) = (live.body)(self.key) else {
            self.gone = true;

            return;
        };

        if body.appearance_epoch != self.epoch {
            self.gone = true;

            return;
        }

        self.mounted = body.appearance.is_some_and(|a| a.mounted);

        self.position = if self.mounted {
            body.skill_anchor.unwrap_or(body.position)
        } else {
            body.position
        };

        if self.mode == 2 {
            self.position.y -= 0.3;
        }

        if self.mode == 3 {
            self.position.y += (self.phase * std::f32::consts::PI).sin().abs();
        }
    }

    fn expired(&self) -> bool {
        self.gone
    }

    fn cull(&self, _eye: Vec3, _field: bool) -> Cull {
        Cull::Kept
    }

    fn submit(&self, _cx: &Submit<'_>, out: &mut Sink<'_>) {
        out.mesh.extend(
            self.meshes()
                .into_iter()
                .map(super::effect_tree::MeshSubmission::Textured),
        );
    }
}

pub fn refresh(
    effects: &mut super::effects::BillboardScene,
    body: &Snapshot,
    mode: u8,
    now: u32,
) -> usize {
    if body.class >= 9 || mode > 4 {
        return 0;
    }

    let group = OwnedGroup::HumanAffect(body.key, slot(mode));

    effects.tree.remove_where(
        Removal::Forced,
        &RemovalContext {
            now_ms: now,
            detail: 0,
            ground: None,
        },
        |node| node.owned_group() == Some(group),
    );

    effects.spawn(Box::new(MagicShield::refreshed(body, mode, now)));

    1
}

pub fn periodic(
    actors: &mut super::actors::ActorScene,
    effects: &mut super::effects::BillboardScene,
    now: u32,
) {
    let snapshots: Vec<_> = actors
        .actors
        .iter()
        .map(|a| super::effect_context::snapshot(a, &actors.models))
        .collect();

    for (actor, body) in actors.actors.iter_mut().zip(&snapshots) {
        if actor.flags.get(0xB36) == 1 && now.wrapping_sub(actor.affect_emit_ms[0]) > 1000 {
            refresh(effects, body, 0, now);

            actor.affect_emit_ms[0] = now;
        }

        if actor.flags.get(0xB39) == 1
            && actor.class < 9
            && now.wrapping_sub(actor.affect_emit_ms[1]) > 1000
        {
            refresh(effects, body, 4, now);

            actor.affect_emit_ms[1] = now;
        }

        if actors.effects_detail.glow
            && actor.flags.get(0xB57) == 1
            && now.wrapping_sub(actor.affect_emit_ms[2]) > 250
        {
            let mut slash = super::slow_slash::SlowSlash::new(
                actor.position,
                actor.position,
                2,
                Some(actor.entity),
                now,
            );

            slash.target_key = Some(body.key);

            effects.spawn(Box::new(slash));

            refresh(effects, body, 2, now);

            actor.affect_emit_ms[2] = now;
        }

        if actor.flags.get(0xB3C) == 1 && now.wrapping_sub(actor.affect_emit_ms[3]) > 1000 {
            let target = actor
                .pending
                .and_then(|p| p.target)
                .and_then(|id| snapshots.iter().find(|s| s.key.entity == id));

            let mut slash = super::slow_slash::SlowSlash::new(
                actor.position,
                actor.position,
                1,
                target.map(|b| b.key.entity),
                now,
            );

            slash.target_key = target.map(|b| b.key);

            effects.spawn(Box::new(slash));

            actor.affect_emit_ms[3] = now;
        }
    }
}
