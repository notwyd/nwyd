use super::*;

#[test]
fn only_one_movement_report_can_wait_behind_other_packets() {
    let first = Action::walk((10, 10), (12, 10), 2, b"66");
    let next = Action::walk((12, 10), (14, 10), 2, b"66");
    let mut outbox = vec![protocol::client::keepalive()];

    assert!(queue_report(&mut outbox, &first));
    outbox.push(formats::net::attack::Attack::default().to_bytes(0x39D));
    let original = outbox.clone();

    for _ in 0..100 {
        assert!(!queue_report(&mut outbox, &next));
    }

    assert_eq!(outbox, original);
    outbox.clear();
    assert!(queue_report(&mut outbox, &next));
    assert_eq!(outbox, [protocol::client::move_to(&next)]);
}

#[test]
fn cancelling_spatial_packets_preserves_other_queued_frames() {
    let mut chat = vec![0; 24];
    protocol::header::set_opcode(&mut chat, 0x333);
    let mut inventory = vec![0; 24];
    protocol::header::set_opcode(&mut inventory, crate::scene::item_move::OPCODE_MOVE);
    let mut outbox = vec![chat.clone()];

    for opcode in [0x36C, 0x2CB, 0x368, 0x367, 0x39D, 0x39E] {
        let mut body = vec![0; 24];
        protocol::header::set_opcode(&mut body, opcode);
        outbox.push(body);
    }

    outbox.push(inventory.clone());
    discard_spatial_outbox(&mut outbox);

    assert_eq!(outbox, [chat, inventory]);
}

fn live_scene() -> Option<crate::scene::Scene> {
    let game = assets::GameDir::open(std::env::var_os("GAME_DIR")?).unwrap();
    let mut packet =
        formats::net::enter::parse(&vec![0; formats::net::enter::MINIMUM_LEN]).unwrap();
    packet.client_id = 37;
    packet.x = 2118;
    packet.z = 2102;
    packet.mob.equipment.items[0].index = 1;
    packet.mob.status.hp = 100;
    packet.mob.status.max_hp = 100;
    let mut field = super::super::field::build(
        &game,
        (800, 600),
        &crate::scene::login_static::SceneTime::pinned(10_000, crate::clock::Calendar::GOLDEN),
        &crate::scene::enter::Entered::from_packet(&packet, 0, None),
        &super::super::field::Chrome::bare(),
    )
    .unwrap();
    field.world.actors.window.fill(0);
    field.outbox.clear();
    Some(crate::scene::Scene::Field(Box::new(field)))
}

fn field_scene(scene: &mut crate::scene::Scene) -> &mut FieldScene {
    let crate::scene::Scene::Field(field) = scene else {
        panic!("expected field scene");
    };
    field
}

#[test]
fn a_pending_report_does_not_stamp_a_new_destination_and_arrival_reports_it_later() {
    let Some(mut scene) = live_scene() else {
        return;
    };
    let field = field_scene(&mut scene);
    let first = Action::walk((2118, 2102), (2120, 2102), 2, b"66");
    let second = Action::walk((2120, 2102), (2122, 2102), 2, b"66");

    assert!(report(field, &first));
    field.time.now_ms = 12_001;
    assert!(!report(field, &second));
    assert_eq!(field.sent_dest, Some(first.to()));
    assert_eq!(field.attack.from, Some((2120, 2102)));
    assert_eq!(field.cc.last_order_ms, 10_000);

    let actor = &mut field.world.actors.actors[field.player.unwrap()];
    actor.position.x = 2122.5;
    actor.position.z = 2102.5;
    actor.speed = 2.0;
    actor.mover.move_alpha = 0.0;
    field.cc.auto_move = second.to();
    tick(field);
    assert_eq!(field.sent_dest, Some(first.to()));
    assert_eq!(field.outbox.len(), 1);

    field.outbox.clear();
    tick(field);
    assert_eq!(field.sent_dest, Some(second.to()));
    assert_eq!(field.outbox.len(), 1);
    let reported = formats::net::action::parse(&field.outbox[0][protocol::header::LEN..]).unwrap();
    assert_eq!(reported.to(), second.to());
}

#[test]
fn server_corrections_discard_unsent_motion_without_cancelling_local_prediction() {
    let Some(mut scene) = live_scene() else {
        return;
    };
    let first = Action::walk((2118, 2102), (2120, 2102), 2, b"66");
    let corrected = Action::walk((2118, 2102), (2121, 2102), 2, b"666");
    let field = field_scene(&mut scene);
    assert!(report(field, &first));
    let keepalive = protocol::client::keepalive();
    field.outbox.push(keepalive.clone());
    assert!(action::apply_op(field, protocol::server::MOVE, 37, &first).walked);
    assert!(has_pending_report(&field.outbox));

    assert!(
        scene
            .action(protocol::server::MOVE, 37, &corrected)
            .unwrap()
            .walked
    );
    let field = field_scene(&mut scene);
    assert_eq!(field.outbox, [keepalive]);
    assert_eq!(field.sent_dest, Some(corrected.to()));
    assert_eq!(field.cc.auto_move, corrected.to());
    assert_eq!(field.attack.from, Some((2121, 2102)));
}

#[test]
fn warps_and_local_death_discard_unsent_spatial_packets() {
    let Some(mut scene) = live_scene() else {
        return;
    };
    let field = field_scene(&mut scene);
    let first = Action::walk((2118, 2102), (2120, 2102), 2, b"66");
    let warp = Action { kind: 7, ..first };
    let keepalive = protocol::client::keepalive();

    assert!(report(field, &first));
    field.outbox.push(keepalive.clone());
    action::apply_op(field, protocol::server::MOVE, 37, &warp);
    assert_eq!(field.outbox, [keepalive.clone()]);
    assert_eq!(field.sent_dest, Some(warp.to()));

    assert!(report(field, &first));
    let player = field.player.unwrap();
    assert!(super::super::kill::die(field, player, 12_000));
    assert_eq!(field.outbox, [keepalive]);
}

#[test]
fn a_blink_does_not_restart_the_cancelled_walk_while_waiting_for_its_reply() {
    let Some(mut scene) = live_scene() else {
        return;
    };
    let field = field_scene(&mut scene);
    let first = Action::walk((2118, 2102), (2120, 2102), 2, b"66");
    assert!(report(field, &first));
    assert!(action::apply_op(field, protocol::server::MOVE, 37, &first).walked);
    field.cc.auto_move = first.to();
    field.entered.class = 3;
    field.entered.skill_mask[0] = u32::MAX;
    field.entered.status.mp = 1_000_000;
    field.entered.quick[0] = super::super::cast::BLINK_CODE;
    field.entered.quick_selected = 0;
    field.world.force_load = None;
    field.time.now_ms = 12_001;

    let result = super::super::cast::cast(field, (2118.5, 0.0, 2104.5), 12_001, false, None);
    assert!(
        matches!(result, super::super::cast::Cast::Blinked { .. }),
        "{result:?}"
    );
    assert_eq!(field.cc.auto_move, (2118, 2104));
    assert_eq!(field.sent_dest, Some((2118, 2104)));
    assert_eq!(field.outbox.len(), 1);
    assert_eq!(
        protocol::header::opcode_of(&field.outbox[0]),
        Some(protocol::client::ACT)
    );

    field.outbox.clear();
    field.time.now_ms = 13_002;
    field.world.actors.tick(13_002);
    assert_eq!(
        attack::cell_of(&field.world.actors.actors[field.player.unwrap()]),
        first.to(),
    );
    tick(field);
    assert!(field.outbox.is_empty());
}

#[test]
fn a_previous_destination_outside_the_window_does_not_block_a_flat_route() {
    let grid = vec![60; 256 * 256];
    let field = HeightField::window(&grid, (16, 16));

    for (from, to, last) in [
        ((2064, 2064), (2068, 2064), (2047, 2064)),
        ((2288, 2064), (2284, 2064), (2304, 2064)),
        ((2064, 2064), (2064, 2068), (2064, 2047)),
        ((2064, 2288), (2064, 2284), (2064, 2304)),
    ] {
        let route = preview_route(&field, from, to, Some(last)).unwrap();

        assert_eq!(route.reached, to);
        assert_eq!(route.dirs.len(), 4);
    }
}

#[test]
fn the_clamped_previous_height_still_rejects_a_real_cliff() {
    let mut grid = vec![60; 256 * 256];

    for z in 0..256 {
        grid[z * 256] = 0;
    }

    let field = HeightField::window(&grid, (16, 16));

    assert!(preview_route(&field, (2064, 2064), (2068, 2064), Some((2047, 2064))).is_none());
    assert!(preview_route(&field, (2064, 2064), (2068, 2064), Some((2064, 2064))).is_some());
}

#[test]
fn outgoing_routes_keep_the_total_height_limit() {
    let mut grid = vec![0; 256 * 256];

    for z in 0..256 {
        for x in 0..256 {
            grid[z * 256 + x] = ((x as i32 - 40) * 4).clamp(-100, 100) as i8;
        }
    }

    let field = HeightField::window(&grid, (0, 0));
    let route = preview_route(&field, (40, 40), (52, 40), None).unwrap();

    assert_eq!(route.reached, (46, 40));
    assert_eq!(route.dirs, b"666666");
}

#[test]
fn clamped_height_reads_do_not_allow_path_origins_outside_the_window() {
    let grid = vec![60; 256 * 256];
    let field = HeightField::window(&grid, (16, 16));

    for (from, to) in [
        ((2047, 2064), (2064, 2064)),
        ((2304, 2064), (2288, 2064)),
        ((2064, 2047), (2064, 2064)),
        ((2064, 2304), (2064, 2288)),
    ] {
        assert!(preview_route(&field, from, to, None).is_none());
    }
}
