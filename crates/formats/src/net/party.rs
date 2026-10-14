use crate::error::{Error, Kind as ErrorKind, Result};
use crate::net::ground_item::HEADER_LEN;
use crate::text::{Codepage, codepage};
use crate::util::{u16_at, u32_at};

const KIND: ErrorKind = ErrorKind::Party;

pub const JOINED: u16 = 0x37D;

pub const LEFT: u16 = 0x37E;

pub const INVITED: u16 = 0x37F;

pub const MARKED: u16 = 0x3EA;

pub const ACCEPT: u16 = 0x3AB;

pub const FAMILY: [u16; 4] = [LEFT, JOINED, INVITED, MARKED];

pub const PANEL_CONTENT: u16 = 0x13;

pub const ROSTER_FRAME_LEN: usize = 0x28;

pub const ROSTER_LEN: usize = ROSTER_FRAME_LEN - HEADER_LEN;

pub const INVITE_FRAME_LEN: usize = 0x2C;

pub const INVITE_LEN: usize = INVITE_FRAME_LEN - HEADER_LEN;

pub const LEAVE_FRAME_LEN: usize = 0x10;

pub const LEAVE_LEN: usize = LEAVE_FRAME_LEN - HEADER_LEN;

pub const ACCEPT_FRAME_LEN: usize = 0x20;

pub const ACCEPT_LEN: usize = ACCEPT_FRAME_LEN - HEADER_LEN;

pub const NAME_OFF: usize = 0x0A;

pub const NAME_LEN: usize = 0x10;

pub mod at {

    pub const CLASS: usize = 0x00;

    pub const FLAG: usize = 0x01;

    pub const LEVEL: usize = 0x02;

    pub const MAX_HP: usize = 0x04;

    pub const HP: usize = 0x06;

    pub const ID: usize = 0x08;

    pub const NAME: usize = super::NAME_OFF;

    pub const LEAVING: usize = 0x00;

    pub const MARKS_OWNER: usize = 0x00;

    pub const MARKS: usize = 0x02;
}

pub const ROWS: usize = 13;

pub const ROW_LEN: usize = 0x1A;

pub const TABLE_LEN: usize = ROWS * ROW_LEN;

pub const MARKS: usize = 12;

pub const ROW_MARKS: usize = 13;

pub const PARTY_TINT: u32 = 0xAAFF_FF00;

pub const KINGDOM_ONE_TINT: u32 = 0xAA00_00FF;

pub const KINGDOM_TWO_TINT: u32 = 0xAAFF_0000;

pub const PENDING_NAME_COLOR: u32 = 0xFFAA_AAFF;

pub const INVITE_CHAT_COLOR: u32 = 0xFFCC_AAFF;

pub const INVITE_STRDEF: usize = 62;

pub const INVITE_HINT_STRDEF: usize = 63;

pub const INVITE_SOUND: i32 = 0x21;

pub const PLAYER_ID_LIMIT: u16 = 0x3E8;

pub const KINGDOM_ZONES: [(u32, u32); 4] = [(8, 15), (8, 16), (9, 15), (9, 16)];

pub const ZONE_SCALE: f64 = 0.0078125;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub id: u16,
    pub name: String,
    pub class: i8,
    pub level: i16,
    pub hp: i16,
    pub max_hp: i16,
    pub pending: bool,
}

impl Member {
    pub fn name_color(&self) -> u32 {
        if self.pending {
            PENDING_NAME_COLOR
        } else {
            u32::MAX
        }
    }

    pub fn row_state(&self, opcode: u16) -> Option<i32> {
        if !self.pending {
            return None;
        }

        match opcode {
            JOINED => Some(2),
            INVITED => Some(1),
            _ => None,
        }
    }

    pub fn raises_the_window(&self) -> bool {
        self.id < PLAYER_ID_LIMIT
    }

    pub fn to_payload(&self, opcode: u16) -> Vec<u8> {
        let len = if opcode == INVITED {
            INVITE_LEN
        } else {
            ROSTER_LEN
        };

        let mut out = vec![0u8; len];

        out[at::CLASS] = self.class as u8;

        out[at::FLAG] = u8::from(!self.pending);

        out[at::LEVEL..][..2].copy_from_slice(&self.level.to_le_bytes());

        out[at::MAX_HP..][..2].copy_from_slice(&self.max_hp.to_le_bytes());

        out[at::HP..][..2].copy_from_slice(&self.hp.to_le_bytes());

        out[at::ID..][..2].copy_from_slice(&self.id.to_le_bytes());

        let name = codepage::encode(&self.name, Codepage::Windows1252);

        let n = name.len().min(NAME_LEN - 1);

        out[NAME_OFF..NAME_OFF + n].copy_from_slice(&name[..n]);

        out
    }
}

pub fn name_at(payload: &[u8]) -> Result<String> {
    let bytes = payload
        .get(NAME_OFF..NAME_OFF + NAME_LEN)
        .ok_or(Error::Truncated {
            kind: KIND,
            offset: NAME_OFF,
            needed: NAME_OFF + NAME_LEN,
            len: payload.len(),
        })?;

    let cut = bytes[..NAME_LEN - 1]
        .iter()
        .position(|b| *b == 0)
        .unwrap_or(NAME_LEN - 1);

    Ok(codepage::decode(&bytes[..cut], Codepage::Windows1252))
}

pub fn parse_member(opcode: u16, payload: &[u8]) -> Result<Member> {
    let needed = match opcode {
        JOINED => ROSTER_LEN,
        INVITED => INVITE_LEN,
        other => {
            return Err(Error::structure(
                KIND,
                format!("opcode {other:#06X} carries no roster row"),
            ));
        }
    };

    if payload.len() < needed {
        return Err(Error::Truncated {
            kind: KIND,
            offset: at::CLASS,
            needed,
            len: payload.len(),
        });
    }

    Ok(Member {
        id: u16_at(payload, at::ID, KIND)?,
        name: name_at(payload)?,
        class: payload[at::CLASS] as i8,
        level: u16_at(payload, at::LEVEL, KIND)? as i16,
        hp: u16_at(payload, at::HP, KIND)? as i16,
        max_hp: u16_at(payload, at::MAX_HP, KIND)? as i16,
        pending: payload[at::FLAG] == 0,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Leave {
    Dissolved,
    Member(u32),
}

impl Leave {
    pub fn from_dword(value: u32) -> Leave {
        if value == 0 {
            Leave::Dissolved
        } else {
            Leave::Member(value)
        }
    }

    pub fn dword(self) -> u32 {
        match self {
            Leave::Dissolved => 0,
            Leave::Member(id) => id,
        }
    }

    pub fn to_payload(self) -> [u8; LEAVE_LEN] {
        let mut out = [0u8; LEAVE_LEN];

        out[at::LEAVING..][..4].copy_from_slice(&self.dword().to_le_bytes());

        out
    }
}

pub fn parse_leave(payload: &[u8]) -> Result<Leave> {
    if payload.len() < LEAVE_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: at::LEAVING,
            needed: LEAVE_LEN,
            len: payload.len(),
        });
    }

    Ok(Leave::from_dword(u32_at(payload, at::LEAVING, KIND)?))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Marks {
    pub owner: u16,
    pub ids: [u16; MARKS],
}

impl Marks {
    pub fn row_for(index: Option<usize>, roster_len: usize) -> Option<usize> {
        let row = index.unwrap_or(roster_len);

        (row < ROWS).then_some(row)
    }

    pub fn marked(&self) -> usize {
        self.ids.iter().filter(|id| **id != 0).count()
    }

    pub fn to_payload(&self) -> [u8; ROSTER_LEN] {
        let mut out = [0u8; ROSTER_LEN];

        out[at::MARKS_OWNER..][..2].copy_from_slice(&self.owner.to_le_bytes());

        for (n, id) in self.ids.iter().enumerate() {
            out[at::MARKS + n * 2..][..2].copy_from_slice(&id.to_le_bytes());
        }

        out
    }
}

pub fn parse_marks(payload: &[u8]) -> Result<Marks> {
    if payload.len() < ROSTER_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: at::MARKS_OWNER,
            needed: ROSTER_LEN,
            len: payload.len(),
        });
    }

    let mut ids = [0u16; MARKS];

    for (n, id) in ids.iter_mut().enumerate() {
        *id = u16_at(payload, at::MARKS + n * 2, KIND)?;
    }

    Ok(Marks {
        owner: u16_at(payload, at::MARKS_OWNER, KIND)?,
        ids,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Accept {
    pub client_id: u16,
    pub inviter: u16,
    pub name: String,
}

impl Accept {
    pub fn to(invite: &Member, client_id: u16) -> Accept {
        Accept {
            client_id,
            inviter: invite.id,
            name: invite.name.clone(),
        }
    }

    pub fn to_payload(&self) -> [u8; ACCEPT_LEN] {
        let mut out = [0u8; ACCEPT_LEN];

        out[0..2].copy_from_slice(&self.inviter.to_le_bytes());

        let name = codepage::encode(&self.name, Codepage::Windows1252);

        let n = name.len().min(ACCEPT_LEN - 2 - 1);

        out[2..2 + n].copy_from_slice(&name[..n]);

        out
    }

    pub fn to_frame(&self) -> [u8; ACCEPT_FRAME_LEN] {
        let mut out = [0u8; ACCEPT_FRAME_LEN];

        out[4..6].copy_from_slice(&ACCEPT.to_le_bytes());

        out[6..8].copy_from_slice(&self.client_id.to_le_bytes());

        out[HEADER_LEN..].copy_from_slice(&self.to_payload());

        out
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    Joined(Member),
    Invited(Member),
    Left(Leave),
    Marked(Marks),
}

impl Change {
    pub fn opcode(&self) -> u16 {
        match self {
            Change::Joined(_) => JOINED,
            Change::Invited(_) => INVITED,
            Change::Left(_) => LEFT,
            Change::Marked(_) => MARKED,
        }
    }
}

pub fn parse(opcode: u16, payload: &[u8]) -> Result<Change> {
    match opcode {
        JOINED => parse_member(opcode, payload).map(Change::Joined),
        INVITED => parse_member(opcode, payload).map(Change::Invited),
        LEFT => parse_leave(payload).map(Change::Left),
        MARKED => parse_marks(payload).map(Change::Marked),
        other => Err(Error::structure(
            KIND,
            format!("opcode {other:#06X} is not a party change"),
        )),
    }
}
