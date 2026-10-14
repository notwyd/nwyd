use crate::error::{Error, Kind as ErrorKind, Result};
use crate::net::ground_item::HEADER_LEN;
use crate::util::u32_at;

const KIND: ErrorKind = ErrorKind::Warehouse;

pub const SET: u16 = 0x339;

pub const WITHDRAW: u16 = 0x387;

pub const DEPOSIT: u16 = 0x388;

pub const FAMILY: [u16; 3] = [SET, DEPOSIT, WITHDRAW];

pub const FRAME_LEN: usize = 0x10;

pub const LEN: usize = FRAME_LEN - HEADER_LEN;

pub const BROADCAST_EVENT: i32 = 0x10;

pub const BROADCAST_ARGS: [i32; 2] = [1, 2];

pub mod at {

    pub const AMOUNT: usize = 0x00;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Set,
    Deposit,
    Withdraw,
}

impl Change {
    pub fn from_opcode(opcode: u16) -> Option<Change> {
        match opcode {
            SET => Some(Change::Set),
            DEPOSIT => Some(Change::Deposit),
            WITHDRAW => Some(Change::Withdraw),
            _ => None,
        }
    }

    pub fn opcode(self) -> u16 {
        match self {
            Change::Set => SET,
            Change::Deposit => DEPOSIT,
            Change::Withdraw => WITHDRAW,
        }
    }

    pub fn repaints_the_money_labels(self) -> bool {
        !matches!(self, Change::Set)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Balances {
    pub carried: u32,
    pub warehouse: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Move {
    pub change: Change,
    pub amount: u32,
}

impl Move {
    pub fn apply(&self, before: Balances) -> Balances {
        match self.change {
            Change::Set => Balances {
                warehouse: self.amount,
                ..before
            },
            Change::Deposit => Balances {
                warehouse: before.warehouse.wrapping_add(self.amount),
                carried: before.carried.wrapping_sub(self.amount),
            },
            Change::Withdraw => Balances {
                warehouse: before.warehouse.wrapping_sub(self.amount),
                carried: before.carried.wrapping_add(self.amount),
            },
        }
    }

    pub fn to_bytes(&self) -> [u8; LEN] {
        let mut out = [0u8; LEN];

        out[at::AMOUNT..][..4].copy_from_slice(&self.amount.to_le_bytes());

        out
    }
}

pub fn parse(opcode: u16, payload: &[u8]) -> Result<Move> {
    let Some(change) = Change::from_opcode(opcode) else {
        return Err(Error::structure(
            KIND,
            format!("opcode {opcode:#06X} is not a warehouse gold change"),
        ));
    };

    if payload.len() < LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: at::AMOUNT,
            needed: LEN,
            len: payload.len(),
        });
    }

    Ok(Move {
        change,
        amount: u32_at(payload, at::AMOUNT, KIND)?,
    })
}
