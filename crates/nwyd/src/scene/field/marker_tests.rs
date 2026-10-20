use super::ClickMarker;
use crate::scene::effect_tree::MeshMaterial;
use crate::scene::quads::Slot;
use glam::Vec3;
use render::billboard::BillboardPass;

fn marker() -> ClickMarker {
    ClickMarker {
        at: Vec3::new(12.0, 3.3, 41.0),
        tile: (12, 41),
        walking: true,
    }
}

fn assert_vector(actual: Vec3, expected: Vec3) {
    assert!(
        actual.abs_diff_eq(expected, 0.00001),
        "{actual:?} != {expected:?}"
    );
}

#[test]
fn destination_meshes_keep_their_vertical_axis_upright_through_the_entire_turn() {
    let marker = marker();

    for now_ms in [0, 125, 250, 375, 500, 625, 750, 875, 999, 1_000, 1_125] {
        let draws = marker.draws(now_ms);

        assert_eq!(draws.meshes.len(), 2);

        for mesh in draws.meshes {
            assert_vector(mesh.world.transform_coord(Vec3::ZERO), marker.at);
            assert_vector(mesh.world.transform_coord(Vec3::Z), marker.at + Vec3::Y);
            assert_vector(mesh.world.transform_coord(Vec3::NEG_Z), marker.at - Vec3::Y);
        }
    }
}

#[test]
fn destination_meshes_rotate_their_sides_around_the_upright_axis_once_per_second() {
    let marker = marker();

    for (now_ms, local_x, local_y) in [
        (0, Vec3::NEG_Z, Vec3::NEG_X),
        (250, Vec3::NEG_X, Vec3::Z),
        (500, Vec3::Z, Vec3::X),
        (750, Vec3::X, Vec3::NEG_Z),
    ] {
        for mesh in marker.draws(now_ms).meshes {
            assert_vector(mesh.world.transform_coord(Vec3::X), marker.at + local_x);
            assert_vector(mesh.world.transform_coord(Vec3::Y), marker.at + local_y);
        }
    }

    assert_eq!(marker.draws(125), marker.draws(1_125));
    assert_eq!(marker.draws(750), marker.draws(1_750));
    assert_ne!(marker.draws(0).meshes, marker.draws(250).meshes);
}

#[test]
fn destination_meshes_keep_their_authored_models_additive_tints_and_textures() {
    for now_ms in [0, 125, 250, 500, 750] {
        let draws = marker().draws(now_ms);

        assert_eq!(draws.meshes.len(), 2);

        for (mesh, id, tint) in [
            (
                &draws.meshes[0],
                316,
                [17.0 / 255.0, 17.0 / 255.0, 136.0 / 255.0, 1.0],
            ),
            (&draws.meshes[1], 317, [1.0, 0.0, 0.0, 1.0]),
        ] {
            assert_eq!(mesh.mesh_id, id);
            assert_eq!(mesh.texture, None);
            assert_eq!(
                mesh.material,
                MeshMaterial::Effect {
                    blend: 1,
                    tint,
                    uv_offset: [0.0; 2],
                }
            );
        }
    }
}

#[test]
fn destination_disc_keeps_its_height_colour_and_pulse_without_spinning() {
    for (now_ms, scale) in [
        (0, 1.0),
        (125, 1.1),
        (250, 1.0),
        (375, 0.9),
        (625, 1.1),
        (875, 0.9),
        (1_000, 1.0),
    ] {
        let draws = marker().draws(now_ms);

        assert_eq!(draws.quads.len(), 1);

        let disc = &draws.quads[0];

        assert_eq!(disc.texture, 232);
        assert_eq!(disc.pass, BillboardPass::Additive);
        assert_eq!(disc.argb, 0xFF99_99AA);
        assert_eq!(disc.slot, Slot::Placed);
        assert_eq!(disc.owner, 0);
        assert_eq!(disc.ordinal, 0);
        assert_eq!(disc.spin_period, 0);
        assert_eq!(disc.grow, 0.0005);
        assert_vector(disc.anchor, Vec3::new(12.0, 3.1, 41.0));
        assert_vector(disc.base_scale, Vec3::splat(scale));
    }
}

#[test]
fn destination_markers_are_shown_only_for_walking_or_running_actions() {
    for action in 0..=32 {
        assert_eq!(ClickMarker::shown(action), action == 2 || action == 3);
    }

    assert!(!ClickMarker::shown(usize::MAX));
}
