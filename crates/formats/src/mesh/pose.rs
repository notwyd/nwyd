use super::bon::{MAX_BONES, Skeleton};
use super::{Matrix4, identity, matrix_multiply};

#[derive(Debug, Clone, Copy)]

pub struct ClipTable<'a> {
    pub matrices: &'a [Matrix4],
    pub bones: usize,
}

impl<'a> ClipTable<'a> {
    pub fn new(matrices: &'a [Matrix4], bones: usize) -> ClipTable<'a> {
        ClipTable { matrices, bones }
    }

    pub fn get(&self, frame: usize, bone: usize) -> Option<&'a Matrix4> {
        if bone >= self.bones {
            return None;
        }

        self.matrices.get(frame * self.bones + bone)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub enum Smoothing {
    On,
    Off,
}

#[derive(Debug, Clone, Copy)]

pub struct PoseRequest {
    pub model: u32,
    pub slot: usize,
    pub anim_base: usize,
    pub frames: usize,
    pub sub_frame_ms: u32,
    pub elapsed_ms: u32,
    pub blend_from: u32,
    pub blend_matrix: usize,
    pub smoothing: Smoothing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct PoseResult {
    pub sub: u32,
    pub frame: usize,
    pub blended: bool,
    pub clears_blend_from: bool,
    pub written: usize,
}

#[derive(Debug, Clone)]

pub struct FrameNode {
    pub bone_id: i32,
    pub parent_id: i32,
    pub local: Matrix4,
    pub combined: Matrix4,
    sibling: Option<usize>,
    first_child: Option<usize>,
}

impl FrameNode {
    pub fn sibling(&self) -> Option<usize> {
        self.sibling
    }

    pub fn first_child(&self) -> Option<usize> {
        self.first_child
    }

    fn new(bone_id: i32) -> FrameNode {
        FrameNode {
            bone_id,
            parent_id: -1,
            local: identity(),
            combined: identity(),
            sibling: None,
            first_child: None,
        }
    }
}

#[derive(Debug, Clone)]

pub struct FrameTree {
    nodes: Vec<FrameNode>,
    slots: Vec<Option<usize>>,
}

impl FrameTree {
    pub fn build(skeleton: &Skeleton) -> FrameTree {
        let mut nodes = vec![FrameNode::new(0)];

        let mut slots = vec![None; MAX_BONES];

        slots[0] = Some(0);

        for r in &skeleton.records {
            let node = nodes.len();

            nodes.push(FrameNode {
                bone_id: r.bone,
                parent_id: r.parent,
                ..FrameNode::new(r.bone)
            });

            slots[r.bone as usize] = Some(node);

            let wanted = if r.parent == -1 { 0 } else { r.parent };

            if let Some(parent) = find_frame(&nodes, 0, wanted) {
                attach_child(&mut nodes, parent, node);
            }
        }

        FrameTree { nodes, slots }
    }

    pub fn root(&self) -> usize {
        0
    }

    pub fn slot(&self, bone: usize) -> Option<usize> {
        self.slots.get(bone).copied().flatten()
    }

    pub fn node(&self, index: usize) -> &FrameNode {
        &self.nodes[index]
    }

    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn children(&self, index: usize) -> Vec<usize> {
        let mut out = Vec::new();

        let mut cur = self.nodes[index].first_child;

        while let Some(n) = cur {
            out.push(n);

            cur = self.nodes[n].sibling;
        }

        out
    }

    pub fn sample(&mut self, req: &PoseRequest, clips: &ClipTable<'_>) -> Option<PoseResult> {
        if req.model > 100 {
            return None;
        }

        let frames = if req.model == 49 {
            req.frames.checked_sub(2)?
        } else {
            req.frames
        };

        if frames == 0 || clips.bones >= 101 {
            return None;
        }

        let step = if req.sub_frame_ms == 0 {
            30
        } else {
            req.sub_frame_ms
        };

        let period = (frames as u32) * 4;

        let sub = (req.elapsed_ms / step) % period;

        let frame = (sub >> 2) as usize;

        let base = req.anim_base + frame;

        let interpolating = frames > 1 && req.smoothing == Smoothing::On;

        let mut written = 0usize;

        let mut blended = false;

        let mut clears_blend_from = false;

        for bone in 0..clips.bones {
            let Some(node) = self.slot(bone) else {
                continue;
            };

            let Some(cur) = clips.get(base, bone) else {
                continue;
            };

            written += 1;

            if !interpolating {
                self.nodes[node].local = *cur;

                continue;
            }

            if req.blend_from != 0 && sub <= 9 {
                blended = true;

                let Some(prev) = clips.matrices.get(req.blend_matrix + bone) else {
                    self.nodes[node].local = *cur;

                    continue;
                };

                let w = (10 - sub) as f32;

                let s = sub as f32;

                let mut m = [0f32; 16];

                for k in 0..16 {
                    m[k] = (s * cur[k] + prev[k] * w) / 10.0;
                }

                self.nodes[node].local = m;
            } else {
                clears_blend_from = true;

                let q = sub & 3;

                if q == 0 {
                    self.nodes[node].local = *cur;

                    continue;
                }

                let next_frame = if sub < period - 3 {
                    base + 1
                } else {
                    req.anim_base
                };

                let Some(next) = clips.get(next_frame, bone) else {
                    self.nodes[node].local = *cur;

                    continue;
                };

                let mut m = [0f32; 16];

                for k in 0..16 {
                    m[k] = match q {
                        1 => (cur[k] + cur[k] + cur[k] + next[k]) * 0.25,
                        2 => (next[k] + cur[k]) * 0.5,
                        _ => (cur[k] + next[k] + next[k] + next[k]) * 0.25,
                    };
                }

                self.nodes[node].local = m;
            }
        }

        Some(PoseResult {
            sub,
            frame,
            blended,
            clears_blend_from,
            written,
        })
    }

    pub fn combine(&mut self, world: &Matrix4) {
        self.combine_from(self.root(), world);
    }

    fn combine_from(&mut self, start: usize, parent: &Matrix4) {
        let mut cur = Some(start);

        while let Some(node) = cur {
            let combined = matrix_multiply(&self.nodes[node].local, parent);

            self.nodes[node].combined = combined;

            if self.nodes[node].first_child.is_some() {
                if !(-1000.0..=1000.0).contains(&combined[0]) {
                    break;
                }

                let child = self.nodes[node].first_child.expect("checked above");

                self.combine_from(child, &combined);
            }

            cur = self.nodes[node].sibling;
        }
    }

    pub fn combined(&self, bone: usize) -> Matrix4 {
        match self.slot(bone) {
            Some(n) => self.nodes[n].combined,
            None => identity(),
        }
    }

    pub fn last_combined(&self, bones: &[i32]) -> Option<Matrix4> {
        fn walk(nodes: &[FrameNode], start: usize, bones: &[i32], found: &mut Option<Matrix4>) {
            let mut cur = Some(start);

            while let Some(node) = cur {
                if bones.contains(&nodes[node].bone_id) {
                    *found = Some(nodes[node].combined);
                }

                if let Some(child) = nodes[node].first_child {
                    walk(nodes, child, bones, found);
                }

                cur = nodes[node].sibling;
            }
        }

        let mut found = None;

        walk(&self.nodes, self.root(), bones, &mut found);

        found
    }

    pub fn set_local(&mut self, bone: usize, local: Matrix4) {
        if let Some(n) = self.slot(bone) {
            self.nodes[n].local = local;
        }
    }
}

pub fn skin_matrix(bone_offset: &Matrix4, combined: &Matrix4) -> Matrix4 {
    matrix_multiply(bone_offset, combined)
}

fn find_frame(nodes: &[FrameNode], from: usize, bone: i32) -> Option<usize> {
    if nodes[from].bone_id == bone {
        return Some(from);
    }

    if let Some(child) = nodes[from].first_child
        && let Some(found) = find_frame(nodes, child, bone)
    {
        return Some(found);
    }

    if let Some(sibling) = nodes[from].sibling {
        return find_frame(nodes, sibling, bone);
    }

    None
}

fn attach_child(nodes: &mut [FrameNode], parent: usize, child: usize) {
    match nodes[parent].first_child {
        None => nodes[parent].first_child = Some(child),
        Some(first) => {
            nodes[child].sibling = nodes[first].sibling;

            nodes[first].sibling = Some(child);
        }
    }
}
