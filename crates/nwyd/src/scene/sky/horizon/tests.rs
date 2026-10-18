use super::*;
use formats::mesh::StaticVertex;
use formats::texture::{MipLevel, PixelFormat};

fn wall() -> StaticMesh {
    StaticMesh {
        fvf: 0x112,
        subsets: Vec::new(),
        texture_fields: Vec::new(),
        indices: vec![0, 1, 2, 2, 1, 3],
        vertices: [
            ([-1.0, 0.0, 1.0], [0.0, 1.0]),
            ([-1.0, 10.0, 1.0], [0.0, 0.0]),
            ([1.0, 0.0, 1.0], [1.0, 1.0]),
            ([1.0, 10.0, 1.0], [1.0, 0.0]),
        ]
        .into_iter()
        .map(|(position, uv)| StaticVertex {
            position,
            uv,
            normal: [0.0; 3],
            color: 0xffffffff,
        })
        .collect(),
        has_normals: true,
        has_uvs: true,
        has_colors: false,
    }
}

fn texture(rows: &[[u8; 4]]) -> Texture {
    let data: Vec<u8> = rows
        .iter()
        .flat_map(|row| (0..8).flat_map(move |_| *row))
        .collect();

    Texture {
        width: 8,
        height: rows.len() as u32,
        format: PixelFormat::Rgba8Unorm,
        mips: vec![MipLevel {
            width: 8,
            height: rows.len() as u32,
            offset: 0,
            len: data.len(),
        }],
        source_len: data.len(),
        data,
    }
}

fn rgb(color: Option<u32>) -> [u8; 3] {
    let color = color.expect("sampled sky horizon");

    [(color >> 16) as u8, (color >> 8) as u8, color as u8]
}

fn assert_color(actual: Option<u32>, expected: [u8; 3]) {
    for (actual, expected) in rgb(actual).into_iter().zip(expected) {
        assert!(
            actual.abs_diff(expected) <= 1,
            "sky color {actual} != {expected}"
        );
    }
}

#[test]
fn horizon_uses_mesh_uvs_at_camera_height_and_preserves_missing_texture_fallback() {
    let texture = texture(&[
        [20, 40, 60, 255],
        [40, 80, 120, 255],
        [60, 120, 180, 255],
        [80, 160, 240, 255],
    ]);
    let horizon = Horizon::new(
        &wall(),
        &Matrix4::translation(0.0, 7.0, 0.0),
        [Some(&texture), None, None, None],
    );

    assert_color(horizon.color(Sky::at(0), 0, true, 12.0), [51, 101, 151]);
    assert_color(horizon.color(Sky::at(0), 0, true, 14.5), [31, 61, 91]);
    assert_color(horizon.color(Sky::at(0), 0, true, 9.5), [71, 141, 211]);
    assert_eq!(horizon.color(Sky::at(1), 0, true, 12.0), None);
    assert_eq!(horizon.color(Sky::at(0), 0, true, f32::NAN), None);
}

#[test]
fn horizon_tracks_the_sky_crossfade_and_does_not_fade_without_a_destination() {
    let red = texture(&[[80, 0, 0, 255]; 4]);
    let blue = texture(&[[0, 0, 160, 255]; 4]);
    let horizon = Horizon::new(
        &wall(),
        &Matrix4::IDENTITY,
        [Some(&red), Some(&blue), None, None],
    );
    let mut sky = Sky::at(0);

    sky.set_day(11, 100);

    assert_eq!(rgb(horizon.color(sky, 100, true, 5.0)), [89, 9, 9]);
    assert_eq!(
        rgb(horizon.color(sky, 100 + super::super::FADE_MS / 2, true, 5.0)),
        [44, 4, 84]
    );
    assert_eq!(rgb(horizon.color(sky, 100, false, 5.0)), [81, 1, 1]);

    let missing = Horizon::new(&wall(), &Matrix4::IDENTITY, [Some(&red), None, None, None]);

    assert_eq!(rgb(missing.color(sky, 100, true, 5.0)), [81, 1, 1]);

    sky.update_blend(100 + super::super::FADE_MS);

    assert_eq!(
        rgb(horizon.color(sky, 100 + super::super::FADE_MS, true, 5.0)),
        [1, 1, 161]
    );
}

#[test]
fn invalid_or_untextured_sky_geometry_retains_the_environment_fallback() {
    let texture = texture(&[[80, 0, 0, 255]; 4]);
    let mut mesh = wall();

    mesh.has_uvs = false;

    let horizon = Horizon::new(&mesh, &Matrix4::IDENTITY, [Some(&texture); 4]);

    assert_eq!(horizon.color(Sky::at(0), 0, true, 5.0), None);
}
