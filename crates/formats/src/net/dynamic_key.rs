use crate::error::{Error, Kind, Result};

pub const OPCODES: [u16; 4] = [0xBBF, 0xFBC, 0x13BD, 0x7BE];

const PERM20: [u32; 20] = [
    12, 11, 10, 9, 8, 13, 22, 21, 20, 7, 14, 23, 24, 19, 6, 15, 16, 17, 18, 5,
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Update {
    Value(u32),
    Experience { normal: u32, transcendent: u32 },
}

impl Update {
    pub fn resolve(self, evolution_tier: i32) -> u32 {
        match self {
            Self::Value(value) => value,
            Self::Experience {
                normal,
                transcendent,
            } => {
                if evolution_tier == 2 {
                    transcendent
                } else {
                    normal
                }
            }
        }
    }
}

fn bad(message: impl Into<String>) -> Error {
    Error::structure(Kind::DynamicKey, message)
}

fn index(value: i32, len: usize) -> Result<usize> {
    usize::try_from(value)
        .ok()
        .filter(|&n| n < len)
        .ok_or_else(|| bad(format!("index {value} outside {len} entries")))
}

fn word(payload: &[u8], offset: usize) -> Result<i32> {
    let at = offset - 12;

    let bytes = payload.get(at..at + 4).ok_or(Error::Truncated {
        kind: Kind::DynamicKey,
        offset: at,
        needed: 4,
        len: payload.len(),
    })?;

    Ok(i32::from_le_bytes(
        bytes.try_into().expect("four checked bytes"),
    ))
}

pub fn parse(opcode: u16, payload: &[u8]) -> Result<Update> {
    let opcode = if opcode == 0xBBF {
        let route = (word(payload, 0xB0)? / 3)
            .wrapping_mul(7)
            .wrapping_add(220)
            .wrapping_add((word(payload, 0xAC)? / 7).wrapping_mul(3))
            % 3;

        [0xFBC, 0x13BD, 0x7BE][index(route, 3)?]
    } else {
        opcode
    };

    match opcode {
        0xFBC => {
            let mut i = (!word(payload, 0x68)?) % 40;

            if i == 23 {
                i = 22;
            }

            Ok(Update::Value(word(payload, 12 + index(i, 40)? * 4)? as u32))
        }
        0x13BD => {
            let i = index(
                word(payload, 0x88)?
                    .wrapping_add(1)
                    .wrapping_mul(word(payload, 0x38)?)
                    % 400,
                400,
            )?;

            let normal = if i == 0 {
                0
            } else {
                super::exp::NORMAL[i - 1] as u32
            };

            let transcendent = if i == 0 {
                0
            } else {
                super::exp::TRANSCENDENT[i - 1] as u32
            };

            Ok(Update::Experience {
                normal: normal ^ 0xFFFF,
                transcendent: transcendent ^ 0xFFFF,
            })
        }
        0x7BE => {
            let selector = word(payload, 0x84)?;

            let selected = word(payload, 12 + index(selector % 30, 30)? * 4)?;

            Ok(Update::Value(
                PERM20[index(selected % 5 + (selector % 4) * 5, 20)?],
            ))
        }
        _ => Err(bad(format!("unsupported opcode {opcode:#x}"))),
    }
}
