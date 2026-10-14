use super::field::FieldScene;
use super::nameplate::{NAME_LEVEL_CLAMP, OUTLINE_BELOW};
use formats::net::plate::{GuildLine, LevelChange, MOB_NAME_LEVEL, PLAYER_ID_LIMIT};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LevelApplied {
    pub entity: u16,
    pub index: Option<usize>,
    pub refused_removal: bool,
    pub name_level: u8,
    pub level: u16,
    pub clamped: bool,
    pub green_name: Option<bool>,
    pub refreshed_screens: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuildApplied {
    pub entity: u16,
    pub index: Option<usize>,
    pub refused_removal: bool,
    pub displayed: String,
}

pub fn level(scene: &mut FieldScene, change: &LevelChange) -> LevelApplied {
    let refused = |index, refused_removal| LevelApplied {
        entity: change.entity,
        index,
        refused_removal,
        name_level: 0,
        level: 0,
        clamped: false,
        green_name: None,
        refreshed_screens: false,
    };

    if change.entity == 0 {
        return refused(None, false);
    }

    let Some(index) = scene.world.actors.entity(change.entity) else {
        return refused(None, false);
    };

    let player = scene.player == Some(index);

    let actor = &mut scene.world.actors.actors[index];

    if actor.removal_mode == 0 {
        return refused(Some(index), true);
    }

    let (level, name_level) = change.stored();

    actor.level = level;

    actor.name_level = name_level;

    if super::nameplate::tagged_name(&actor.name) {
        super::nameplate::refresh_backdrop(actor);

        if player {
            scene.entered.refresh_class();
        }

        return LevelApplied {
            entity: change.entity,
            index: Some(index),
            refused_removal: false,
            name_level: actor.name_level,
            level: actor.level,
            clamped: false,
            green_name: Some(true),
            refreshed_screens: player,
        };
    }

    let clamped = name_level > NAME_LEVEL_CLAMP;

    if name_level >= OUTLINE_BELOW {
        actor.name_level = name_level.min(NAME_LEVEL_CLAMP);
    }

    actor.plate_name_color = super::nameplate::name_colour(actor.name_level);

    let mob = actor.entity >= PLAYER_ID_LIMIT;

    if actor.name_level < OUTLINE_BELOW {
        actor.plate_outline = true;
    } else if !mob {
        actor.plate_outline = false;
    }

    let green_name = match actor.plate_outline {
        true => None,
        false => {
            actor.green_name = false;

            Some(false)
        }
    };

    if player {
        scene.entered.refresh_class();
    }

    LevelApplied {
        entity: change.entity,
        index: Some(index),
        refused_removal: false,
        name_level: actor.name_level,
        level: actor.level,
        clamped,
        green_name,
        refreshed_screens: player,
    }
}

pub fn guild(scene: &mut FieldScene, line: &GuildLine) -> GuildApplied {
    let refused = |index, refused_removal| GuildApplied {
        entity: line.entity,
        index,
        refused_removal,
        displayed: String::new(),
    };

    if line.entity == 0 {
        return refused(None, false);
    }

    let Some(index) = scene.world.actors.entity(line.entity) else {
        return refused(None, false);
    };

    let actor = &mut scene.world.actors.actors[index];

    if actor.removal_mode == 0 {
        return refused(Some(index), true);
    }

    let displayed = line.displayed();

    actor.guild_name = displayed.clone();

    GuildApplied {
        entity: line.entity,
        index: Some(index),
        refused_removal: false,
        displayed,
    }
}

pub fn describe_level(applied: &LevelApplied) -> String {
    match (applied.index, applied.refused_removal) {
        (None, _) => {
            format!(
                "NWYD: 0x2F7 for entity {} — no body in this world carries that id",
                applied.entity,
            )
        }
        (Some(_), true) => {
            format!(
                "NWYD: 0x2F7 for entity {} — the body is being removed, so the level is dropped",
                applied.entity,
            )
        }
        (Some(_), false) => {
            format!(
                "NWYD: 0x2F7 — entity {} is level {}, name level {}{}{}{}",
                applied.entity,
                applied.level,
                applied.name_level,
                match applied.clamped {
                    true => " (clamped, and the clamp sticks on the body)",
                    false => "",
                },
                match applied.green_name {
                    Some(false) => ", green name cleared",
                    Some(true) => ", green name set",
                    None => ", green name untouched",
                },
                match applied.refreshed_screens {
                    true => ", and the HUD is repainted",
                    false => "",
                },
            )
        }
    }
}

pub fn describe_guild(applied: &GuildApplied) -> String {
    match (applied.index, applied.refused_removal) {
        (None, _) => {
            format!(
                "NWYD: 0x2F8 for entity {} — no body in this world carries that id",
                applied.entity,
            )
        }
        (Some(_), true) => {
            format!(
                "NWYD: 0x2F8 for entity {} — the body is being removed, so the guild line is dropped",
                applied.entity,
            )
        }
        (Some(_), false) => match applied.displayed.is_empty() {
            true => {
                format!(
                    "NWYD: 0x2F8 — entity {}'s guild line is cleared",
                    applied.entity
                )
            }
            false => {
                format!(
                    "NWYD: 0x2F8 — entity {} is {:?}",
                    applied.entity, applied.displayed
                )
            }
        },
    }
}

pub const MOB_LEVEL: u8 = MOB_NAME_LEVEL;
