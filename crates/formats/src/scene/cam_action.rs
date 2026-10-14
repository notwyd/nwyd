use crate::error::{Error, Kind, Result};

pub const RECORD_LEN: usize = 0x1C;

pub const MAX_RECORDS: usize = 1000;

pub const CUTSCENE_DISTANCE: f32 = 11.0;

pub const PLAYBACK_LEAD_MS: u32 = 100;

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Keyframe {
    pub time_ms: u32,
    pub relative: u32,
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
}

impl Keyframe {
    pub fn is_relative(&self) -> bool {
        self.relative as u16 == 1
    }
}

#[derive(Debug, Clone, PartialEq)]

pub struct CamAction {
    pub keyframes: Vec<Keyframe>,
}

fn f32_at(rec: &[u8], off: usize) -> f32 {
    f32::from_le_bytes(rec[off..off + 4].try_into().expect("4 bytes"))
}

fn u32_at(rec: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(rec[off..off + 4].try_into().expect("4 bytes"))
}

pub fn parse(bytes: &[u8]) -> Result<CamAction> {
    let count = u32_at(
        bytes.get(..4).ok_or(Error::Truncated {
            kind: Kind::CamAction,
            offset: 0,
            needed: 4,
            len: bytes.len(),
        })?,
        0,
    ) as i32;

    if count < 0 {
        return Err(Error::structure(
            Kind::CamAction,
            format!("negative keyframe count {count}"),
        ));
    }

    let count = count as usize;

    if count > MAX_RECORDS {
        return Err(Error::structure(
            Kind::CamAction,
            format!("keyframe count {count} exceeds the {MAX_RECORDS}-frame buffer"),
        ));
    }

    let needed = 4 + count * RECORD_LEN;

    if bytes.len() != needed {
        return Err(Error::structure(
            Kind::CamAction,
            format!(
                "{count} keyframes need exactly {needed} bytes, file is {}",
                bytes.len()
            ),
        ));
    }

    let keyframes = bytes[4..]
        .chunks_exact(RECORD_LEN)
        .map(|rec| Keyframe {
            time_ms: u32_at(rec, 0x00),
            relative: u32_at(rec, 0x04),
            x: f32_at(rec, 0x08),
            y: f32_at(rec, 0x0C),
            z: f32_at(rec, 0x10),
            yaw: f32_at(rec, 0x14),
            pitch: f32_at(rec, 0x18),
        })
        .collect();

    Ok(CamAction { keyframes })
}

impl CamAction {
    pub fn len(&self) -> usize {
        self.keyframes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.keyframes.is_empty()
    }

    pub fn duration_ms(&self) -> u32 {
        self.keyframes.last().map_or(0, |k| k.time_ms)
    }

    pub fn is_monotonic(&self) -> bool {
        self.keyframes
            .windows(2)
            .all(|w| w[0].time_ms <= w[1].time_ms)
    }

    pub fn sample(&self, t: u32) -> Option<Sample> {
        for w in self.keyframes.windows(2) {
            let (a, b) = (w[0], w[1]);

            if a.time_ms < t && t < b.time_ms {
                let span = (b.time_ms - a.time_ms) as f32;

                let f = (t - a.time_ms) as f32 / span;

                let g = 1.0 - f;

                return Some(Sample {
                    x: a.x * g + b.x * f,
                    y: a.y * g + b.y * f,
                    z: a.z * g + b.z * f,
                    yaw: a.yaw * g + b.yaw * f,
                    pitch: a.pitch * g + b.pitch * f,
                    relative: a.is_relative(),
                });
            }
        }

        None
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]

pub struct Sample {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub yaw: f32,
    pub pitch: f32,
    pub relative: bool,
}
