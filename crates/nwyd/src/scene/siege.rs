use super::field::FieldScene;
use formats::net::siege::{self, Siege, WarFlag};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct State {
    pub war_guild: u16,
    pub ally: u16,
    pub zone_kingdom: u8,
    pub war_flag: u32,
    pub war_state: u32,
    pub castle_latch: bool,
    pub arena_music_slot: i32,
}

impl State {
    pub fn new() -> State {
        State {
            war_guild: siege::NO_GUILD,
            ally: siege::NO_GUILD,
            zone_kingdom: siege::KINGDOM_UNSET,
            war_flag: 0,
            war_state: 0,
            castle_latch: false,
            arena_music_slot: siege::MUSIC_SLOT_INVALID,
        }
    }

    pub fn at_war(self) -> bool {
        self.war_flag != 0
    }
}

impl Default for State {
    fn default() -> State {
        State::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Applied {
    Set {
        size: u16,
        war_guild: u16,
        sentinel_skipped: bool,
        zone_kingdom: Option<u8>,
        ally: Option<u16>,
    },
    Ignored {
        size: u16,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WarApplied {
    pub war_flag: u32,
    pub at_war: bool,
    pub cleared_field_music: bool,
    pub cleared_arena_music: bool,
    pub survives_the_zone_block: bool,
}

pub fn apply(scene: &mut FieldScene, frame: &Siege) -> Applied {
    match frame {
        Siege::Ignored { size } => Applied::Ignored { size: *size },
        Siege::Set(state) => {
            scene.siege.war_guild = state.war_guild;

            if let Some(kingdom) = state.kingdom {
                scene.siege.zone_kingdom = kingdom;
            }

            if let Some(ally) = state.ally {
                scene.siege.ally = ally;

                if let Some(panels) = scene.panels.as_mut() {
                    panels.guild.session.ally = u32::from(ally);

                    if let Some(session) = panels.guild_live_session.as_mut() {
                        session.ally = u32::from(ally);
                    }
                }
            }

            Applied::Set {
                size: state.size,
                war_guild: state.war_guild,
                sentinel_skipped: state.has_zero_guild_id(),
                zone_kingdom: state.kingdom,
                ally: state.ally,
            }
        }
    }
}

pub fn war(scene: &mut FieldScene, flag: WarFlag, in_castle_zone: bool) -> WarApplied {
    scene.siege.war_flag = flag.flag;

    scene.siege.arena_music_slot = siege::MUSIC_SLOT_INVALID;

    let cleared_arena_music = flag.clears_arena_music();

    let cleared_field_music = flag.clears_field_music();

    if cleared_field_music {
        scene.city.music_slot = siege::MUSIC_SLOT_INVALID;
    }

    WarApplied {
        war_flag: flag.flag,
        at_war: scene.siege.at_war(),
        cleared_field_music,
        cleared_arena_music,
        survives_the_zone_block: !zone_block_writes(&scene.siege, in_castle_zone),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZoneStep {
    Nothing,
    Raised,
    Lowered,
}

fn zone_block_writes(state: &State, in_castle_zone: bool) -> bool {
    match in_castle_zone {
        true => state.war_state == 0,
        false => state.castle_latch,
    }
}

pub fn zone_tick(state: &mut State, in_castle_zone: bool) -> ZoneStep {
    match in_castle_zone {
        true if state.war_state == 0 => {
            state.war_state = 1;

            state.war_flag = 1;

            state.castle_latch = true;

            ZoneStep::Raised
        }
        false if state.castle_latch => {
            state.war_state = 0;

            state.war_flag = 0;

            ZoneStep::Lowered
        }
        _ => ZoneStep::Nothing,
    }
}

pub fn describe(applied: &Applied) -> String {
    match applied {
        Applied::Ignored { size } => {
            format!("NWYD: 0x3A8 castle siege — size {size:#06x} has no arm; consumed and dropped")
        }
        Applied::Set {
            size,
            war_guild,
            sentinel_skipped,
            zone_kingdom,
            ally,
        } => {
            format!(
                "NWYD: 0x3A8 castle siege — war guild {}{}, kingdom {}, ally {} (arm {size:#06x})",
                match *war_guild {
                    siege::NO_GUILD => "none".to_string(),
                    guild => guild.to_string(),
                },
                match sentinel_skipped {
                    true => " (D-508: the 0xFFFF sentinel was skipped, so this is guild 0)",
                    false => "",
                },
                match zone_kingdom {
                    Some(kingdom) => kingdom.to_string(),
                    None => "unchanged".to_string(),
                },
                match ally {
                    Some(siege::NO_GUILD) => "none".to_string(),
                    Some(ally) => ally.to_string(),
                    None => "unchanged".to_string(),
                },
            )
        }
    }
}

pub fn describe_war(applied: &WarApplied) -> String {
    format!(
        "NWYD: 0x3AC war flag — {} ({:#010x}){}{}{}",
        match applied.at_war {
            true => "raised",
            false => "lowered",
        },
        applied.war_flag,
        match applied.cleared_field_music {
            true => ", field music slot invalidated",
            false => "",
        },
        match applied.cleared_arena_music {
            true => ", arena music slot invalidated",
            false => "",
        },
        match applied.survives_the_zone_block {
            true => "",
            false => ", and the world's zone block overwrites it on the next frame",
        },
    )
}
