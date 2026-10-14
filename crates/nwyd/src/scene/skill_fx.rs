use super::actors::Actor;
use formats::vfx::dispatch;
use formats::world::path;

pub fn supports_delayed_effect(skill: i16) -> bool {
    (0..=0x67).contains(&skill)
        || (0x97..=0x9B).contains(&skill)
        || matches!(skill, 0x68 | 0x69 | 0x6F)
}

pub fn dispatches(skill: i16) -> bool {
    (0..=0x5F).contains(&skill) || (0x97..=0x99).contains(&skill) || matches!(skill, 0x68 | 0x69)
}

pub fn fuse_ms(skill: i16) -> u32 {
    match (0x97..=0x99).contains(&skill) || matches!(skill, 0x68 | 0x69) {
        true => 200,
        false => 500,
    }
}

pub const CLEAR_EXEMPT: [i16; 6] = [0x97, 0x98, 0x99, 0x68, 0x69, 0x5A];

pub const CANCEL_DISTANCE: i32 = 0x14;

pub const STAMP_ROWS: i16 = 0xF8;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Pending {
    pub fire_at_ms: u32,
    pub to: glam::Vec3,
    pub target: Option<u16>,
    pub skill: i16,
    pub variant: i16,
}

pub fn baseline_spawns(chain: formats::vfx::Chain) -> Vec<u16> {
    let mut ids: Vec<u16> = formats::vfx::rows_for(chain, 0xF0)
        .flat_map(|r| r.spawn_ids.iter().copied())
        .collect();

    ids.sort_unstable();

    ids.dedup();

    ids
}

pub fn own_spawns(skill: i16) -> Vec<&'static formats::vfx::Spawn> {
    let Ok(id) = u8::try_from(skill) else {
        return Vec::new();
    };

    let baseline: Vec<&'static formats::vfx::Spawn> = baseline_spawns(formats::vfx::Chain::Caster)
        .iter()
        .filter_map(|i| dispatch::SPAWNS.get(usize::from(*i)))
        .collect();

    let mut ids: Vec<u16> = formats::vfx::rows_for(formats::vfx::Chain::Caster, id)
        .flat_map(|r| r.spawn_ids.iter().copied())
        .filter(|i| match dispatch::SPAWNS.get(usize::from(*i)) {
            None => false,
            Some(spawn) => !baseline
                .iter()
                .any(|b| b.equivalent_ignoring_allocation(spawn)),
        })
        .collect();

    ids.sort_unstable();

    ids.dedup();

    ids.iter()
        .filter_map(|i| dispatch::SPAWNS.get(usize::from(*i)))
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Recipe {
    Baseline(i16),
    Built(i16),
    Partial(i16),
    Unhandled(i16),
}

impl Recipe {
    pub fn skill(self) -> i16 {
        match self {
            Recipe::Baseline(s) | Recipe::Built(s) | Recipe::Partial(s) | Recipe::Unhandled(s) => s,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Recipe::Baseline(_) => "builds nothing of its own",
            Recipe::Built(_) => "built",
            Recipe::Partial(_) => "partly built",
            Recipe::Unhandled(_) => "not built",
        }
    }
}

pub fn recipe_for(skill: i16) -> Recipe {
    let own = own_spawns(skill);

    if own.is_empty() {
        return Recipe::Baseline(skill);
    }

    let built = own
        .iter()
        .filter(|s| super::vfx::recipe_for(s.class).is_some())
        .count();

    match (built, built == own.len()) {
        (0, _) => Recipe::Unhandled(skill),
        (_, true) => Recipe::Built(skill),
        _ => Recipe::Partial(skill),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Blocking {
    pub class: dispatch::Class,
    pub codes: usize,
    pub sole: usize,
    pub clearable: usize,
}

pub fn blockers() -> Vec<Blocking> {
    let mut codes: std::collections::BTreeMap<dispatch::Class, (usize, usize, usize)> =
        std::collections::BTreeMap::new();

    for skill in (-1..=0x100).filter(|s| supports_delayed_effect(*s) || dispatches(*s)) {
        let own = own_spawns(skill);

        if own.is_empty() {
            continue;
        }

        let mut missing: Vec<dispatch::Class> = own
            .iter()
            .filter(|s| super::vfx::recipe_for(s.class).is_none())
            .map(|s| s.class)
            .collect();

        missing.sort_unstable();

        missing.dedup();

        for class in &missing {
            let entry = codes.entry(*class).or_default();

            entry.0 += 1;

            if missing.len() == 1 {
                entry.1 += 1;

                let placed = own
                    .iter()
                    .filter(|s| s.class == *class)
                    .all(|s| !matches!(s.at, formats::vfx::PositionRef::Unresolved));

                if placed {
                    entry.2 += 1;
                }
            }
        }
    }

    let mut out: Vec<Blocking> = codes
        .into_iter()
        .map(|(class, (codes, sole, clearable))| Blocking {
            class,
            codes,
            sole,
            clearable,
        })
        .collect();

    out.sort_by_key(|b| {
        let rank = std::cmp::Reverse;

        (
            rank(b.clearable),
            rank(b.sole),
            rank(b.codes),
            b.class.name(),
        )
    });

    out
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]

pub struct Volley {
    pub armed: bool,
    pub shots: i32,
}

impl Volley {
    pub fn spent(&self) -> bool {
        1 < self.shots
    }
}

pub fn rearm_ms(skill: i16, variant: i16) -> u32 {
    match (skill, variant) {
        (0x69, 2) => 700,
        _ => 500,
    }
}

pub fn schedule_effect(
    skill: i16,
    variant: u8,
    action: u8,
    target: Option<u16>,
    to: glam::Vec3,
    now_ms: u32,
) -> Option<Pending> {
    if !supports_delayed_effect(skill) {
        return None;
    }

    let variant = match action as i8 {
        -2 => 1,
        _ => i16::from(variant),
    };

    Some(Pending {
        fire_at_ms: now_ms.wrapping_add(fuse_ms(skill)),
        to,
        target,
        skill,
        variant,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Fired {
    Waiting,
    Dispatch(Recipe),
    Nothing,
}

pub fn drain(
    pending: &mut Pending,
    viewer_cell: (i32, i32),
    target_cell: Option<(i32, i32)>,
    stamps: &mut [u32],
    now_ms: u32,
) -> Fired {
    if now_ms.wrapping_sub(pending.fire_at_ms) as i32 <= 0 {
        return Fired::Waiting;
    }

    match target_cell {
        None => pending.skill = 0,
        Some(cell) if path::tile_distance(viewer_cell, cell) > CANCEL_DISTANCE => {
            pending.skill = 0;
        }
        Some(_) => {}
    }

    if pending.skill < STAMP_ROWS
        && let Some(stamp) = usize::try_from(pending.skill)
            .ok()
            .and_then(|i| stamps.get_mut(i))
    {
        match *stamp == 0 || pending.fire_at_ms.wrapping_sub(*stamp) as i32 <= 0 {
            true => *stamp = now_ms,
            false => pending.fire_at_ms = *stamp,
        }
    }

    match dispatches(pending.skill) {
        true => Fired::Dispatch(recipe_for(pending.skill)),
        false => Fired::Nothing,
    }
}

pub fn clears(skill: i16, volley: Volley) -> bool {
    !(CLEAR_EXEMPT.contains(&skill) && volley.armed && !volley.spent())
}

pub const D343_SKILL: i16 = 0x5A;

pub fn pending_of(actor: &Actor) -> Option<&Pending> {
    actor.pending.as_ref()
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Dispatched {
    pub actor: usize,
    pub recipe: Recipe,
    pub pending: Pending,
    pub cleared: bool,
}

pub fn tick(scene: &mut super::field::FieldScene, now_ms: u32) -> Vec<Dispatched> {
    let mut fired = Vec::new();

    let viewer = scene
        .player
        .and_then(|i| scene.world.actors.actors.get(i))
        .map(|a| (a.position.x as i32, a.position.z as i32));

    for index in 0..scene.world.actors.actors.len() {
        let actor = &mut scene.world.actors.actors[index];

        if actor.flags.get(0xB55) != 0 || actor.flags.get(0xB3E) == 1 {
            for hand in &mut actor.swings {
                hand.emitters = super::aura::Emitters::default();
            }

            continue;
        }

        let Some(mut pending) = scene.world.actors.actors[index].pending else {
            continue;
        };

        let target = pending
            .target
            .and_then(|id| scene.world.actors.entity(id))
            .map(|at| {
                let p = scene.world.actors.actors[at].position;

                (p.x as i32, p.z as i32)
            });

        let outcome = drain(
            &mut pending,
            viewer.unwrap_or((0, 0)),
            viewer.and(target),
            &mut scene.cc.skill_stamps,
            now_ms,
        );

        let Fired::Dispatch(recipe) = outcome else {
            let actor = &mut scene.world.actors.actors[index];

            actor.pending = match outcome {
                Fired::Waiting => Some(pending),
                _ if clears(pending.skill, actor.volley) => None,
                _ => Some(pending),
            };

            continue;
        };

        let actor = &mut scene.world.actors.actors[index];

        actor.volley.shots += 1;

        let cleared = clears(pending.skill, actor.volley);

        match cleared {
            true => {
                actor.pending = None;

                actor.volley.shots = 0;
            }
            false => {
                pending.fire_at_ms = pending
                    .fire_at_ms
                    .wrapping_add(rearm_ms(pending.skill, pending.variant));

                actor.pending = Some(pending);
            }
        }

        fired.push(Dispatched {
            actor: index,
            recipe,
            pending,
            cleared,
        });
    }

    fired
}

pub fn describe(fired: &[Dispatched]) -> Option<String> {
    if fired.is_empty() {
        return None;
    }

    let mut line = String::from("NWYD: skill effect —");

    for d in fired {
        let code = d.recipe.skill();

        let tail = match d.cleared {
            true => "",
            false => ", re-armed",
        };

        line.push_str(&format!(
            " {code:#04x} variant {} at ({:.1}, {:.1}, {:.1}) is {}{tail};",
            d.pending.variant,
            d.pending.to.x,
            d.pending.to.y,
            d.pending.to.z,
            d.recipe.label(),
        ));
    }

    Some(line)
}
