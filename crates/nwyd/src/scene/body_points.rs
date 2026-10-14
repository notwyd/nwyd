use glam::Vec3;
use render::camera::Matrix4;

#[derive(Debug, Clone, Copy, Default, PartialEq)]

pub struct State {
    pub points: [Option<Vec3>; 11],
    pub last_ms: u32,
}

pub fn bindings(model: u32) -> [Option<(usize, Vec3)>; 11] {
    let mut out = [None; 11];

    let (bones, offset): (&[usize], Vec3) = match model {
        20 => (&[8, 9, 13, 16, 11, 18, 24, 31], Vec3::ZERO),
        0 | 1 | 54 | 56 | 57 => (&[8, 7, 29, 34], Vec3::ZERO),
        8 => (&[33, 34, 35, 40, 41, 42, 5], Vec3::ZERO),
        38 => (&[33, 16, 11, 21, 26, 5, 2], Vec3::ZERO),
        2 => (&[8, 27, 32], Vec3::ZERO),
        4 => (&[8, 25, 31], Vec3::ZERO),
        29 => (&[8], Vec3::ZERO),
        26 => (&[14, 22], Vec3::new(0.25, 0.15, 0.0)),
        7 => (&[6, 7, 12, 18, 23, 28], Vec3::ZERO),
        25 => (&[6], Vec3::ZERO),
        28 => (&[10], Vec3::ZERO),
        22 => (&[25], Vec3::ZERO),
        6 => (&[18], Vec3::new(0.0, 0.05, 0.05)),
        31 => (&[14, 19, 24, 29, 10], Vec3::new(0.3, 0.1, 0.0)),
        _ => (&[], Vec3::ZERO),
    };

    for (slot, &bone) in bones.iter().enumerate() {
        out[slot] = Some((bone, offset));
    }

    match model {
        20 => {
            out[8] = Some((7, Vec3::new(0.05, 0.37, 0.1)));

            out[9] = Some((7, Vec3::new(0.05, 0.37, -0.1)));

            out[10] = out[9];
        }
        0 | 1 | 54 | 56 | 57 => {
            out[0] = Some((8, Vec3::new(0.4, 0.2, 0.0)));

            let pair = formats::mesh::appearance::HAND_BONES
                .get(model as usize)
                .copied()
                .unwrap_or([0, 0]);

            out[6] = Some((pair[0] as usize, Vec3::ZERO));

            if pair[1] != pair[0] {
                out[7] = Some((pair[1] as usize, Vec3::ZERO));
            }

            out[8] = Some((8, Vec3::new(0.16, 0.08, 0.035)));

            out[9] = Some((8, Vec3::new(0.16, 0.08, -0.035)));

            out[10] = Some((8, Vec3::new(0.07, 0.1, 0.0)));
        }
        2 => out[0] = Some((8, Vec3::new(0.0, 0.2, 0.0))),
        4 => out[0] = Some((8, Vec3::new(-0.5, 0.0, 0.0))),
        31 => out[4] = Some((10, Vec3::new(-0.5, -0.25, 0.1))),
        45 => {
            out[8] = Some((8, Vec3::new(-0.5, 0.0, 0.05)));

            out[9] = Some((14, Vec3::new(0.1, 0.0, -0.1)));

            out[10] = Some((20, Vec3::new(0.1, 0.0, -0.1)));
        }
        46 => {
            out[8] = Some((8, Vec3::new(0.11, 0.06, 0.15)));

            out[9] = Some((8, Vec3::new(0.11, -0.03, 0.15)));

            out[10] = Some((9, Vec3::new(-0.11, 0.02, -0.15)));
        }
        _ => {}
    }

    out
}

impl State {
    pub fn publish(&mut self, model: u32, mut bone: impl FnMut(usize) -> Option<Matrix4>) {
        for (slot, binding) in bindings(model).into_iter().enumerate() {
            let Some((id, local)) = binding else {
                continue;
            };

            let Some(matrix) = bone(id) else {
                continue;
            };

            let at = matrix.transform_coord(local);

            if at.is_finite() {
                self.points[slot] = Some(at);
            }
        }
    }
}

pub(crate) fn publish(actor: &mut super::actors::Actor) {
    let tree = &actor.tree;

    actor.body_points.publish(actor.model_id, |id| {
        tree.slot(id).map(|_| Matrix4(tree.combined(id)))
    });
}
