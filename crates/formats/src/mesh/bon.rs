use crate::error::{Error, Kind, Result};

pub const RECORD_LEN: usize = 8;

pub const MAX_BONES: usize = 100;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BoneRecord {
    pub parent: i32,
    pub bone: i32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skeleton {
    pub records: Vec<BoneRecord>,
    parent_of: Vec<Option<i32>>,
}

fn err(message: impl Into<String>) -> Error {
    Error::Structure {
        kind: Kind::Bon,
        message: message.into(),
    }
}

pub fn parse(bytes: &[u8]) -> Result<Skeleton> {
    if !bytes.len().is_multiple_of(RECORD_LEN) {
        return Err(err(format!(
            "file of {} bytes is not a whole number of 8-byte records",
            bytes.len()
        )));
    }

    if bytes.is_empty() {
        return Err(err("skeleton has no records"));
    }

    let records: Vec<BoneRecord> = bytes
        .chunks_exact(RECORD_LEN)
        .map(|c| BoneRecord {
            parent: i32::from_le_bytes(c[0..4].try_into().expect("chunk is 8 bytes")),
            bone: i32::from_le_bytes(c[4..8].try_into().expect("chunk is 8 bytes")),
        })
        .collect();

    let mut parent_of = vec![None; MAX_BONES];

    for r in &records {
        if r.bone < 0 || r.bone as usize >= MAX_BONES {
            return Err(err(format!(
                "bone id {} is outside the {MAX_BONES}-slot frame array",
                r.bone
            )));
        }

        if r.parent < -1 || r.parent >= MAX_BONES as i32 {
            return Err(err(format!(
                "bone {} has out-of-range parent {}",
                r.bone, r.parent
            )));
        }

        parent_of[r.bone as usize] = Some(r.parent);
    }

    Ok(Skeleton { records, parent_of })
}

impl Skeleton {
    pub fn max_bone_id(&self) -> i32 {
        self.records.iter().map(|r| r.bone).max().unwrap_or(0)
    }

    pub fn parent_of(&self, bone: i32) -> Option<i32> {
        usize::try_from(bone)
            .ok()
            .and_then(|b| self.parent_of.get(b).copied().flatten())
    }

    pub fn defines(&self, bone: i32) -> bool {
        self.parent_of(bone).is_some()
    }

    pub fn topological_order(&self) -> Vec<i32> {
        let mut out = Vec::with_capacity(self.records.len());

        let mut emitted = [false; MAX_BONES];

        loop {
            let mut progressed = false;

            for r in &self.records {
                let id = r.bone as usize;

                if emitted[id] {
                    continue;
                }

                let ready = match self.parent_of(r.bone) {
                    Some(-1) | None => true,
                    Some(p) => emitted[p as usize] || !self.defines(p),
                };

                if ready {
                    emitted[id] = true;

                    out.push(r.bone);

                    progressed = true;
                }
            }

            if !progressed {
                break;
            }
        }

        out
    }
}
