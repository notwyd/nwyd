use super::*;

fn heights(mut height: impl FnMut(i32, i32) -> i8) -> Vec<i8> {
    let mut grid = vec![0; 256 * 256];

    for z in 0..256 {
        for x in 0..256 {
            grid[z * 256 + x] = height(x as i32, z as i32);
        }
    }

    grid
}

#[test]
fn incoming_corrections_accept_slopes_with_large_total_height_changes() {
    let grid = heights(|x, _| ((x - 40) * 4).clamp(-100, 100) as i8);
    let field = HeightField::window(&grid, (0, 0));
    let packet = Action::walk((40, 40), (52, 40), 2, &[]);

    for remote in [false, true] {
        let mut mover = Mover::standing([40.0, 40.0], 0.0);

        assert!(mover.accept(&field, (40, 40), &packet, 12, 1_000, remote));
        assert_eq!(mover.dirs, b"666666666666");
        assert_eq!(mover.last_dest, (52, 40));
        assert_eq!(mover.waypoints[12], [52.5, 40.5]);
        assert_eq!(mover.advance(7_000, 2.0, &field).position, [52.5, 40.5]);
    }
}

#[test]
fn successful_walks_cancel_an_existing_teleport_blend() {
    let grid = heights(|_, _| 0);
    let field = HeightField::window(&grid, (0, 0));

    for dirs in [b"66".as_slice(), b"".as_slice()] {
        let mut mover = Mover::standing([10.0, 10.0], 0.0);
        mover.teleport((20, 10), 1_000);
        assert_eq!(mover.advance(1_500, 2.0, &field).position, [13.0, 10.5]);

        let packet = Action::walk((13, 10), (15, 10), 2, dirs);

        assert!(mover.accept(&field, (13, 10), &packet, 12, 2_000, false));
        assert!(mover.teleport_from.is_none());
        assert_eq!(mover.last_dest, (15, 10));
        assert_eq!(mover.advance(2_250, 2.0, &field).position, [14.0, 10.5]);
    }
}

#[test]
fn incoming_digits_stop_before_a_blocked_step_without_replacing_the_packet_endpoint() {
    let grid = heights(|x, _| if x == 11 { 127 } else { 0 });
    let field = HeightField::window(&grid, (0, 0));
    let packet = Action::walk((10, 10), (12, 10), 2, b"66");

    for remote in [false, true] {
        let mut mover = Mover::standing([10.0, 10.0], 0.0);

        assert!(mover.accept(&field, (10, 10), &packet, 12, 1_000, remote));
        assert_eq!(mover.last_dest, (12, 10));
        assert_eq!(mover.dirs, b"66");
        assert_eq!(mover.count, 3);
        assert_eq!(mover.waypoints, [[10.5, 10.5]; 48]);

        let step = mover.advance(1_750, 2.0, &field);

        assert_eq!(step.position, [10.5, 10.5]);
        assert!(step.arrived);
    }
}

#[test]
fn incoming_digits_allow_an_eight_unit_step_and_stop_before_a_larger_one() {
    let grid = heights(|x, _| match x {
        11 => 8,
        12 => 17,
        _ => 0,
    });
    let field = HeightField::window(&grid, (0, 0));
    let packet = Action::walk((10, 10), (12, 10), 2, b"66");
    let mut mover = Mover::standing([10.0, 10.0], 0.0);

    assert!(mover.accept(&field, (10, 10), &packet, 12, 1_000, false));
    assert_eq!(mover.waypoints[0], [10.5, 10.5]);
    assert!(mover.waypoints[1..].iter().all(|p| *p == [11.5, 10.5]));
    assert_eq!(mover.advance(1_750, 2.0, &field).position, [11.5, 10.5]);
}

#[test]
fn rejected_corrections_keep_the_previous_ring_and_restart_its_clock() {
    let grid = heights(|x, _| if x == 13 { 127 } else { 0 });
    let field = HeightField::window(&grid, (0, 0));
    let initial = Action::walk((10, 10), (12, 10), 2, b"66");
    let rejected = Action::walk((10, 10), (20, 10), 2, &[]);

    for remote in [false, true] {
        let mut mover = Mover::standing([10.0, 10.0], 0.0);

        assert!(mover.accept(&field, (10, 10), &initial, 12, 1_000, remote));
        assert_eq!(mover.advance(1_500, 2.0, &field).position, [11.5, 10.5]);

        let ring = mover.waypoints;
        let count = mover.count;
        let cursor = mover.cursor;

        assert!(!mover.accept(&field, (11, 10), &rejected, 12, 7_000, remote));
        assert_eq!(mover.waypoints, ring);
        assert_eq!(mover.count, count);
        assert_eq!(mover.cursor, cursor);
        assert_eq!(mover.dirs, b"66");
        assert_eq!(mover.last_dest, (12, 10));
        assert_eq!(mover.start_ms, 7_000);
        assert!(mover.pathless);
        assert_eq!(mover.advance(7_000, 2.0, &field).position, [10.5, 10.5]);
    }
}

#[test]
fn rejected_corrections_do_not_cancel_a_teleport_blend() {
    let grid = heights(|x, _| if x == 14 { 127 } else { 0 });
    let field = HeightField::window(&grid, (0, 0));
    let mut mover = Mover::standing([10.0, 10.0], 0.0);

    mover.teleport((20, 10), 1_000);
    mover.advance(1_500, 2.0, &field);

    let packet = Action::walk((13, 10), (20, 10), 2, &[]);

    assert!(!mover.accept(&field, (13, 10), &packet, 12, 2_000, false));
    assert_eq!(mover.teleport_from, Some([10.0, 10.0]));
    assert_eq!(mover.last_dest, (20, 10));
    assert_eq!(mover.start_ms, 2_000);
}

#[test]
fn only_remote_bodies_retry_the_original_destination_after_an_empty_boundary_search() {
    let grid = heights(|_, _| 0);
    let field = HeightField::window(&grid, (0, 0));
    let packet = Action::walk((2, 40), (8, 40), 2, &[]);
    let mut local = Mover::standing([0.0, 40.0], 0.0);
    let mut remote = local.clone();

    assert!(!local.accept(&field, (0, 40), &packet, 12, 1_000, false));
    assert_eq!(local.last_dest, (0, 40));
    assert!(remote.accept(&field, (0, 40), &packet, 12, 1_000, true));
    assert_eq!(remote.waypoints[0], [2.5, 40.5]);
    assert_eq!(remote.waypoints[6], [8.5, 40.5]);
    assert_eq!(remote.last_dest, (8, 40));
}

#[test]
fn only_remote_bodies_retry_from_the_packet_source_after_a_partial_search() {
    let grid = heights(|_, _| 0);
    let field = HeightField::window(&grid, (0, 0));
    let packet = Action::walk((51, 40), (55, 40), 2, b"6666");
    let mut local = Mover::standing([40.0, 40.0], 0.0);
    let mut remote = local.clone();

    assert!(!local.accept(&field, (40, 40), &packet, 12, 1_000, false));
    assert_eq!(local.last_dest, (40, 40));
    assert!(remote.accept(&field, (40, 40), &packet, 12, 1_000, true));
    assert_eq!(remote.waypoints[0], [51.5, 40.5]);
    assert_eq!(remote.waypoints[4], [55.5, 40.5]);
    assert_eq!(remote.last_dest, (55, 40));
}
