use super::lights::{GLOW_MESH_ID, Glow};
use super::objects::{BoundTexture, MeshResource};
use formats::mesh::StaticVertex;
use glam::{Vec2, Vec3};
use render::camera::Matrix4;
use render::light_shafts::LightShaft;

#[cfg(test)]
mod tests;

pub(super) fn enabled(scene: u32, name: &str) -> bool {
    scene == super::world::SCENE_SELECT_CHAR && name == super::world::SELECT_CHAR_ZONE
}

pub(super) fn from_glow(
    resource: &MeshResource,
    textures: &[BoundTexture],
    glow: &Glow,
    now_ms: u32,
) -> Option<Vec<LightShaft>> {
    if glow.mesh_id != GLOW_MESH_ID || resource.mesh.subsets.len() != 1 || !resource.mesh.has_uvs {
        return None;
    }

    let texture = resource.subset_textures.first().copied().flatten()?;

    let bound = textures.get(texture)?;

    if bound.pending || bound.texture.width == 0 || bound.texture.height == 0 {
        return None;
    }

    let indices = &resource.mesh.indices;

    if indices.is_empty() || !indices.len().is_multiple_of(6) {
        return None;
    }

    let tint = glow.tint(now_ms);

    if tint.iter().any(|value| !value.is_finite()) {
        return None;
    }

    let peak = tint[0].max(tint[1]).max(tint[2]);

    if peak <= 0.0 {
        return None;
    }

    let color = [tint[0], tint[1], tint[2]].map(|value| 1.5 + 0.5 * value / peak);

    let transform = glow.world();

    indices
        .chunks_exact(6)
        .map(|quad| {
            let mut unique = quad.to_vec();

            unique.sort_unstable();

            unique.dedup();

            let [a, b, c, d]: [u16; 4] = unique.try_into().ok()?;

            let vertices = [a, b, c, d]
                .map(|index| resource.mesh.vertices.get(usize::from(index)).copied())
                .into_iter()
                .collect::<Option<Vec<_>>>()?;

            let (local, uv_scale_offset, source_width) = volume(vertices.try_into().ok()?)?;

            let world = Matrix4::multiply(&local, &transform);

            if world.0.iter().any(|value| !value.is_finite()) {
                return None;
            }

            let width = Vec3::from_slice(&world.0[..3]);

            let thickness = Vec3::from_slice(&world.0[4..7]);

            let length = Vec3::from_slice(&world.0[8..11]);

            if width.cross(thickness).dot(length).abs() < 0.0001 {
                return None;
            }

            Some(LightShaft {
                world,
                color,
                density: 0.08 * tint[3] / thickness.length(),
                texture,
                uv_scale_offset,
                source_width: [source_width; 2],
            })
        })
        .collect()
}

fn volume(mut vertices: [StaticVertex; 4]) -> Option<(Matrix4, [f32; 4], f32)> {
    if vertices.iter().any(|vertex| {
        !Vec3::from_array(vertex.position).is_finite() || !Vec2::from_array(vertex.uv).is_finite()
    }) {
        return None;
    }

    vertices.sort_by(|a, b| a.uv[1].total_cmp(&b.uv[1]));

    let (lower, upper) = vertices.split_at_mut(2);

    lower.sort_by(|a, b| a.uv[0].total_cmp(&b.uv[0]));

    upper.sort_by(|a, b| a.uv[0].total_cmp(&b.uv[0]));

    let uv_min = Vec2::from_array(lower[0].uv).min(Vec2::from_array(upper[0].uv));

    let uv_max = Vec2::from_array(lower[1].uv).max(Vec2::from_array(upper[1].uv));

    let uv_range = uv_max - uv_min;

    if uv_range.min_element() < 0.001
        || lower
            .iter()
            .any(|vertex| (vertex.uv[1] - uv_min.y) / uv_range.y > 0.1)
        || upper
            .iter()
            .any(|vertex| (vertex.uv[1] - uv_min.y) / uv_range.y < 0.9)
    {
        return None;
    }

    let point = |vertex: &StaticVertex| Vec3::from_array(vertex.position);

    let lower_center = (point(&lower[0]) + point(&lower[1])) * 0.5;

    let upper_center = (point(&upper[0]) + point(&upper[1])) * 0.5;

    let lower_width = (point(&lower[1]) - point(&lower[0])) * 0.5;

    let upper_width = (point(&upper[1]) - point(&upper[0])) * 0.5;

    let lower_length = lower_width.length();

    let upper_length = upper_width.length();

    if lower_length < 0.001 || upper_length < 0.001 || upper_length > lower_length * 1.001 {
        return None;
    }

    let lower_tangent = lower_width / lower_length;

    let upper_tangent = upper_width / upper_length;

    if lower_tangent.dot(upper_tangent) < 0.95 {
        return None;
    }

    let x = (lower_tangent + upper_tangent).try_normalize()? * lower_length;

    let z = (upper_center - lower_center) * 0.5;

    let y = z.cross(x).try_normalize()? * lower_length * 0.25;

    let center = (lower_center + upper_center) * 0.5;

    Some((
        Matrix4([
            x.x, x.y, x.z, 0.0, y.x, y.y, y.z, 0.0, z.x, z.y, z.z, 0.0, center.x, center.y,
            center.z, 1.0,
        ]),
        [uv_range.x, uv_range.y, uv_min.x, uv_min.y],
        (upper_length / lower_length).min(1.0),
    ))
}
