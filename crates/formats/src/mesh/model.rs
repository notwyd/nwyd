use super::ani::AnimClip;
use super::bon::Skeleton;
use super::registry::ModelEntry;
use super::{Matrix4, identity};
use crate::error::{Error, Kind, Result};

#[derive(Debug, Clone)]
pub enum Slot<'a> {
    Loaded(&'a AnimClip),
    Missing,
}

#[derive(Debug, Clone)]
pub struct ModelAnimation {
    pub model_id: u32,
    pub name: String,
    pub num_parts: u32,
    pub num_anims: u32,
    pub skeleton: Skeleton,
    pub num_bones: usize,
    pub ani_num_bones: usize,
    frame_count: Vec<usize>,
    anim_base: Vec<usize>,
    matrices: Vec<Matrix4>,
    pub missing: usize,
    pub bone_count_conflicts: usize,
}

fn err(message: impl Into<String>) -> Error {
    Error::Structure {
        kind: Kind::Ani,
        message: message.into(),
    }
}

impl ModelAnimation {
    pub fn assemble(
        entry: &ModelEntry,
        skeleton: Skeleton,
        bon_len: usize,
        slots: &[Slot<'_>],
    ) -> Result<ModelAnimation> {
        let declared = entry.num_anims as usize;

        let considered = slots.len().min(declared);

        let mut ani_num_bones = 0usize;

        let mut frame_count = Vec::new();

        let mut anim_base = Vec::new();

        let mut matrices: Vec<Matrix4> = Vec::new();

        let mut missing = 0usize;

        let mut conflicts = 0usize;

        let mut total_frames = 0usize;

        for slot in &slots[..considered] {
            let clip = match slot {
                Slot::Missing => {
                    missing += 1;

                    continue;
                }
                Slot::Loaded(clip) => clip,
            };

            if ani_num_bones != 0 && clip.num_bones != ani_num_bones {
                conflicts += 1;
            }

            ani_num_bones = clip.num_bones;

            anim_base.push(total_frames);

            frame_count.push(clip.num_frames);

            total_frames += clip.num_frames;

            matrices.extend_from_slice(&clip.matrices);
        }

        if ani_num_bones == 0 {
            return Err(err(format!(
                "model {} ({}) loaded no clips out of {declared} slots",
                entry.model_id, entry.name
            )));
        }

        if conflicts == 0 && matrices.len() != total_frames * ani_num_bones {
            return Err(err(format!(
                "model {}: {} matrices for {total_frames} frames x {ani_num_bones} bones",
                entry.model_id,
                matrices.len(),
            )));
        }

        Ok(ModelAnimation {
            model_id: entry.model_id,
            name: entry.name.clone(),
            num_parts: entry.num_parts,
            num_anims: entry.num_anims,
            skeleton,
            num_bones: bon_len / super::bon::RECORD_LEN,
            ani_num_bones,
            frame_count,
            anim_base,
            matrices,
            missing,
            bone_count_conflicts: conflicts,
        })
    }

    pub fn clip_count(&self) -> usize {
        self.frame_count.len()
    }

    pub fn frames(&self, slot: usize) -> Option<usize> {
        self.frame_count.get(slot).copied()
    }

    pub fn anim_base(&self, slot: usize) -> Option<usize> {
        self.anim_base.get(slot).copied()
    }

    pub fn total_frames(&self) -> usize {
        self.frame_count.iter().sum()
    }

    pub fn keyframe(&self, frame: usize) -> Option<&[Matrix4]> {
        let start = frame.checked_mul(self.ani_num_bones)?;

        self.matrices.get(start..start + self.ani_num_bones)
    }

    pub fn matrices(&self) -> &[Matrix4] {
        &self.matrices
    }

    pub fn matrix_bytes(&self) -> usize {
        self.matrices.len() * super::MATRIX_BYTES
    }

    pub fn signed_matrix_byte(&self, requested: i32) -> i32 {
        let offset = match usize::try_from(requested) {
            Ok(offset) if offset < self.matrix_bytes() => offset,
            _ => 0,
        };

        let component = offset / size_of::<f32>();

        let byte = offset % size_of::<f32>();

        let matrix = &self.matrices[component / 16];

        i32::from(matrix[component % 16].to_le_bytes()[byte] as i8)
    }

    pub fn bind_pose(&self) -> Vec<Matrix4> {
        vec![identity(); self.ani_num_bones]
    }
}
