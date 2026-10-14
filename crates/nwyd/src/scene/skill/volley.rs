use crate::scene::{
    arrow::Arrow, effect_context::Construction, effect_tree::Effect, effects::Billboard, rng::Lcg,
};
use glam::Vec3;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Member {
    pub source: Vec3,
    pub dest: Vec3,
    pub delay: u32,
    pub variant: i32,
    pub argb: u32,
}

pub fn plan(context: &Construction, fallback: Vec3, mastery: u16) -> Vec<Member> {
    let body = context.caster;

    let (dest, count) = context
        .target
        .map_or((fallback + Vec3::Y * 1.2, 3), |target| {
            (
                target.position + Vec3::Y * (target.scale / 1.5),
                (mastery / 80 + 3).clamp(3, 6),
            )
        });

    let mut source = body.position;

    (0..count)
        .map(|i| {
            let angle = i as f32 / 6.0 * std::f32::consts::TAU;

            source.x = source.x - 0.1 + body.scale * angle.sin();

            source.z = source.z - 0.1 + body.scale * angle.cos();

            source.y = body.position.y + body.scale;

            let variant = if i < 2 { 0 } else { i as i32 + 3 };

            Member {
                source,
                dest,
                delay: i as u32 * 100,
                variant,
                argb: [0x777777, 0x777777, 0x222288, 0x338843, 0x884388, 0x883333][i as usize],
            }
        })
        .collect()
}

pub fn build(
    members: &[Member],
    submode: i32,
    now: u32,
    rng: &mut Lcg,
    out: &mut Vec<Box<dyn Effect>>,
) {
    for member in members {
        rng.next();

        let mut sprite = Billboard::new(56, member.delay + 400, Vec3::ONE, 0.0, 1, 80);

        sprite.position = member.source;

        sprite.set_color(member.argb);

        sprite.pass = render::billboard::BillboardPass::Additive;

        sprite.colour_mode = 2;

        out.push(Box::new(sprite));

        out.push(Box::new(Arrow::new(
            member.source,
            member.dest,
            submode,
            0x97,
            0,
            member.variant,
            0,
            now.wrapping_add(member.delay),
        )));
    }
}
