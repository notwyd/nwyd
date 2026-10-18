use super::*;

#[test]
fn extended_views_include_diagonals_and_second_ring_at_zone_edges() {
    let corner = Vec2::splat(17.0 * 128.0 - 1.0);
    let zones = required_zones(corner, 70.0, 140.0, 15.0, (1920, 1080));
    for zone in [
        (16, 16),
        (17, 17),
        (18, 16),
        (16, 18),
        (18, 17),
        (17, 18),
        (15, 15),
    ] {
        assert!(zones.contains(&zone), "missing visible zone {zone:?}");
    }
    assert!(!zones.contains(&(20, 20)));
    assert_eq!(zones.first(), Some(&(16, 16)));
}

#[test]
fn camera_offset_and_wide_frustum_corners_are_covered() {
    let player = Vec2::new(16.0 * 128.0 + 127.0, 16.0 * 128.0 + 64.0);
    let zones = required_zones(player, 90.0, 180.0, 15.0, (3440, 1440));
    let half_y = (render::camera::FOV_FRACTION * render::camera::PI * 0.5).tan();
    let corner_radius =
        180.0 * (1.0 + half_y * half_y * (1.0 + (3440.0f32 / 1440.0).powi(2))).sqrt();
    for angle in [0.0f32, 0.4, 0.9, 1.6, 2.7, 3.6, 4.7, 5.8] {
        let point = player + Vec2::new(angle.cos(), angle.sin()) * (corner_radius + 15.0);
        let zone = ((point.x / 128.0) as u8, (point.y / 128.0) as u8);
        assert!(
            zones.contains(&zone),
            "missing camera/frustum corner {zone:?}"
        );
    }
    assert!(zones.contains(&(19, 16)));
}

#[test]
fn prefetch_is_bounded_to_the_world_and_retail_adds_no_requests() {
    for position in [Vec2::ZERO, Vec2::splat(4095.99), Vec2::new(0.0, 4095.99)] {
        let zones = required_zones(position, 70.0, 140.0, 15.0, (1920, 1080));
        assert!(!zones.is_empty());
        assert!(zones.iter().all(|&(x, z)| x < 32 && z < 32));
        let unique: std::collections::BTreeSet<_> = zones.iter().copied().collect();
        assert_eq!(unique.len(), zones.len());
        assert!(required_zones(position, 70.0, 70.0, 15.0, (1920, 1080)).is_empty());
    }
    assert!(required_zones(Vec2::splat(f32::NAN), 70.0, 140.0, 15.0, (1920, 1080)).is_empty());
    assert!(required_zones(Vec2::splat(4096.0), 70.0, 140.0, 15.0, (1920, 1080)).is_empty());
}

#[test]
fn three_and_four_times_views_add_distinct_outer_rings_and_diagonals() {
    let player = Vec2::splat(17.0 * 128.0 - 1.0);
    let double = required_zones(player, 70.0, 140.0, 18.0, (1920, 1080));
    let triple = required_zones(player, 70.0, 210.0, 18.0, (1920, 1080));
    let maximum = required_zones(player, 70.0, 280.0, 18.0, (1920, 1080));

    assert!(double.iter().all(|zone| triple.contains(zone)));
    assert!(triple.iter().all(|zone| maximum.contains(zone)));
    assert!(!double.contains(&(19, 18)));
    assert!(triple.contains(&(19, 18)));
    assert!(!triple.contains(&(19, 19)));
    assert!(maximum.contains(&(19, 19)));
    assert!(!triple.contains(&(20, 16)));
    assert!(maximum.contains(&(20, 16)));
    assert!(double.len() < triple.len());
    assert!(triple.len() < maximum.len());
}

#[test]
fn large_view_radii_cover_frustum_corners_without_escaping_map_bounds() {
    for retail_far in [70.0, 90.0] {
        for multiplier in [3.0, 4.0, 5.0, 6.0, 7.0, 8.0] {
            for viewport in [(1920, 1080), (3440, 1440), (5120, 1440), (1080, 1920)] {
                for player in [
                    Vec2::ZERO,
                    Vec2::splat(127.99),
                    Vec2::splat(2175.0),
                    Vec2::new(0.0, 4095.99),
                    Vec2::new(4095.99, 0.0),
                    Vec2::splat(4095.99),
                ] {
                    let far = retail_far * multiplier;
                    let zones = required_zones(player, retail_far, far, 18.0, viewport);
                    let unique: std::collections::BTreeSet<_> = zones.iter().copied().collect();
                    assert!(!zones.is_empty());
                    assert_eq!(unique.len(), zones.len());
                    assert!(zones.iter().all(|&(x, z)| x < 32 && z < 32));
                    let half_y = (std::f32::consts::FRAC_PI_4 * 0.5).tan();
                    let half_x = half_y * viewport.0 as f32 / viewport.1 as f32;
                    let radius = Vec2::new(far, far * half_x).length().hypot(far * half_y) + 18.0;

                    for step in 0..64 {
                        let angle = step as f32 * std::f32::consts::TAU / 64.0;
                        let point = (player + Vec2::new(angle.cos(), angle.sin()) * radius)
                            .clamp(Vec2::ZERO, Vec2::splat(4095.99));
                        let zone = ((point.x / 128.0) as u8, (point.y / 128.0) as u8);
                        assert!(
                            zones.contains(&zone),
                            "missing {zone:?} at {multiplier}x from {player:?} in {viewport:?}"
                        );
                    }
                }
            }
        }
    }
}

fn game_with_zones(world: &World, zones: &[(u8, u8)]) -> assets::GameDir {
    use formats::world::object_bin;

    let mut masks = vec![0; object_bin::PAYLOAD_LEN];
    masks.extend_from_slice(&object_bin::checksum(&masks).to_le_bytes());
    let mut files = vec![("object.bin".into(), masks)];
    for &(x, z) in zones {
        let name = format!("Field{x:02}{z:02}");
        let mut bytes = vec![name.len() as u8];
        bytes.extend_from_slice(name.as_bytes());
        bytes.extend_from_slice(&[x, z]);
        for cell in &world.terrain.cells {
            bytes.extend_from_slice(&[7, cell.uv_mode, cell.uv_mode2, cell.pad]);
            bytes.extend_from_slice(&cell.tile0.to_le_bytes());
            bytes.extend_from_slice(&cell.tile1.to_le_bytes());
            bytes.extend_from_slice(&cell.color);
        }
        files.push((format!("Env/{name}.trn"), bytes));
        files.push((format!("Env/{name}.dat"), Vec::new()));
    }
    assets::GameDir::from_files("visual-stream-test", files).unwrap()
}

fn wait_for_visual(
    world: &mut World,
    game: &assets::GameDir,
    mut stage: impl FnMut(u64, &super::super::loading::Prepared) -> bool,
) {
    let start = std::time::Instant::now();
    while world.visual_zone.is_some() {
        world.poll_visual_zone(game, &mut stage).unwrap();
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}

#[test]
fn higher_view_tiers_load_distant_terrain_without_expanding_collision() {
    use crate::scene::view_distance::ViewDistance;

    let mut world = super::super::fog_tests::field_world();
    let outer = [
        (19, 18),
        (19, 19),
        (20, 16),
        (20, 19),
        (21, 18),
        (22, 16),
        (22, 20),
    ];
    let game = game_with_zones(&world, &outer);
    let player = Vec2::splat(2175.0);
    let window = world.actors.window.clone();
    let placement = world.actors.placement_window.clone();
    let anchor = world.actors.zone;
    let current = world.current;

    world.set_view_distance(ViewDistance::Distant);
    assert!(
        !world
            .prefetch_visual_zones(&game, player, 18.0, (1920, 1080), &mut |_, _| true)
            .unwrap()
    );
    assert!(world.visual_zone.is_none());

    for (tier, expected) in [
        (ViewDistance::Far, 1),
        (ViewDistance::Maximum, 3),
        (ViewDistance::FiveTimes, 4),
        (ViewDistance::SixTimes, 5),
        (ViewDistance::SevenTimes, 6),
        (ViewDistance::EightTimes, 7),
    ] {
        world.set_view_distance(tier);
        let start = std::time::Instant::now();
        while world.streamed.len() < expected {
            world.poll_visual_zone(&game, &mut |_, _| true).unwrap();
            world
                .prefetch_visual_zones(&game, player, 18.0, (1920, 1080), &mut |_, _| true)
                .unwrap();
            assert!(start.elapsed() < std::time::Duration::from_secs(5));
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        assert_eq!(world.current, current);
        assert_eq!(world.actors.zone, anchor);
        assert!(world.actors.window == window);
        assert!(world.actors.placement_window == placement);
        assert_eq!(world.force_load, None);
        for (index, &zone) in outer.iter().enumerate() {
            assert_eq!(world.holds(zone), index < expected, "{tier:?}: {zone:?}");
        }
        assert!(
            !world
                .prefetch_visual_zones(&game, player, 18.0, (1920, 1080), &mut |_, _| true)
                .unwrap()
        );
        assert!(world.visual_zone.is_none());
    }
    world.set_view_distance(ViewDistance::Retail);
    for _ in 0..3 {
        assert!(
            !world
                .prefetch_visual_zones(&game, player, 18.0, (1920, 1080), &mut |_, _| true)
                .unwrap()
        );
        assert!(world.visual_zone.is_none());
    }
    assert_eq!(world.streamed.len(), outer.len());
    assert_eq!(world.current, current);
    assert_eq!(world.actors.zone, anchor);
    assert_eq!(world.actors.window, window);
    assert_eq!(world.actors.placement_window, placement);
}

#[test]
fn visual_commit_keeps_collision_and_placement_unchanged_even_at_forced_destination() {
    let mut world = super::super::fog_tests::field_world();
    let zone = (17, 16);
    let game = game_with_zones(&world, &[zone]);
    let window = world.actors.window.clone();
    let placement = world.actors.placement_window.clone();
    let heights = world.actors.heights.clone();
    let anchor = world.actors.zone;
    let current = world.current;
    world.force_load = Some(zone);
    world.visual_zone = Some(zone);
    wait_for_visual(&mut world, &game, |_, _| true);
    assert!(world.holds(zone));
    assert_eq!(world.current, current);
    assert_eq!(world.actors.zone, anchor);
    assert!(world.actors.window == window);
    assert!(world.actors.placement_window == placement);
    assert!(world.actors.heights == heights);
    assert_eq!(world.force_load, Some(zone));
}

#[test]
fn cached_visual_terrain_preserves_repeated_back_and_forth_collision_transitions() {
    use super::super::Direction;

    let mut world = super::super::fog_tests::field_world();
    let game = game_with_zones(&world, &[(15, 16), (17, 16), (16, 17)]);
    let original = world.actors.window.clone();
    for zone in [(15, 16), (17, 16), (16, 17)] {
        world.visual_zone = Some(zone);
        wait_for_visual(&mut world, &game, |_, _| true);
        assert_eq!(world.actors.window, original);
    }
    let mut snapshots = Vec::new();
    for cycle in 0..3 {
        for (index, direction) in [
            Direction::MinusX,
            Direction::PlusX,
            Direction::PlusX,
            Direction::MinusX,
            Direction::PlusZ,
            Direction::MinusZ,
        ]
        .into_iter()
        .enumerate()
        {
            assert!(!world.stream(&game, direction).unwrap());
            assert!(world.rebase(direction));
            assert!(world.current.0 >= world.actors.zone.0);
            assert!(world.current.1 >= world.actors.zone.1);
            assert!(world.current.0 <= world.actors.zone.0 + 1);
            assert!(world.current.1 <= world.actors.zone.1 + 1);
            if cycle == 0 {
                snapshots.push((world.actors.zone, world.actors.window.clone()));
            } else {
                assert_eq!(world.actors.zone, snapshots[index].0);
                assert_eq!(world.actors.window, snapshots[index].1);
            }
        }
    }
    assert_eq!(world.streamed.len(), 3);
}

#[test]
fn prefetched_neighbour_enters_collision_only_when_movement_streaming_activates_it() {
    let mut world = super::super::fog_tests::field_world();
    let game = game_with_zones(&world, &[(17, 16)]);
    let anchor = world.actors.zone;
    let window = world.actors.window.clone();
    world.visual_zone = Some((17, 16));
    wait_for_visual(&mut world, &game, |_, _| true);
    assert!(world.actors.window == window);
    assert!(!world.stream(&game, super::super::Direction::PlusX).unwrap());
    assert_eq!(world.actors.zone, anchor);
    assert!(world.actors.window != window);
    assert!(world.actors.window.contains(&7));
    assert!(world.actors.placement_window == world.actors.window);
}

#[test]
fn prefetched_neighbour_enters_collision_on_a_cached_teleport() {
    let mut world = super::super::fog_tests::field_world();
    let game = game_with_zones(&world, &[(17, 16)]);
    let anchor = world.actors.zone;
    let window = world.actors.window.clone();
    world.visual_zone = Some((17, 16));
    wait_for_visual(&mut world, &game, |_, _| true);
    assert!(world.actors.window == window);
    assert!(
        world
            .reanchor(&game, (17, 16), (17 * 128, 16 * 128), 0)
            .unwrap()
    );
    assert_eq!(world.actors.zone, anchor);
    assert!(world.actors.window != window);
    assert!(world.actors.window.contains(&7));
}

#[test]
fn visual_staging_is_sequential_and_retail_finishes_only_the_existing_request() {
    use crate::scene::view_distance::ViewDistance;

    let mut world = super::super::fog_tests::field_world();
    let game = game_with_zones(&world, &[(17, 16), (17, 17)]);
    let player = Vec2::splat(16.0 * 128.0 + 64.0);
    let window = world.actors.window.clone();
    assert!(
        !world
            .prefetch_visual_zones(&game, player, 18.0, (1920, 1080), &mut |_, _| true)
            .unwrap()
    );
    assert!(world.background_loader.is_none());
    world.set_view_distance(ViewDistance::Distant);
    assert!(
        !world
            .prefetch_visual_zones(&game, player, 18.0, (1920, 1080), &mut |_, _| false)
            .unwrap()
    );
    assert_eq!(world.visual_zone, Some((17, 16)));
    let mut serial = None;
    let start = std::time::Instant::now();
    while serial.is_none() {
        assert!(
            !world
                .poll_visual_zone(&game, &mut |_, prepared| {
                    serial = Some(prepared.serial);
                    false
                })
                .unwrap()
        );
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    for _ in 0..3 {
        assert!(
            !world
                .load_zone_background(&game, (17, 17), &mut |_, _| true)
                .unwrap()
        );
        assert!(
            !world
                .prefetch_visual_zones(&game, player, 18.0, (1920, 1080), &mut |_, _| true)
                .unwrap()
        );
        assert!(
            !world
                .poll_visual_zone(&game, &mut |_, prepared| {
                    assert_eq!(Some(prepared.serial), serial);
                    false
                })
                .unwrap()
        );
        assert!(!world.holds((17, 16)));
        assert!(!world.holds((17, 17)));
        assert!(world.actors.window == window);
    }
    world.set_view_distance(ViewDistance::Retail);
    wait_for_visual(&mut world, &game, |_, prepared| {
        assert_eq!(Some(prepared.serial), serial);
        true
    });
    assert!(world.actors.window == window);
    assert!(world.holds((17, 16)));
    assert!(
        !world
            .prefetch_visual_zones(&game, player, 18.0, (1920, 1080), &mut |_, _| true)
            .unwrap()
    );
    assert!(!world.holds((17, 17)));
    world.set_view_distance(ViewDistance::Distant);
    world
        .prefetch_visual_zones(&game, player, 18.0, (1920, 1080), &mut |_, _| true)
        .unwrap();
    wait_for_visual(&mut world, &game, |_, _| true);
    for _ in 0..3 {
        assert!(
            !world
                .prefetch_visual_zones(&game, player, 18.0, (1920, 1080), &mut |_, _| true)
                .unwrap()
        );
        assert!(world.visual_zone.is_none());
    }
    assert_eq!(world.streamed.len(), 2);
}

#[test]
fn visual_prefetch_reuses_the_collision_request_before_queueing_another() {
    use crate::scene::view_distance::ViewDistance;

    let mut world = super::super::fog_tests::field_world();
    let game = game_with_zones(&world, &[(17, 16), (16, 17)]);
    world.set_view_distance(ViewDistance::Distant);
    assert!(
        !world
            .stream_background(&game, super::super::Direction::PlusZ)
            .unwrap()
    );
    assert!(
        !world
            .prefetch_visual_zones(
                &game,
                Vec2::splat(2112.0),
                18.0,
                (1920, 1080),
                &mut |_, _| false
            )
            .unwrap()
    );
    assert_eq!(world.visual_zone, Some((16, 17)));
    assert!(!world.holds((17, 16)));
    wait_for_visual(&mut world, &game, |_, _| true);
    assert!(
        !world
            .stream_background(&game, super::super::Direction::PlusZ)
            .unwrap()
    );
    world
        .prefetch_visual_zones(
            &game,
            Vec2::splat(2112.0),
            18.0,
            (1920, 1080),
            &mut |_, _| true,
        )
        .unwrap();
    assert_eq!(world.visual_zone, Some((17, 16)));
    wait_for_visual(&mut world, &game, |_, _| true);
    assert_eq!(world.streamed.len(), 2);
}

#[test]
fn synchronous_loading_of_a_visual_request_does_not_stall_future_prefetch() {
    use crate::scene::view_distance::ViewDistance;

    let mut world = super::super::fog_tests::field_world();
    let game = game_with_zones(&world, &[(17, 16), (17, 17)]);
    let player = Vec2::splat(2112.0);
    world.set_view_distance(ViewDistance::Distant);
    world
        .prefetch_visual_zones(&game, player, 18.0, (1920, 1080), &mut |_, _| true)
        .unwrap();
    assert_eq!(world.visual_zone, Some((17, 16)));
    assert!(world.stream(&game, super::super::Direction::PlusX).unwrap());
    wait_for_visual(&mut world, &game, |_, _| {
        panic!("resident terrain must not be staged again")
    });
    assert!(world.background_loader.as_ref().unwrap().is_idle());
    world
        .prefetch_visual_zones(&game, player, 18.0, (1920, 1080), &mut |_, _| true)
        .unwrap();
    assert_eq!(world.visual_zone, Some((17, 17)));
    wait_for_visual(&mut world, &game, |_, _| true);
    assert_eq!(world.streamed.len(), 2);
}

#[test]
fn crossing_cached_terrain_after_another_axis_commits_keeps_the_collision_window_aligned() {
    use super::super::Direction;

    let mut world = super::super::fog_tests::field_world();
    let game = game_with_zones(&world, &[(16, 15), (15, 16)]);
    world.visual_zone = Some((16, 15));
    wait_for_visual(&mut world, &game, |_, _| true);
    assert!(!world.stream(&game, Direction::MinusZ).unwrap());
    assert_eq!(world.actors.zone, (16, 15));
    assert!(world.stream(&game, Direction::MinusX).unwrap());
    assert_eq!(world.actors.zone, (15, 16));
    assert!(world.rebase(Direction::MinusZ));
    assert_eq!(world.current, (16, 15));
    assert_eq!(world.actors.zone, (15, 15));
    assert!(world.actors.window.contains(&7));
    assert!(world.actors.placement_window == world.actors.window);
}

#[test]
fn a_forced_destination_retires_unrelated_visual_staging() {
    use crate::scene::view_distance::ViewDistance;

    let mut world = super::super::fog_tests::field_world();
    let game = game_with_zones(&world, &[(17, 16), (17, 17)]);
    world.set_view_distance(ViewDistance::Distant);
    world
        .prefetch_visual_zones(
            &game,
            Vec2::splat(2112.0),
            18.0,
            (1920, 1080),
            &mut |_, _| false,
        )
        .unwrap();
    let mut staged = false;
    let start = std::time::Instant::now();
    while !staged {
        world
            .poll_visual_zone(&game, &mut |_, _| {
                staged = true;
                false
            })
            .unwrap();
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    world.force_load = Some((17, 17));
    assert!(
        !world
            .poll_visual_zone(&game, &mut |_, _| panic!(
                "obsolete visual job must not delay teleport staging"
            ))
            .unwrap()
    );
    assert!(world.visual_zone.is_none());
    assert!(!world.holds((17, 16)));
    assert!(world.background_loader.as_ref().unwrap().is_idle());
    let start = std::time::Instant::now();
    while !world.holds((17, 17)) {
        world
            .load_zone_background(&game, (17, 17), &mut |_, _| true)
            .unwrap();
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(world.streamed.len(), 1);
}

#[test]
fn extended_prefetch_retires_a_normal_request_already_loaded_synchronously() {
    use super::super::Direction;
    use crate::scene::view_distance::ViewDistance;

    let mut world = super::super::fog_tests::field_world();
    let game = game_with_zones(&world, &[(17, 16), (17, 17)]);
    world.set_view_distance(ViewDistance::Distant);
    assert!(!world.stream_background(&game, Direction::PlusX).unwrap());
    assert!(world.stream(&game, Direction::PlusX).unwrap());
    let start = std::time::Instant::now();
    while !world.holds((17, 17)) {
        world.poll_visual_zone(&game, &mut |_, _| true).unwrap();
        world
            .prefetch_visual_zones(
                &game,
                Vec2::splat(2112.0),
                18.0,
                (1920, 1080),
                &mut |_, prepared| {
                    assert_eq!((prepared.terrain.zone_x, prepared.terrain.zone_y), (17, 17));
                    true
                },
            )
            .unwrap();
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert_eq!(world.streamed.len(), 2);
}

#[test]
fn extended_prefetch_finishes_a_normal_request_after_leaving_the_stream_margin() {
    use super::super::Direction;
    use crate::scene::view_distance::ViewDistance;

    let mut world = super::super::fog_tests::field_world();
    let game = game_with_zones(&world, &[(17, 16)]);
    let window = world.actors.window.clone();
    world.set_view_distance(ViewDistance::Distant);
    assert!(!world.stream_background(&game, Direction::PlusX).unwrap());
    let start = std::time::Instant::now();
    while !world.holds((17, 16)) {
        world.poll_visual_zone(&game, &mut |_, _| true).unwrap();
        world
            .prefetch_visual_zones(
                &game,
                Vec2::splat(2112.0),
                18.0,
                (1920, 1080),
                &mut |_, _| true,
            )
            .unwrap();
        assert!(start.elapsed() < std::time::Duration::from_secs(5));
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    assert!(world.actors.window == window);
    assert!(world.visual_collision_dirty);
}
