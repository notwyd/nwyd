use crate::scene::{
    arrow::Arrow, double_swing::DoubleSwing, effect_context::Construction, effect_tree::Effect,
};
use glam::Vec3;

pub fn admits(skill: i16) -> bool {
    matches!(skill, 0x68 | 0x69 | 0x97..=0x99)
}

#[derive(Debug)]
pub struct Built {
    pub companion: Option<Arrow>,
    pub arrow: Option<Arrow>,
    pub swing: Option<DoubleSwing>,
    pub variant: i16,
}

pub fn build(
    context: &Construction,
    skill: i16,
    variant: i16,
    retained: Option<Vec3>,
    local_main_hand_item: u16,
    now: u32,
) -> Option<Built> {
    if !admits(skill) {
        return None;
    }

    let caster = context.caster;

    let target = context.target?;

    let (_, height) = caster.dimensions?;

    let source = caster.position
        + Vec3::Y
            * (height * caster.scale * 0.7
                + if caster.model_id == 8 {
                    caster.scale
                } else {
                    0.0
                });

    let dest = target.position + Vec3::Y * (target.scale / 2.5);

    if !source.is_finite() || !dest.is_finite() {
        return None;
    }

    let owner = caster.skill_child.map_or(0, u32::from);

    let flag = u8::from(caster.affect_29);

    let companion = retained
        .filter(|p| p.is_finite())
        .map(|source| Arrow::new(source, dest, i32::from(variant), 10000, flag, 0, owner, now));

    if skill == 0x69 && variant == 2 {
        let mut swing = DoubleSwing::new(source, dest, 3, None, now);

        swing.target_key = Some(target.key);

        return Some(Built {
            companion,
            arrow: None,
            swing: Some(swing),
            variant,
        });
    }

    let variant = if local_main_hand_item == 1010 {
        99
    } else {
        variant
    };

    let arrow = Arrow::new(
        source,
        dest,
        i32::from(variant),
        i32::from(skill),
        flag,
        i32::from(caster.primary_tier),
        owner,
        now,
    );

    Some(Built {
        companion,
        arrow: Some(arrow),
        swing: None,
        variant,
    })
}

impl Built {
    pub fn into_effects(self) -> Vec<Box<dyn Effect>> {
        let mut out: Vec<Box<dyn Effect>> = Vec::new();

        if let Some(effect) = self.companion {
            out.push(Box::new(effect));
        }

        if let Some(effect) = self.arrow {
            out.push(Box::new(effect));
        }

        if let Some(effect) = self.swing {
            out.push(Box::new(effect));
        }

        out
    }
}
