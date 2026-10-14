use glam::Vec3;
use render::camera::Matrix4;

pub(super) fn load_radius(
    game: &assets::GameDir,
    list: &formats::mesh::mesh_list::MeshList,
) -> Option<f32> {
    let bytes = game.read(list.get(3)?.as_bytes()).ok()?;

    let mesh = formats::mesh::msa::parse(&bytes).ok()?;

    Some(
        mesh.vertices
            .iter()
            .flat_map(|v| v.position)
            .fold(1.0_f32, |r, v| r.max(v.abs())),
    )
}

pub(super) struct Camera {
    pub eye: Vec3,
    pub forward: Vec3,
    pub clip: Matrix4,
    pub viewport: (u32, u32),
    pub fog_end: f32,
}

impl Camera {
    pub fn admits(&self, position: Vec3, scale: f32, radius: Option<f32>) -> bool {
        let Some(radius) = radius else {
            return false;
        };

        let delta = position - self.eye;

        if delta.length() <= radius {
            return true;
        }

        if delta.dot(self.forward) <= 0.0 || delta.x.hypot(delta.z) > self.fog_end + radius {
            return false;
        }

        let extent = radius * scale;

        let points: [Vec3; 9] = std::array::from_fn(|i| {
            if i == 0 {
                return position;
            }

            let corner = i - 1;

            position
                + Vec3::new(
                    if corner & 1 == 0 { -extent } else { extent },
                    if corner & 2 == 0 { -extent } else { extent },
                    if corner & 4 == 0 { -extent } else { extent },
                )
        });

        let mut rect = [i32::MAX, i32::MAX, i32::MIN, i32::MIN];

        let mut centre_projected = false;

        for (i, point) in points.into_iter().enumerate() {
            let projected = self.clip.transform_coord(point);

            if !projected.is_finite() {
                continue;
            }

            let x = ((projected.x + 1.0) * self.viewport.0 as f32 * 0.5) as i32;

            let y = ((1.0 - projected.y) * self.viewport.1 as f32 * 0.5) as i32;

            rect[0] = rect[0].min(x);

            rect[1] = rect[1].min(y);

            rect[2] = rect[2].max(x);

            rect[3] = rect[3].max(y);

            if point.distance(self.eye) < self.fog_end + 3.0 {
                if projected.z >= -0.01
                    && projected.z < 1.0
                    && x > 0
                    && x < self.viewport.0 as i32
                    && y > 0
                    && y < self.viewport.1 as i32
                {
                    return true;
                }

                centre_projected |= i == 0 && projected.z < 1.0;
            }
        }

        centre_projected
            && rect[0].max(0) < rect[2].min(self.viewport.0 as i32)
            && rect[1].max(0) < rect[3].min(self.viewport.1 as i32)
    }
}
