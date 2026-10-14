use glam::Vec3;
use render::cull::Aabb;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct State {
    pub opacity: f32,
    last: Option<u32>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            opacity: 1.0,
            last: None,
        }
    }
}

impl State {
    pub fn update(&mut self, now: u32, blocked: bool) {
        if self.last == Some(now) {
            return;
        }

        self.last = Some(now);

        self.opacity = if blocked {
            if self.opacity <= 0.75 {
                0.7
            } else {
                self.opacity - 0.02
            }
        } else if self.opacity >= 0.95 {
            1.0
        } else {
            self.opacity + 0.05
        };
    }
}

pub fn kind_enabled(kind: u32) -> bool {
    matches!(
        kind, 0x74E..= 0x753 | 0x755..= 0x760 | 0x763..= 0x767 | 0x775..= 0x779 | 0x77C
        ..= 0x77F | 0x76D | 0x78B | 0x78F..= 0x790
    )
}

pub fn enabled(kind: u32, position: Vec3) -> bool {
    let zone = (position.x as i32 >> 7, position.z as i32 >> 7);

    kind_enabled(kind) || matches!(zone, (31, 31) | (17..=19, 30..) | (13..=14, 28))
}

pub fn obstructs(bounds: &Aabb, eye: Vec3, target: Vec3) -> bool {
    let direction = target - eye;

    if !eye.is_finite() || !target.is_finite() || direction.length_squared() < 0.000001 {
        return false;
    }

    let mut near = 0.0_f32;

    let mut far = 1.0_f32;

    for axis in 0..3 {
        let (lo, hi) = (bounds.min[axis], bounds.max[axis]);

        if !lo.is_finite() || !hi.is_finite() || lo > hi {
            return false;
        }

        if direction[axis].abs() < 0.000001 {
            if eye[axis] < lo || eye[axis] > hi {
                return false;
            }
        } else {
            let a = (lo - eye[axis]) / direction[axis];

            let b = (hi - eye[axis]) / direction[axis];

            near = near.max(a.min(b));

            far = far.min(a.max(b));

            if near > far {
                return false;
            }
        }
    }

    true
}
