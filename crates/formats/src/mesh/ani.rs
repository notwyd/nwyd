use super::{MATRIX_BYTES, Matrix4, read_matrix};
use crate::error::{Error, Kind, Result};
use crate::util::u32_at;

const HEADER_LEN: usize = 8;

#[derive(Debug, Clone, PartialEq)]

pub struct AnimClip {
    pub num_frames: usize,
    pub num_bones: usize,
    pub matrices: Vec<Matrix4>,
}

fn err(message: impl Into<String>) -> Error {
    Error::Structure {
        kind: Kind::Ani,
        message: message.into(),
    }
}

pub fn parse(bytes: &[u8]) -> Result<AnimClip> {
    let num_frames = u32_at(bytes, 0, Kind::Ani)? as usize;

    let num_bones = u32_at(bytes, 4, Kind::Ani)? as usize;

    if num_frames == 0 || num_bones == 0 {
        return Err(err(format!(
            "degenerate clip: {num_frames} frames x {num_bones} bones"
        )));
    }

    let count = num_frames
        .checked_mul(num_bones)
        .ok_or_else(|| err("frame x bone count overflows"))?;

    let needed = count
        .checked_mul(MATRIX_BYTES)
        .and_then(|n| n.checked_add(HEADER_LEN))
        .ok_or_else(|| err("matrix block overflows"))?;

    if bytes.len() < needed {
        return Err(Error::Truncated {
            kind: Kind::Ani,
            offset: HEADER_LEN,
            needed,
            len: bytes.len(),
        });
    }

    let matrices = (0..count)
        .map(|i| read_matrix(&bytes[HEADER_LEN + i * MATRIX_BYTES..]))
        .collect();

    Ok(AnimClip {
        num_frames,
        num_bones,
        matrices,
    })
}

impl AnimClip {
    pub fn frame(&self, index: usize) -> Option<&[Matrix4]> {
        if index >= self.num_frames {
            return None;
        }

        let start = index * self.num_bones;

        self.matrices.get(start..start + self.num_bones)
    }

    pub fn bone(&self, frame: usize, bone: usize) -> Option<&Matrix4> {
        if bone >= self.num_bones {
            return None;
        }

        self.frame(frame)?.get(bone)
    }

    pub fn encoded_len(&self) -> usize {
        HEADER_LEN + self.num_frames * self.num_bones * MATRIX_BYTES
    }
}
