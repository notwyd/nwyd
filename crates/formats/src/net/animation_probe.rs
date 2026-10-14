use crate::error::{Error, Kind, Result};
use crate::mesh::model::ModelAnimation;
use crate::util::u32_at;

const KIND: Kind = Kind::AnimationProbe;

pub const REQUEST: u16 = 0x1C1;

pub const RESPONSE: u16 = 0x2C2;

pub const FRAME_LEN: usize = 0x18;

pub const PAYLOAD_LEN: usize = 0x0C;

pub const HEADER_LEN: usize = FRAME_LEN - PAYLOAD_LEN;

pub const MODELS: usize = 100;

pub mod at {

    pub const MODEL: usize = 0x00;

    pub const BYTE_OFFSET: usize = 0x04;

    pub const RESULT: usize = 0x08;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Request {
    pub model: i32,
    pub byte_offset: i32,
    pub ignored_result: i32,
}

impl Request {
    pub fn effective_model(self) -> usize {
        usize::try_from(self.model)
            .ok()
            .filter(|&model| model < MODELS)
            .unwrap_or(0)
    }

    pub fn answer(self, animation: &ModelAnimation, client_id: u16) -> Response {
        Response {
            client_id,
            model: self.model,
            byte_offset: self.byte_offset,
            result: animation.signed_matrix_byte(self.byte_offset),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct Response {
    pub client_id: u16,
    pub model: i32,
    pub byte_offset: i32,
    pub result: i32,
}

impl Response {
    pub fn to_payload(self) -> [u8; PAYLOAD_LEN] {
        let mut out = [0; PAYLOAD_LEN];

        out[at::MODEL..at::MODEL + 4].copy_from_slice(&self.model.to_le_bytes());

        out[at::BYTE_OFFSET..at::BYTE_OFFSET + 4].copy_from_slice(&self.byte_offset.to_le_bytes());

        out[at::RESULT..at::RESULT + 4].copy_from_slice(&self.result.to_le_bytes());

        out
    }

    pub fn to_frame(self) -> [u8; FRAME_LEN] {
        let mut out = [0; FRAME_LEN];

        out[0x04..0x06].copy_from_slice(&RESPONSE.to_le_bytes());

        out[0x06..0x08].copy_from_slice(&self.client_id.to_le_bytes());

        out[HEADER_LEN..].copy_from_slice(&self.to_payload());

        out
    }
}

pub fn parse(payload: &[u8]) -> Result<Request> {
    if payload.len() < PAYLOAD_LEN {
        return Err(Error::Truncated {
            kind: KIND,
            offset: 0,
            needed: PAYLOAD_LEN,
            len: payload.len(),
        });
    }

    Ok(Request {
        model: u32_at(payload, at::MODEL, KIND)? as i32,
        byte_offset: u32_at(payload, at::BYTE_OFFSET, KIND)? as i32,
        ignored_result: u32_at(payload, at::RESULT, KIND)? as i32,
    })
}
