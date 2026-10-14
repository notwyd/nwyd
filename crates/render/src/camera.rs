use glam::Vec3;

pub const FOV_FRACTION: f32 = 0.25;

#[allow(clippy::approx_constant)]
pub const PI: f32 = 3.1415927;

pub const NEAR_BASE: f32 = 0.7;

pub const NEAR_SCALE: f32 = 1.4;

pub const FAR_INITIAL: f32 = 540.0;

pub const FAR_SKY: f32 = 70.0;

pub const FAR_SKY_SPECIAL_ZONE: f32 = 90.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Matrix4(pub [f32; 16]);

impl Matrix4 {
    pub const IDENTITY: Matrix4 = Matrix4([
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);

    pub fn perspective_fov_lh(fov_y: f32, aspect: f32, zn: f32, zf: f32) -> Matrix4 {
        let y_scale = 1.0 / (fov_y * 0.5).tan();

        let x_scale = y_scale / aspect;

        let q = zf / (zf - zn);

        Matrix4([
            x_scale,
            0.0,
            0.0,
            0.0,
            0.0,
            y_scale,
            0.0,
            0.0,
            0.0,
            0.0,
            q,
            1.0,
            0.0,
            0.0,
            -zn * q,
            0.0,
        ])
    }

    pub fn look_at_lh(eye: Vec3, at: Vec3, up: Vec3) -> Matrix4 {
        let z = (at - eye).normalize();

        let x = up.cross(z).normalize();

        let y = z.cross(x);

        Matrix4([
            x.x,
            y.x,
            z.x,
            0.0,
            x.y,
            y.y,
            z.y,
            0.0,
            x.z,
            y.z,
            z.z,
            0.0,
            -x.dot(eye),
            -y.dot(eye),
            -z.dot(eye),
            1.0,
        ])
    }

    pub fn scaling(x: f32, y: f32, z: f32) -> Matrix4 {
        let mut m = Matrix4::IDENTITY;

        m.0[0] = x;

        m.0[5] = y;

        m.0[10] = z;

        m
    }

    pub fn translation(x: f32, y: f32, z: f32) -> Matrix4 {
        let mut m = Matrix4::IDENTITY;

        m.0[12] = x;

        m.0[13] = y;

        m.0[14] = z;

        m
    }

    pub fn rotation_quaternion([x, y, z, w]: [f32; 4]) -> Matrix4 {
        Matrix4([
            1.0 - 2.0 * (y * y + z * z),
            2.0 * (x * y + z * w),
            2.0 * (x * z - y * w),
            0.0,
            2.0 * (x * y - z * w),
            1.0 - 2.0 * (x * x + z * z),
            2.0 * (y * z + x * w),
            0.0,
            2.0 * (x * z + y * w),
            2.0 * (y * z - x * w),
            1.0 - 2.0 * (x * x + y * y),
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
        ])
    }

    pub fn rotation_x(theta: f32) -> Matrix4 {
        let (s, c) = theta.sin_cos();

        let mut m = Matrix4::IDENTITY;

        m.0[5] = c;

        m.0[6] = s;

        m.0[9] = -s;

        m.0[10] = c;

        m
    }

    pub fn rotation_y(theta: f32) -> Matrix4 {
        let (s, c) = theta.sin_cos();

        let mut m = Matrix4::IDENTITY;

        m.0[0] = c;

        m.0[2] = -s;

        m.0[8] = s;

        m.0[10] = c;

        m
    }

    pub fn rotation_z(theta: f32) -> Matrix4 {
        let (s, c) = theta.sin_cos();

        let mut m = Matrix4::IDENTITY;

        m.0[0] = c;

        m.0[1] = s;

        m.0[4] = -s;

        m.0[5] = c;

        m
    }

    pub fn rotation_axis(axis: glam::Vec3, angle: f32) -> Matrix4 {
        let (s, c) = angle.sin_cos();

        let (x, y, z) = (axis.x, axis.y, axis.z);

        let k = 1.0 - c;

        let mut m = Matrix4::IDENTITY;

        m.0[0] = k * x * x + c;

        m.0[1] = k * x * y + s * z;

        m.0[2] = k * x * z - s * y;

        m.0[4] = k * y * x - s * z;

        m.0[5] = k * y * y + c;

        m.0[6] = k * y * z + s * x;

        m.0[8] = k * z * x + s * y;

        m.0[9] = k * z * y - s * x;

        m.0[10] = k * z * z + c;

        m
    }

    pub fn rotation_yaw_pitch_roll(yaw: f32, pitch: f32, roll: f32) -> Matrix4 {
        let m = Matrix4::multiply(&Matrix4::rotation_z(roll), &Matrix4::rotation_x(pitch));

        Matrix4::multiply(&m, &Matrix4::rotation_y(yaw))
    }

    pub fn multiply(a: &Matrix4, b: &Matrix4) -> Matrix4 {
        let mut out = [0.0f32; 16];

        for r in 0..4 {
            for c in 0..4 {
                out[r * 4 + c] = (0..4).map(|k| a.0[r * 4 + k] * b.0[k * 4 + c]).sum();
            }
        }

        Matrix4(out)
    }

    pub fn transpose(&self) -> Matrix4 {
        let m = &self.0;

        let mut out = [0.0f32; 16];

        for r in 0..4 {
            for c in 0..4 {
                out[c * 4 + r] = m[r * 4 + c];
            }
        }

        Matrix4(out)
    }

    pub fn as_d3dx(&self) -> [f32; 16] {
        self.0
    }

    pub fn shader_constants(&self) -> [f32; 16] {
        self.transpose().0
    }

    pub fn transform_coord(&self, v: Vec3) -> Vec3 {
        let m = &self.0;

        let x = v.x * m[0] + v.y * m[4] + v.z * m[8] + m[12];

        let y = v.x * m[1] + v.y * m[5] + v.z * m[9] + m[13];

        let z = v.x * m[2] + v.y * m[6] + v.z * m[10] + m[14];

        let w = v.x * m[3] + v.y * m[7] + v.z * m[11] + m[15];

        Vec3::new(x / w, y / w, z / w)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Projection {
    pub fov_y: f32,
    pub aspect: f32,
    pub near: f32,
    pub far: f32,
}

impl Projection {
    pub fn new(backbuffer: (u32, u32), inset: (u32, u32), far: f32) -> Projection {
        Projection {
            fov_y: FOV_FRACTION * PI,
            aspect: (backbuffer.0 - inset.0) as f32 / (backbuffer.1 - inset.1) as f32,
            near: NEAR_BASE * NEAR_SCALE,
            far,
        }
    }

    pub fn matrix(&self) -> Matrix4 {
        Matrix4::perspective_fov_lh(self.fov_y, self.aspect, self.near, self.far)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct View {
    pub eye: Vec3,
    pub at: Vec3,
    pub up: Vec3,
}

impl View {
    pub fn from_pose(eye: Vec3, yaw: f32, pitch: f32) -> View {
        View {
            eye,
            at: eye + forward(yaw, pitch),
            up: Vec3::Y,
        }
    }

    pub fn matrix(&self) -> Matrix4 {
        Matrix4::look_at_lh(self.eye, self.at, self.up)
    }
}

pub fn forward(yaw: f32, pitch: f32) -> Vec3 {
    let cos_pitch = pitch.cos();

    Vec3::new(yaw.cos() * cos_pitch, pitch.sin(), yaw.sin() * cos_pitch)
}

#[cfg(test)]
mod tests {

    use super::*;

    fn close(actual: Vec3, expected: Vec3) {
        assert!(
            actual.abs_diff_eq(expected, 1e-5),
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn view_places_eye_at_origin_and_target_on_positive_z() {
        let eye = Vec3::new(3.0, -2.0, 7.0);

        let at = Vec3::new(-1.0, 4.0, 12.0);

        let view = Matrix4::look_at_lh(eye, at, Vec3::Y);

        close(view.transform_coord(eye), Vec3::ZERO);

        close(
            view.transform_coord(at),
            Vec3::new(0.0, 0.0, eye.distance(at)),
        );

        let direction = forward(0.7, -0.3);

        assert!((direction.length() - 1.0).abs() < 1e-6);

        let posed = View::from_pose(eye, 0.7, -0.3);

        close(posed.matrix().transform_coord(posed.at), Vec3::Z);
    }

    #[test]
    fn perspective_maps_near_far_and_horizontal_edge_to_clip_bounds() {
        let matrix = Matrix4::perspective_fov_lh(std::f32::consts::FRAC_PI_2, 2.0, 1.0, 11.0);

        close(matrix.transform_coord(Vec3::Z), Vec3::ZERO);

        close(matrix.transform_coord(Vec3::new(0.0, 0.0, 11.0)), Vec3::Z);

        close(
            matrix.transform_coord(Vec3::new(2.0, 1.0, 1.0)),
            Vec3::new(1.0, 1.0, 0.0),
        );
    }

    #[test]
    fn matrix_composition_preserves_row_vector_transform_order() {
        let scale = Matrix4::scaling(2.0, 3.0, 4.0);

        let translation = Matrix4::translation(5.0, 6.0, 7.0);

        let point = Vec3::new(1.0, 2.0, 3.0);

        close(
            Matrix4::multiply(&scale, &translation).transform_coord(point),
            Vec3::new(7.0, 12.0, 19.0),
        );

        close(
            Matrix4::multiply(&translation, &scale).transform_coord(point),
            Vec3::new(12.0, 24.0, 40.0),
        );
    }

    #[test]
    fn rotation_constructors_agree_on_axis_orientation() {
        let angle = std::f32::consts::FRAC_PI_2;

        close(Matrix4::rotation_x(angle).transform_coord(Vec3::Y), Vec3::Z);

        close(Matrix4::rotation_y(angle).transform_coord(Vec3::Z), Vec3::X);

        let (s, c) = (angle * 0.5).sin_cos();

        for matrix in [
            Matrix4::rotation_z(angle),
            Matrix4::rotation_axis(Vec3::Z, angle),
            Matrix4::rotation_quaternion([0.0, 0.0, s, c]),
            Matrix4::rotation_yaw_pitch_roll(0.0, 0.0, angle),
        ] {
            close(matrix.transform_coord(Vec3::X), Vec3::Y);
        }
    }

    #[test]
    fn projection_uses_drawable_area_after_insets() {
        let projection = Projection::new((1024, 768), (224, 168), 300.0);

        assert!((projection.aspect - 4.0 / 3.0).abs() < 1e-6);

        close(
            projection
                .matrix()
                .transform_coord(Vec3::new(0.0, 0.0, projection.near)),
            Vec3::ZERO,
        );

        close(
            projection
                .matrix()
                .transform_coord(Vec3::new(0.0, 0.0, projection.far)),
            Vec3::Z,
        );
    }
}
