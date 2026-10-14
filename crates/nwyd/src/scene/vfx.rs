use super::effect_tree::Effect;
use super::rng::Lcg;
use super::shade::Ground;
use formats::vfx::dispatch::{self, Class};
use formats::vfx::{Anchor, AxisRef, DispatchRow, PositionRef, Spawn};
use glam::Vec3;

pub const CONSTRUCT_AND_FORGET: [Class; 10] = [
    Class::SkillCure,
    Class::SkillFlash,
    Class::SkillHaste,
    Class::SkillHeal,
    Class::SkillHeavensDust,
    Class::SkillHolyTouch,
    Class::SkillJudgement,
    Class::SkillPoison,
    Class::SkillSpeedUp,
    Class::SkillThunderBolt,
];

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Anchors {
    pub caster: Vec3,
    pub target: Option<Vec3>,
    pub target_cell: Vec3,
}

impl Anchors {
    pub fn midpoint(&self) -> Option<Vec3> {
        let target = self.target?;

        Some(Vec3::new(
            (self.caster.x + target.x) * 0.5,
            target.y,
            (self.caster.z + target.z) * 0.5,
        ))
    }

    pub fn anchor(&self, anchor: Anchor) -> Option<Vec3> {
        match anchor {
            Anchor::Caster => Some(self.caster),
            Anchor::Target => self.target,
            Anchor::TargetCell => Some(self.target_cell),
            Anchor::Midpoint => self.midpoint(),
        }
    }

    pub fn at(&self, at: PositionRef) -> Option<Vec3> {
        match at {
            PositionRef::Caster => Some(self.caster),
            PositionRef::Target => self.target,
            PositionRef::TargetCell => Some(self.target_cell),
            PositionRef::Midpoint => self.midpoint(),
            PositionRef::Offset(anchor, [x, y, z]) => {
                self.anchor(anchor).map(|p| p + Vec3::new(x, y, z))
            }
            PositionRef::Along {
                from,
                to,
                from_offset,
                to_offset,
                divisor,
            } => {
                if !divisor.is_finite() || divisor <= 0.0 {
                    return None;
                }

                let start = self.anchor(from)? + Vec3::from(from_offset);

                let end = self.anchor(to)? + Vec3::from(to_offset);

                let point = start + (end - start) / divisor;

                point.is_finite().then_some(point)
            }
            PositionRef::Composite(axes) => {
                let mut out = Vec3::ZERO;

                for (i, axis) in axes.iter().enumerate() {
                    let AxisRef::From(anchor, delta) = axis else {
                        return None;
                    };

                    out[i] = self.anchor(*anchor)?[i] + delta;
                }

                Some(out)
            }
            PositionRef::Unresolved => None,
        }
    }

    pub fn at_axes(&self, at: PositionRef) -> [Option<f32>; 3] {
        let PositionRef::Composite(axes) = at else {
            let p = self.at(at);

            return [p.map(|v| v.x), p.map(|v| v.y), p.map(|v| v.z)];
        };

        let mut out = [None; 3];

        for (i, axis) in axes.iter().enumerate() {
            if let AxisRef::From(anchor, delta) = axis {
                out[i] = self.anchor(*anchor).map(|p| p[i] + delta);
            }
        }

        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Cx<'a> {
    pub at: &'a Anchors,
    pub rate: f32,
    pub detail: i32,
    pub now_ms: u32,
}

pub struct Ambient<'a> {
    pub construction: Option<super::effect_context::Construction>,
    pub rng: &'a mut Lcg,
    pub ground: Option<&'a Ground<'a>>,
    pub skins: Vec<super::skinfx::SkinMeshEffect>,
}

impl<'a> Ambient<'a> {
    pub fn new(rng: &'a mut Lcg) -> Ambient<'a> {
        Ambient {
            construction: None,
            rng,
            ground: None,
            skins: Vec::new(),
        }
    }

    pub fn on(mut self, ground: &'a Ground<'a>) -> Ambient<'a> {
        self.ground = Some(ground);

        self
    }

    pub fn with_construction(mut self, context: super::effect_context::Construction) -> Self {
        self.construction = Some(context);

        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Declined {
    MissingAssets,
    MissingContext,
    NoRecipe,
    Unplaced,
    Arguments,
}

impl Declined {
    pub fn label(self) -> &'static str {
        match self {
            Declined::MissingAssets => "missing assets",
            Declined::MissingContext => "missing context",
            Declined::NoRecipe => "no recipe",
            Declined::Unplaced => "unplaced",
            Declined::Arguments => "arguments",
        }
    }
}

pub type Build =
    fn(&Spawn, &Cx<'_>, &mut Ambient<'_>, &mut Vec<Box<dyn Effect>>) -> Result<(), Declined>;

pub struct Recipe {
    pub class: Class,
    pub build: Build,
}

impl std::fmt::Debug for Recipe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Recipe({:?})", self.class)
    }
}

pub const RECIPES: &[Recipe] = &[
    Recipe {
        class: Class::Arrow,
        build: super::skill::arrow::build_spawn,
    },
    Recipe {
        class: Class::EffectBillboard,
        build: super::skill::billboard::build_spawn,
    },
    Recipe {
        class: Class::EffectLevelUp,
        build: super::skill::level_up::build_spawn,
    },
    Recipe {
        class: Class::EffectMesh,
        build: super::skill::effect_mesh::build_spawn,
    },
    Recipe {
        class: Class::EffectParticle,
        build: super::skill::particle::build_spawn,
    },
    Recipe {
        class: Class::EffectSkinMesh,
        build: super::skill::skin_mesh::build_spawn,
    },
    Recipe {
        class: Class::SkillDoubleSwing,
        build: super::skill::double_swing::build_spawn,
    },
    Recipe {
        class: Class::SkillFireRing,
        build: super::skill::fire_ring::build_spawn,
    },
    Recipe {
        class: Class::Shade,
        build: super::skill::shade_decal::build_spawn,
    },
    Recipe {
        class: Class::SkillBash,
        build: super::skill::bash::build_spawn,
    },
    Recipe {
        class: Class::SkillCure,
        build: super::skill::cure::build_spawn,
    },
    Recipe {
        class: Class::SkillFire,
        build: super::skill::fire::build_spawn,
    },
    Recipe {
        class: Class::SkillFlash,
        build: super::skill::flash::build_spawn,
    },
    Recipe {
        class: Class::SkillFreezeBlade,
        build: super::skill::freeze_blade::build_spawn,
    },
    Recipe {
        class: Class::SkillHaste,
        build: super::skill::haste::build_spawn,
    },
    Recipe {
        class: Class::SkillHeal,
        build: super::skill::heal::recipe,
    },
    Recipe {
        class: Class::SkillHeavensDust,
        build: super::skill::heavens_dust::recipe,
    },
    Recipe {
        class: Class::SkillHolyTouch,
        build: super::skill::holy_touch::build_into,
    },
    Recipe {
        class: Class::SkillIceSpear,
        build: super::skill::ice_spear::build_spawn,
    },
    Recipe {
        class: Class::SkillJudgement,
        build: super::skill::judgement::build,
    },
    Recipe {
        class: Class::SkillMagicArrow,
        build: super::skill::magic_arrow::build_spawn,
    },
    Recipe {
        class: Class::SkillMeteorStorm,
        build: super::skill::meteor_storm::build_spawn,
    },
    Recipe {
        class: Class::SkillPoison,
        build: super::skill::poison::build_into,
    },
    Recipe {
        class: Class::SkillSlowSlash,
        build: super::skill::slow_slash::build_spawn,
    },
    Recipe {
        class: Class::SkillSpChange,
        build: super::skill::sp_change::build_spawn,
    },
    Recipe {
        class: Class::SkillSpeedUp,
        build: super::skill::speedup::build,
    },
    Recipe {
        class: Class::SkillThunderBolt,
        build: super::skill::thunderbolt::build_into,
    },
    Recipe {
        class: Class::SkillTownPortal,
        build: super::skill::town_portal::build_spawn,
    },
];

pub fn recipe_for(class: Class) -> Option<&'static Recipe> {
    RECIPES.iter().find(|r| r.class == class)
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Skips {
    entries: Vec<(Class, Declined)>,
}

impl Skips {
    pub fn push(&mut self, class: Class, why: Declined) {
        self.entries.push((class, why));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn iter(&self) -> impl Iterator<Item = (Class, Declined)> + '_ {
        self.entries.iter().copied()
    }

    pub fn count_of(&self, class: Class) -> usize {
        self.entries.iter().filter(|(c, _)| *c == class).count()
    }

    pub fn count_why(&self, why: Declined) -> usize {
        self.entries.iter().filter(|(_, w)| *w == why).count()
    }

    pub fn named(&self, class: Class) -> bool {
        self.count_of(class) != 0
    }

    pub fn report_lines(&self) -> Vec<String> {
        let mut seen: Vec<(Class, Declined, usize)> = Vec::new();

        for (class, why) in self.iter() {
            match seen.iter_mut().find(|(c, w, _)| *c == class && *w == why) {
                Some((_, _, n)) => *n += 1,
                None => seen.push((class, why, 1)),
            }
        }

        seen.sort_by(|a, b| b.2.cmp(&a.2).then(a.0.cmp(&b.0)));

        seen.iter()
            .map(|(class, why, n)| {
                format!(
                    "NWYD: {n} skill effect spawn(s) skipped — {class:?}: {}",
                    why.label()
                )
            })
            .collect()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Built {
    pub objects: usize,
    pub sounds: &'static [u16],
}

pub fn interpret(
    row: &DispatchRow,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
    skips: &mut Skips,
) -> Built {
    interpret_with(RECIPES, row, cx, env, out, skips)
}

pub fn interpret_with(
    recipes: &[Recipe],
    row: &DispatchRow,
    cx: &Cx<'_>,
    env: &mut Ambient<'_>,
    out: &mut Vec<Box<dyn Effect>>,
    skips: &mut Skips,
) -> Built {
    let before = out.len();

    if let Some(context) = env.construction.as_mut() {
        context.producer = Some((row.chain, row.skill));
    }

    for spawn in row.spawns() {
        match recipes.iter().find(|r| r.class == spawn.class) {
            None => skips.push(spawn.class, Declined::NoRecipe),
            Some(recipe) => {
                if let Err(why) = (recipe.build)(spawn, cx, env, out) {
                    skips.push(spawn.class, why);
                }
            }
        }
    }

    Built {
        objects: out.len().saturating_sub(before),
        sounds: row.sounds,
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Census {
    pub spawns: usize,
    pub built: usize,
    pub unbuilt: Vec<(Class, usize)>,
}

impl Census {
    pub fn report_lines(&self) -> Vec<String> {
        let mut lines = vec![format!(
            "NWYD: skill effect table — {} of {} spawn(s) across {} row(s) build, {} class(es) \
             unbuilt, {} of the 10 construct-and-forget recipes landed",
            self.built,
            self.spawns,
            dispatch::ROWS.len(),
            self.unbuilt.len(),
            CONSTRUCT_AND_FORGET
                .iter()
                .filter(|c| recipe_for(**c).is_some())
                .count(),
        )];

        lines.extend(
            self.unbuilt
                .iter()
                .map(|(class, n)| format!("NWYD:   {n} spawn(s) of {class:?} are not built")),
        );

        lines
    }
}

pub fn census() -> Census {
    let mut spawns = 0usize;

    let mut built = 0usize;

    let mut unbuilt: Vec<(Class, usize)> = Vec::new();

    for row in dispatch::ROWS.iter() {
        for spawn in row.spawns() {
            spawns += 1;

            if recipe_for(spawn.class).is_some() {
                built += 1;

                continue;
            }

            match unbuilt.iter_mut().find(|(c, _)| *c == spawn.class) {
                Some((_, n)) => *n += 1,
                None => unbuilt.push((spawn.class, 1)),
            }
        }
    }

    unbuilt.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));

    Census {
        spawns,
        built,
        unbuilt,
    }
}

pub fn report(census: &Census) {
    for line in census.report_lines() {
        tracing::info!("{line}");
    }
}
