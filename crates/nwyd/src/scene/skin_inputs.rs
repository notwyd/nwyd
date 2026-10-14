use super::effect_context::{Construction, Snapshot};
use super::skinfx::{Motion, RigSnapshot, SkinConstruction, SkinInput, SkinMeshEffect};
use super::vfx::Declined;
use formats::mesh::appearance::PartRef;
use formats::vfx::Body;
use glam::Vec3;

fn input(context: &Construction, body: Snapshot) -> Result<SkinConstruction, Declined> {
    let appearance = body.appearance.ok_or(Declined::MissingContext)?;

    let pose = context.pose(body.key);

    let primary = SkinInput {
        model_id: appearance.model_id as i32,
        parts: appearance.parts,
        pose: pose.map(|pose| RigSnapshot {
            model: pose.body.model,
            mesh_id: pose.body.mesh_id,
            tree: pose.body.tree.clone(),
        }),
        animation: None,
        clip_action: None,
        scale: 1.0,
    };

    Ok(SkinConstruction {
        primary,
        secondary: None,
        cape: matches!(body.class, 4 | 8),
        body: pose.cloned(),
    })
}

fn mounted(request: &mut SkinConstruction, dual: bool) {
    let Some(pose) = request.body.as_ref() else {
        return;
    };

    let Some(mount) = pose.body.mount.as_ref() else {
        return;
    };

    let mut parts = [PartRef::EMPTY; 8];

    if let Some(mount_parts) = pose.mount_parts {
        parts[..3].copy_from_slice(&mount_parts);
    }

    let primary = SkinInput {
        model_id: mount.mesh_id,
        parts,
        pose: Some(mount.clone()),
        animation: None,
        clip_action: None,
        scale: 1.0,
    };

    let rider = std::mem::replace(&mut request.primary, primary);

    if dual {
        request.secondary = Some(rider);
    } else {
        request.cape = false;
    }
}

pub(crate) fn impact_sink(context: &Construction) -> Result<SkinMeshEffect, Declined> {
    let body = context.target.ok_or(Declined::MissingContext)?;

    let mut request = input(context, body)?;

    mounted(&mut request, true);

    request.primary.clip_action = Some(0);

    if let Some(rider) = request.secondary.as_mut() {
        rider.clip_action = Some(24);
    }

    let mut effect = SkinMeshEffect::new(request.primary.model_id, Vec3::ZERO, Vec3::ZERO, 0);

    effect.mesh_id2 = request.secondary.as_ref().map(|rider| rider.model_id);

    effect.lifetime_ms = 3000;

    effect.colour_a = [0.5; 3];

    effect.colour_anim = super::skinfx::ColourAnim::Cosine;

    effect.blend = super::skinfx::Blend::Additive;

    effect.motion = Motion::Sink;

    effect.spin = body.yaw;

    effect.place(body.position + Vec3::Y * 6.0);

    effect.construction = Some(request);

    Ok(effect)
}

pub fn bind(
    context: &Construction,
    order: &super::skill::skin_mesh::Order,
    effect: &mut SkinMeshEffect,
) -> Result<(), Declined> {
    effect.owner = match order.owner {
        Some(body) => Some(context.body(body).ok_or(Declined::MissingContext)?.key),
        None => None,
    };

    let motion = order.motion.unwrap_or(Motion::None);

    let mut request = match motion {
        Motion::Projectile | Motion::WobbleProjectile => {
            effect.spin = context.caster.yaw;

            let mut parts = [PartRef::EMPTY; 8];

            for patch in &order.unmodelled {
                if let super::skill::skin_mesh::Unmodelled::Appearance { offset, bytes } = *patch {
                    let index = usize::from((offset - 0xE8) / 4);

                    if let Some(part) = parts.get_mut(index) {
                        *part = PartRef {
                            mesh: bytes as i16,
                            tex: (bytes >> 16) as i16,
                        };
                    }
                }
            }

            let scale = match (order.mesh_id, order.variant) {
                (0x14, 0) => 0.4,
                (0x14, 1) => 0.2,
                (0x20, 2) => 2.0,
                _ => return Err(Declined::Arguments),
            };

            SkinConstruction {
                primary: SkinInput {
                    model_id: order.mesh_id,
                    parts,
                    pose: None,
                    animation: None,
                    clip_action: None,
                    scale,
                },
                secondary: None,
                cape: false,
                body: context.pose(context.caster.key).cloned(),
            }
        }
        Motion::Rise | Motion::Drop | Motion::Sink => {
            let body = *context.body(Body::Target).ok_or(Declined::MissingContext)?;

            effect.spin = body.yaw;

            let mut request = input(context, body)?;

            mounted(&mut request, true);

            request.primary.clip_action = Some(0);

            let clip_owner = if motion == Motion::Sink {
                body
            } else {
                context.caster
            };

            let mounted_clip = clip_owner.appearance.is_some_and(|look| look.mounted)
                && context
                    .pose(clip_owner.key)
                    .is_some_and(|pose| pose.body.mount.is_some());

            if mounted_clip && let Some(secondary) = request.secondary.as_mut() {
                secondary.clip_action = Some(24);
            }

            request
        }
        Motion::OwnerSeek | Motion::None => {
            let body = context.caster;

            effect.spin = body.yaw;

            effect.yaw = body.yaw;

            effect.scale = body.scale;

            let mut request = input(context, body)?;

            mounted(&mut request, false);

            request.primary.scale = body.scale;

            let look = body.appearance.ok_or(Declined::MissingContext)?;

            let slot = if motion == Motion::None
                && request
                    .body
                    .as_ref()
                    .is_some_and(|pose| pose.body.mount.is_some())
            {
                request.body.as_ref().and_then(|pose| pose.mount_run_clip)
            } else {
                look.afterimage_clip
            }
            .ok_or(Declined::MissingContext)?;

            request.primary.animation =
                Some(super::actors::Anim::new(slot, look.animation.sub_frame_ms));

            if motion == Motion::OwnerSeek {
                effect.dst = context.target.ok_or(Declined::MissingContext)?.position;
            } else if let Some(position) =
                request.body.as_ref().and_then(|pose| pose.mount_position)
            {
                effect.place(Vec3::new(position.x, body.position.y + 0.1, position.z));
            }

            request
        }
        _ => return Err(Declined::Arguments),
    };

    if !effect.spin.is_finite()
        || !effect.yaw.is_finite()
        || !effect.scale.is_finite()
        || !effect.dst.is_finite()
    {
        return Err(Declined::MissingContext);
    }

    effect.mesh_id = request.primary.model_id;

    effect.mesh_id2 = request.secondary.as_ref().map(|input| input.model_id);

    if let Some(secondary) = request.secondary.as_mut() {
        secondary.scale = effect.scale2;
    }

    effect.construction = Some(request);

    Ok(())
}

pub fn sp_change(context: &Construction, owner: Option<Snapshot>, effect: &mut SkinMeshEffect) {
    effect.owner = owner.map(|body| body.key);

    effect.construction = Some(SkinConstruction {
        primary: SkinInput {
            model_id: 0x56,
            parts: [PartRef::EMPTY; 8],
            pose: None,
            animation: None,
            clip_action: None,
            scale: 1.0,
        },
        secondary: None,
        cape: false,
        body: owner.and_then(|body| context.pose(body.key)).cloned(),
    });
}
