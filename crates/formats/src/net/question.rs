use crate::error::{Error, Kind, Result};
use crate::text::codepage::{self, Codepage};

const KIND: Kind = Kind::Question;

pub const QUESTION: u16 = 0x1C6;

pub const CHOICE: u16 = 0x2C7;

pub const QUESTION_LEN: usize = 0x80;

pub const ANSWER_LEN: usize = 0x20;

pub const ANSWERS: usize = 4;

pub const MIN_PAYLOAD: usize = QUESTION_LEN + ANSWERS * ANSWER_LEN;

pub const HEADER_LEN: usize = 0x0C;

pub const CHOICE_FRAME_LEN: usize = 0x10;

#[derive(Debug, Clone, PartialEq, Eq)]

pub struct Question {
    pub text: String,
    pub answers: [String; ANSWERS],
    pub tail: Vec<u8>,
}

fn forced_nul_text(record: &[u8]) -> String {
    let text = &record[..record.len() - 1];

    let end = text
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(text.len());

    codepage::decode(&text[..end], Codepage::Windows1252)
}

pub fn parse(payload: &[u8]) -> Result<Question> {
    if payload.len() < MIN_PAYLOAD {
        return Err(Error::Truncated {
            kind: KIND,
            offset: 0,
            needed: MIN_PAYLOAD,
            len: payload.len(),
        });
    }

    let answers = std::array::from_fn(|index| {
        let start = QUESTION_LEN + index * ANSWER_LEN;

        forced_nul_text(&payload[start..start + ANSWER_LEN])
    });

    Ok(Question {
        text: forced_nul_text(&payload[..QUESTION_LEN]),
        answers,
        tail: payload[MIN_PAYLOAD..].to_vec(),
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct ChoiceResponse {
    pub client_id: u16,
    pub command: u32,
}

impl ChoiceResponse {
    pub fn new(client_id: u16, command: u32) -> Result<Self> {
        if command >= ANSWERS as u32 {
            return Err(Error::structure(
                KIND,
                format!("choice command {command} is outside 0..=3"),
            ));
        }

        Ok(Self { client_id, command })
    }

    pub fn to_frame(self) -> [u8; CHOICE_FRAME_LEN] {
        let mut out = [0u8; CHOICE_FRAME_LEN];

        out[0x04..0x06].copy_from_slice(&CHOICE.to_le_bytes());

        out[0x06..0x08].copy_from_slice(&self.client_id.to_le_bytes());

        out[HEADER_LEN..].copy_from_slice(&self.command.to_le_bytes());

        out
    }
}
