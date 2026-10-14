use crate::error::{Error, Kind, Result};

pub const RECORD_LEN: usize = 0x10;

pub const ACTORS: usize = 50;

pub const EVENTS_PER_ACTOR: usize = 16;

pub const EVENTS: usize = ACTORS * EVENTS_PER_ACTOR;

pub const FILE_LEN: usize = EVENTS * RECORD_LEN;

pub const READ_LEN: usize = 12_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Move,
    Action,
    Other(u8),
}

impl Command {
    pub fn from_byte(b: u8) -> Command {
        match b {
            b'M' => Command::Move,
            b'A' => Command::Action,
            other => Command::Other(other),
        }
    }

    pub fn to_byte(self) -> u8 {
        match self {
            Command::Move => b'M',
            Command::Action => b'A',
            Command::Other(b) => b,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Event {
    pub time_ms: u32,
    pub command: Command,
    pub padding: u8,
    pub arg: i16,
    pub x: i32,
    pub z: i32,
}

impl Event {
    pub fn is_live(&self) -> bool {
        self.time_ms != 0 || self.command.to_byte() != 0
    }
}

#[derive(Debug, Clone)]
pub struct TimeTable {
    events: Vec<Event>,
}

pub fn parse(bytes: &[u8]) -> Result<TimeTable> {
    if bytes.len() != FILE_LEN {
        return Err(Error::structure(
            Kind::TimeTable,
            format!("expected exactly {FILE_LEN} bytes, file is {}", bytes.len()),
        ));
    }

    let events = bytes
        .chunks_exact(RECORD_LEN)
        .map(|rec| Event {
            time_ms: u32::from_le_bytes(rec[0..4].try_into().expect("4 bytes")),
            command: Command::from_byte(rec[4]),
            padding: rec[5],
            arg: i16::from_le_bytes(rec[6..8].try_into().expect("2 bytes")),
            x: i32::from_le_bytes(rec[8..12].try_into().expect("4 bytes")),
            z: i32::from_le_bytes(rec[12..16].try_into().expect("4 bytes")),
        })
        .collect();

    Ok(TimeTable { events })
}

impl TimeTable {
    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    pub fn actor(&self, actor: usize) -> &[Event] {
        let start = actor * EVENTS_PER_ACTOR;

        &self.events[start..start + EVENTS_PER_ACTOR]
    }

    pub fn actor_events(&self, actor: usize) -> &[Event] {
        let row = self.actor(actor);

        let n = row.iter().position(|e| !e.is_live()).unwrap_or(row.len());

        &row[..n]
    }

    pub fn iter(&self) -> impl Iterator<Item = (usize, &Event)> {
        self.events.iter().enumerate().filter(|(_, e)| e.is_live())
    }

    pub fn live(&self) -> usize {
        self.events.iter().filter(|e| e.is_live()).count()
    }

    pub fn active_actors(&self) -> Vec<usize> {
        (0..ACTORS)
            .filter(|&a| self.actor(a).iter().any(Event::is_live))
            .collect()
    }

    pub fn actors_with_interior_gaps(&self) -> Vec<usize> {
        (0..ACTORS)
            .filter(|&a| {
                let row = self.actor(a);

                let first_dead = row.iter().position(|e| !e.is_live());

                first_dead.is_some_and(|d| row[d..].iter().any(Event::is_live))
            })
            .collect()
    }

    pub fn events_beyond_loaded_prefix(&self) -> Vec<usize> {
        let cutoff = READ_LEN / RECORD_LEN;

        self.iter()
            .map(|(i, _)| i)
            .filter(|&i| i >= cutoff)
            .collect()
    }
}
