use super::Sky;
use formats::mesh::msa::StaticMesh;
use formats::texture::Texture;
use glam::{Vec2, Vec3};
use render::camera::Matrix4;

#[cfg(test)]
mod tests;

const SAMPLES: usize = 33;

#[derive(Debug)]
pub struct Horizon {
    colors: [[Option<[f32; 3]>; SAMPLES]; 4],
    heights: [f32; 2],
    pub current: Option<u32>,
}

impl Horizon {
    pub fn new(mesh: &StaticMesh, world: &Matrix4, textures: [Option<&Texture>; 4]) -> Self {
        let vertices: Vec<_> = mesh
            .vertices
            .iter()
            .map(|v| {
                (
                    world.transform_coord(Vec3::from(v.position)),
                    Vec2::from(v.uv),
                )
            })
            .collect();
        let mut colors = [[None; SAMPLES]; 4];
        let low = vertices.iter().map(|v| v.0.y).fold(f32::INFINITY, f32::min);
        let high = vertices
            .iter()
            .map(|v| v.0.y)
            .fold(f32::NEG_INFINITY, f32::max);

        if mesh.has_uvs && low.is_finite() && high.is_finite() && high > low {
            for sample in 0..SAMPLES {
                let t = (sample as f32 / (SAMPLES - 1) as f32).clamp(0.00001, 0.99999);
                let height = low + (high - low) * t;
                let mut uv_samples = Vec::new();

                for triangle in mesh.indices.chunks_exact(3) {
                    let mut crossings = Vec::with_capacity(2);

                    for edge in 0..3 {
                        let Some(&(a, auv)) = vertices.get(triangle[edge] as usize) else {
                            continue;
                        };
                        let Some(&(b, buv)) = vertices.get(triangle[(edge + 1) % 3] as usize)
                        else {
                            continue;
                        };

                        if (a.y <= height) == (b.y <= height) {
                            continue;
                        }

                        let t = (height - a.y) / (b.y - a.y);

                        crossings.push((a.lerp(b, t), auv.lerp(buv, t)));
                    }

                    if let [a, b] = crossings.as_slice() {
                        let weight = a.0.distance(b.0);

                        for step in 0..8 {
                            uv_samples.push((a.1.lerp(b.1, (step as f32 + 0.5) / 8.0), weight));
                        }
                    }
                }

                for (row, texture) in textures.iter().enumerate() {
                    let Some(texture) = texture else { continue };
                    let mut total = Vec3::ZERO;
                    let mut weight = 0.0;

                    for &(uv, length) in &uv_samples {
                        if let Some(rgb) = sample_uv(texture, uv) {
                            total += rgb * length;
                            weight += length;
                        }
                    }

                    if weight > 0.0 {
                        colors[row][sample] = Some((total / weight).to_array());
                    }
                }
            }
        }

        Self {
            colors,
            heights: [low, high],
            current: None,
        }
    }

    fn at_height(&self, row: usize, height: f32) -> Option<Vec3> {
        if !height.is_finite() {
            return None;
        }

        let position = ((height - self.heights[0]) / (self.heights[1] - self.heights[0]))
            .clamp(0.0, 1.0)
            * (SAMPLES - 1) as f32;
        let index = position as usize;
        let colors = self.colors.get(row)?;
        let a = Vec3::from(colors[index]?);
        let b = Vec3::from(colors[(index + 1).min(SAMPLES - 1)]?);

        Some(a.lerp(b, position.fract()))
    }

    pub fn color(&self, sky: Sky, now_ms: u32, day_tail: bool, height: f32) -> Option<u32> {
        let base = (self.at_height(sky.from, height)? + Vec3::splat(0.5 / 255.0)).min(Vec3::ONE);
        let destination = (day_tail && sky.fading(now_ms))
            .then(|| self.at_height(sky.to, height))
            .flatten();
        let color = if let Some(destination) = destination {
            let t = now_ms.wrapping_sub(sky.stamp) as f32 / super::FADE_MS as f32;
            let t = (t.clamp(0.0, 1.0) * 255.0).trunc() / 255.0;

            (base + Vec3::splat(8.0 / 255.0))
                .min(Vec3::ONE)
                .lerp(destination, t)
        } else {
            base
        };
        let [r, g, b] = color
            .to_array()
            .map(|c| (c.clamp(0.0, 1.0) * 255.0).round() as u32);

        Some((r << 16) | (g << 8) | b)
    }

    pub fn update(&mut self, sky: Sky, now_ms: u32, day_tail: bool, height: f32) {
        self.current = self.color(sky, now_ms, day_tail, height);
    }
}

fn sample_uv(texture: &Texture, uv: Vec2) -> Option<Vec3> {
    if texture.width == 0 || texture.height == 0 || !uv.is_finite() {
        return None;
    }

    let point = uv * Vec2::new(texture.width as f32, texture.height as f32) - Vec2::splat(0.5);
    let origin = point.floor();
    let fraction = point - origin;
    let pixel = |dx: i64, dy: i64| {
        let x = (origin.x as i64 + dx).rem_euclid(i64::from(texture.width)) as u32;
        let y = (origin.y as i64 + dy).rem_euclid(i64::from(texture.height)) as u32;
        let [r, g, b, _] = texture.sample_rgba8(x, y)?;

        Some(Vec3::new(r as f32, g as f32, b as f32) / 255.0)
    };

    Some(
        pixel(0, 0)?
            .lerp(pixel(1, 0)?, fraction.x)
            .lerp(pixel(0, 1)?.lerp(pixel(1, 1)?, fraction.x), fraction.y),
    )
}
