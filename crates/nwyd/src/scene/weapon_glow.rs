pub fn bahamut(model: u32, mesh: u32, refine: u8, class: u8) -> Option<usize> {
    if !matches!(
        model,
        0 | 1 | 2 | 4 | 5 | 6 | 9 | 10 | 11 | 12 | 45 | 46 | 47
    ) || !(0xB6C..=0xB78).contains(&mesh)
    {
        return None;
    }

    let base = match class {
        5 => 487,
        6 => 475,
        7 => 499,
        _ => 463,
    };

    Some(base + usize::from(refine.clamp(1, 12)))
}

pub fn scroll(now: u32) -> f32 {
    (now % 6200) as f32 / 6200.0
}

pub(super) fn class_run(mesh: u32) -> bool {
    matches!(mesh, 0xB49..= 0xB55 | 0xB5A..= 0xB5D)
}

pub fn special(model: u32, mesh: u32, refine: u8, class: u8) -> Option<usize> {
    if !formats::mesh::appearance::EFFECT_MESH_MODELS.contains(&model) {
        return None;
    }

    if class_run(mesh) {
        let base = if (0xB5A..=0xB5D).contains(&mesh) {
            260
        } else {
            match class {
                6 => 166,
                7 => 246,
                8 => 260,
                _ => 153,
            }
        };

        return Some(base + usize::from(refine.min(12)));
    }

    if matches!(
        mesh, 0xB60..= 0xB6B | 0xC37..= 0xC41 | 0xBFC..= 0xC06 | 0xC2B..= 0xC35 | 0xC12
        ..= 0xC1B | 0xC2A
    ) {
        let level = match mesh {
            0xC37..=0xC41 | 0xBFC..=0xC06 | 0xC2B..=0xC35 => 9,
            0xC12..=0xC1B | 0xC2A => 12,
            _ => refine.clamp(1, 12),
        };

        return Some(440 + usize::from(level));
    }

    if matches!(
        mesh, 0xB79..= 0xBBC | 0xC07..= 0xC11 | 0xBF4..= 0xBF6 | 0xC1F..= 0xC29 | 0xC42
        ..= 0xC4C
    ) {
        let base = match class {
            5 => 523,
            6 => 535,
            7 => 547,
            _ => 559,
        };

        return Some(base + usize::from(refine.clamp(1, 12)));
    }

    bahamut(model, mesh, refine, class)
}

pub fn after_draw(mesh: u32, slot: usize) -> usize {
    if (0xB8F..=0xB99).contains(&mesh) && (524..=571).contains(&slot) {
        529 + (slot - 524) / 12 * 12
    } else {
        slot
    }
}

pub fn scroll_for(mesh: u32, now: u32) -> f32 {
    if class_run(mesh) {
        (now % 10000) as f32 / 10000.0
    } else {
        scroll(now)
    }
}

pub fn blend(mesh: u32, color_key: bool) -> render::mesh::Stage1Blend {
    if class_run(mesh) {
        render::mesh::Stage1Blend::AddSmooth
    } else if color_key {
        render::mesh::Stage1Blend::ModulateAlphaAddColor
    } else {
        render::mesh::Stage1Blend::Modulate
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Ordinary {
    pub refine: u8,
    pub class: u8,
    pub mmulti: bool,
    pub costume: bool,
}

impl Ordinary {
    pub fn for_hand(
        look: &formats::mesh::appearance::Look,
        hand: formats::mesh::appearance::Hand,
        mesh: u32,
    ) -> Option<Self> {
        use formats::mesh::{appearance, glow};

        let model = look.model?;

        let part = hand.part();

        let refine = look.part_color[part].min(15);

        let class = glow::effect_class(look.part_tier[part], look.part_tint[part]);

        if special(model, mesh, refine, class).is_some()
            || !i16::try_from(mesh).is_ok_and(appearance::hand_mesh_draws)
        {
            return None;
        }

        Some(Self {
            refine,
            class,
            mmulti: appearance::MMULTI_CLASSES.contains(&look.class),
            costume: look.costume_body != 0,
        })
    }

    pub fn resolve(self, color_key: bool) -> Option<formats::mesh::glow::Glow> {
        formats::mesh::glow::for_part(
            self.refine,
            self.class,
            formats::mesh::glow::Part {
                texture: 0,
                color_key,
                mmulti: self.mmulti,
                costume: self.costume,
                mount_kind: 0,
            },
        )
    }
}
