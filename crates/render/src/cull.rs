use crate::camera::Matrix4;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Aabb {
    pub min: [f32; 3],
    pub max: [f32; 3],
}

impl Aabb {
    pub const EMPTY: Aabb = Aabb {
        min: [f32::INFINITY; 3],
        max: [f32::NEG_INFINITY; 3],
    };

    pub fn grow(&mut self, p: [f32; 3]) {
        for (a, v) in p.iter().enumerate() {
            self.min[a] = self.min[a].min(*v);

            self.max[a] = self.max[a].max(*v);
        }
    }

    pub fn translated(&self, dx: f32, dy: f32, dz: f32) -> Aabb {
        Aabb {
            min: [self.min[0] + dx, self.min[1] + dy, self.min[2] + dz],
            max: [self.max[0] + dx, self.max[1] + dy, self.max[2] + dz],
        }
    }

    pub fn transformed(&self, m: &Matrix4) -> Aabb {
        let mut out = Aabb::EMPTY;

        for i in 0..8u8 {
            let pick = |a: usize| {
                if i >> a & 1 == 0 {
                    self.min[a]
                } else {
                    self.max[a]
                }
            };

            let p = m.transform_coord(glam::Vec3::new(pick(0), pick(1), pick(2)));

            out.grow([p.x, p.y, p.z]);
        }

        out
    }

    pub fn contains(&self, p: [f32; 3]) -> bool {
        (0..3).all(|a| p[a] >= self.min[a] && p[a] <= self.max[a])
    }

    pub fn admits_nothing(&self) -> bool {
        (0..3).any(|a| self.min[a] > self.max[a])
    }

    fn positive_corner(&self, n: [f32; 3]) -> [f32; 3] {
        std::array::from_fn(|a| {
            if n[a] >= 0.0 {
                self.max[a]
            } else {
                self.min[a]
            }
        })
    }
}

pub fn clip_matrix(uniforms: &crate::world::FrameUniforms) -> Matrix4 {
    Matrix4::multiply(
        &Matrix4::multiply(
            &Matrix4(uniforms.world).transpose(),
            &Matrix4(uniforms.view).transpose(),
        ),
        &Matrix4(uniforms.projection).transpose(),
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Frustum {
    planes: [[f32; 4]; 6],
}

impl Frustum {
    pub const ADMITS_EVERYTHING: Frustum = Frustum {
        planes: [[0.0, 0.0, 0.0, 1.0]; 6],
    };

    pub fn from_clip(clip: &Matrix4) -> Frustum {
        let m = clip.as_d3dx();

        let col = |c: usize| [m[c], m[4 + c], m[8 + c], m[12 + c]];

        let add = |a: [f32; 4], b: [f32; 4]| std::array::from_fn(|i| a[i] + b[i]);

        let sub = |a: [f32; 4], b: [f32; 4]| std::array::from_fn(|i| a[i] - b[i]);

        let (x, y, z, w) = (col(0), col(1), col(2), col(3));

        Frustum {
            planes: [add(w, x), sub(w, x), add(w, y), sub(w, y), z, sub(w, z)],
        }
    }

    pub fn from_frame(uniforms: &crate::world::FrameUniforms) -> Frustum {
        Frustum::from_clip(&clip_matrix(uniforms))
    }

    pub fn admits(&self, aabb: &Aabb) -> bool {
        if aabb.admits_nothing() {
            return false;
        }

        for p in &self.planes {
            let n = [p[0], p[1], p[2]];

            let c = aabb.positive_corner(n);

            if n[0] * c[0] + n[1] * c[1] + n[2] * c[2] + p[3] < 0.0 {
                return false;
            }
        }

        true
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use glam::Vec3;

    #[test]
    fn growing_empty_bounds_includes_extrema_and_translates_them() {
        let mut bounds = Aabb::EMPTY;

        assert!(bounds.admits_nothing());

        assert!(!Frustum::ADMITS_EVERYTHING.admits(&bounds));

        for point in [[2.0, -3.0, 8.0], [-4.0, 7.0, 1.0], [1.0, 2.0, 3.0]] {
            bounds.grow(point);

            assert!(bounds.contains(point));
        }

        assert_eq!(
            bounds,
            Aabb {
                min: [-4.0, -3.0, 1.0],
                max: [2.0, 7.0, 8.0]
            }
        );

        assert!(bounds.contains(bounds.min));

        assert!(bounds.contains(bounds.max));

        assert!(!bounds.contains([2.1, 0.0, 2.0]));

        assert_eq!(
            bounds.translated(10.0, 20.0, -1.0),
            Aabb {
                min: [6.0, 17.0, 0.0],
                max: [12.0, 27.0, 7.0]
            }
        );
    }

    #[test]
    fn rotated_bounds_enclose_all_corners_after_translation() {
        let bounds = Aabb {
            min: [-2.0, -1.0, -3.0],
            max: [4.0, 5.0, 6.0],
        };

        let matrix = Matrix4::multiply(
            &Matrix4::rotation_z(std::f32::consts::FRAC_PI_2),
            &Matrix4::translation(10.0, 20.0, 30.0),
        );

        let transformed = bounds.transformed(&matrix);

        assert!(Vec3::from(transformed.min).abs_diff_eq(Vec3::new(5.0, 18.0, 27.0), 1e-5));

        assert!(Vec3::from(transformed.max).abs_diff_eq(Vec3::new(11.0, 24.0, 36.0), 1e-5));
    }

    #[test]
    fn frustum_clips_all_six_planes_but_retains_touching_bounds() {
        let frustum = Frustum::from_clip(&Matrix4::IDENTITY);

        for point in [
            [-1.0, 0.0, 0.5],
            [1.0, 0.0, 0.5],
            [0.0, -1.0, 0.5],
            [0.0, 1.0, 0.5],
            [0.0, 0.0, 0.0],
            [0.0, 0.0, 1.0],
        ] {
            assert!(
                frustum.admits(&Aabb {
                    min: point,
                    max: point
                }),
                "{point:?}"
            );
        }

        for point in [
            [-1.01, 0.0, 0.5],
            [1.01, 0.0, 0.5],
            [0.0, -1.01, 0.5],
            [0.0, 1.01, 0.5],
            [0.0, 0.0, -0.01],
            [0.0, 0.0, 1.01],
        ] {
            assert!(
                !frustum.admits(&Aabb {
                    min: point,
                    max: point
                }),
                "{point:?}"
            );
        }

        assert!(frustum.admits(&Aabb {
            min: [-2.0, -2.0, -2.0],
            max: [0.0, 0.0, 0.5]
        }));
    }

    #[test]
    fn clip_matrix_decodes_shader_constants_in_world_view_projection_order() {
        use bytemuck::Zeroable;

        let world = Matrix4::translation(2.0, 3.0, 4.0);

        let view = Matrix4::look_at_lh(Vec3::new(1.0, 2.0, -5.0), Vec3::ZERO, Vec3::Y);

        let projection = Matrix4::perspective_fov_lh(1.0, 1.5, 0.5, 50.0);

        let frame = crate::world::FrameUniforms {
            world: world.shader_constants(),
            view: view.shader_constants(),
            projection: projection.shader_constants(),
            ..crate::world::FrameUniforms::zeroed()
        };

        let point = Vec3::new(-1.0, 0.5, 2.0);

        let expected =
            projection.transform_coord(view.transform_coord(world.transform_coord(point)));

        assert!(
            clip_matrix(&frame)
                .transform_coord(point)
                .abs_diff_eq(expected, 1e-5)
        );
    }
}
