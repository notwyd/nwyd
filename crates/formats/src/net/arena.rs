use crate::error::{Error, Kind, Result};
use crate::text::{Codepage, codepage};
use crate::util::u32_at;

const KIND: Kind = Kind::RoyalArena;

pub const ROUND: u16 = 0x2F1;

pub const KILL: u16 = 0x2F2;

pub const LIST: u16 = 0x2F5;

pub const FAMILY: [u16; 3] = [ROUND, KILL, LIST];

pub const SCORES: usize = 4;

pub mod round_at {

    pub const STATE: usize = 0x00;

    pub const SCORES: usize = 0x04;
}

pub mod kill_at {

    pub const KILLER: usize = 0x00;

    pub const VICTIM: usize = 0x10;

    pub const KILLER_TEAM: usize = 0x20;

    pub const VICTIM_TEAM: usize = 0x24;

    pub const SCORES: usize = 0x28;

    pub const LAST_LIFE: usize = 0x38;
}

pub mod list_at {

    pub const STATE: usize = 0x28;

    pub const CURRENT: usize = 0x50;

    pub const MAX: usize = 0x78;

    pub const COST: usize = 0xA0;
}

pub const SLOTS: usize = 10;

pub const ROUND_LEN: usize = round_at::SCORES + SCORES * 4;

pub const KILL_LEN: usize = kill_at::LAST_LIFE + 4;

pub const LIST_LEN: usize = list_at::COST + SLOTS * 4;

pub const NAME_LEN: usize = 0x10;

pub const ROUND_ARM: i32 = -1;

pub const ROUND_DRAW: i32 = 5;

pub const BLANK: i32 = -1;

pub const KILL_SILENT: i32 = -1;

pub const KILL_TOWER: i32 = -3;

pub const LIST_OPEN: i32 = 1;

pub const BLANK_TEXT: &str = "-";

pub const SLOT_FORMAT: &str = "( %2d / %2d )";

pub const SEED_SECONDS: i32 = 900;

pub const ROUND_BANNER_MS: u32 = 5_000;

pub const OBJECTIVE_BANNER_MS: u32 = 8_000;

pub const KILL_BANNER_MS: u32 = 10_000;

pub const WINS_STRDEF: usize = 1567;

pub const DRAW_STRDEF: usize = 1568;

pub const KILLED_STRDEF: usize = 1569;

pub const OBJECTIVE_STRDEF: usize = 1571;

pub const NAME_STRDEF: usize = 1576;

pub const NO_ENTRY_STRDEF: usize = 1579;

pub const PROCEEDING_STRDEF: usize = 1580;

pub const TOWER_STRDEF: usize = 1610;

pub const LAST_LIFE_STRDEF: usize = 1628;

pub const CONFIRM_STRDEF: usize = 1577;

pub const COST_STRDEF: usize = 1578;

pub const ACCEPT: u16 = 0x2F3;

pub const ACCEPT_LEN: usize = 0x18;

pub const ACCEPT_REPLY: usize = 0x0C;

pub const ACCEPT_BOX: u32 = 0x3B;

pub const ACCEPT_PAYLOAD: u32 = 1;

pub fn accept_frame(reply: u32) -> [u8; ACCEPT_LEN] {
    let mut out = [0u8; ACCEPT_LEN];

    out[0x04..0x06].copy_from_slice(&ACCEPT.to_le_bytes());

    out[ACCEPT_REPLY..ACCEPT_REPLY + 4].copy_from_slice(&reply.to_le_bytes());

    out
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Round {
    pub state: i32,
    pub scores: [i32; SCORES],
}

impl Round {
    pub fn is_active(self) -> bool {
        self.state == ROUND_ARM
    }

    pub fn draw(self) -> bool {
        self.state == ROUND_DRAW
    }

    pub fn winning_team(self) -> i32 {
        self.state.saturating_add(1)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Kill {
    pub killer: String,
    pub victim: String,
    pub killer_team: i32,
    pub victim_team: i32,
    pub scores: [i32; SCORES],
    pub last_life: i32,
}

impl Kill {
    pub fn silent(&self) -> bool {
        self.victim_team == KILL_SILENT
    }

    pub fn tower(&self) -> bool {
        self.victim_team == KILL_TOWER
    }

    pub fn record(&self) -> Option<usize> {
        match (self.silent(), self.tower(), self.last_life != 0) {
            (true, _, _) => None,
            (_, true, _) => Some(TOWER_STRDEF),
            (_, _, true) => Some(LAST_LIFE_STRDEF),
            (_, _, false) => Some(KILLED_STRDEF),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct List {
    pub state: [i32; SLOTS],
    pub current: [i32; SLOTS],
    pub max: [i32; SLOTS],
    pub cost: [i32; SLOTS],
}

impl List {
    pub fn chosen(&self) -> usize {
        self.state.iter().position(|s| *s != 0).unwrap_or(0)
    }
}

pub fn parse_round(payload: &[u8]) -> Result<Round> {
    need(payload, ROUND_LEN)?;

    let mut scores = [0i32; SCORES];

    for (i, slot) in scores.iter_mut().enumerate() {
        *slot = u32_at(payload, round_at::SCORES + i * 4, KIND)? as i32;
    }

    Ok(Round {
        state: u32_at(payload, round_at::STATE, KIND)? as i32,
        scores,
    })
}

pub fn parse_kill(payload: &[u8]) -> Result<Kill> {
    need(payload, KILL_LEN)?;

    let mut scores = [0i32; SCORES];

    for (i, slot) in scores.iter_mut().enumerate() {
        *slot = u32_at(payload, kill_at::SCORES + i * 4, KIND)? as i32;
    }

    Ok(Kill {
        killer: name_at(payload, kill_at::KILLER),
        victim: name_at(payload, kill_at::VICTIM),
        killer_team: u32_at(payload, kill_at::KILLER_TEAM, KIND)? as i32,
        victim_team: u32_at(payload, kill_at::VICTIM_TEAM, KIND)? as i32,
        scores,
        last_life: u32_at(payload, kill_at::LAST_LIFE, KIND)? as i32,
    })
}

pub fn parse_list(payload: &[u8]) -> Result<List> {
    need(payload, LIST_LEN)?;

    let mut list = List {
        state: [0; SLOTS],
        current: [0; SLOTS],
        max: [0; SLOTS],
        cost: [0; SLOTS],
    };

    let arrays: [(usize, &mut [i32; SLOTS]); 4] = [
        (list_at::STATE, &mut list.state),
        (list_at::CURRENT, &mut list.current),
        (list_at::MAX, &mut list.max),
        (list_at::COST, &mut list.cost),
    ];

    for (base, array) in arrays {
        for (i, slot) in array.iter_mut().enumerate() {
            *slot = u32_at(payload, base + i * 4, KIND)? as i32;
        }
    }

    Ok(list)
}

fn name_at(payload: &[u8], off: usize) -> String {
    let field = &payload[off..off + NAME_LEN];

    let cut = field.iter().position(|b| *b == 0).unwrap_or(NAME_LEN);

    codepage::decode(&field[..cut], Codepage::Windows1252)
}

fn need(payload: &[u8], len: usize) -> Result<()> {
    match payload.len() < len {
        true => Err(Error::Truncated {
            kind: KIND,
            offset: 0,
            needed: len,
            len: payload.len(),
        }),
        false => Ok(()),
    }
}
