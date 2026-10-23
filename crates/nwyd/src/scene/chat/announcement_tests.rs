use super::*;
use crate::scene::login_panels::language_tests::{placed, translated};
use formats::data::item_list::{BODY_LEN, ItemList};
use protocol::server::SystemMessage;

fn message(record: usize, kind: u8, argument: &str) -> SystemMessage {
    SystemMessage {
        kind,
        code: (record as i32 - 1000) as i16,
        argument: argument.into(),
        has_argument: true,
    }
}

fn items() -> ItemList {
    let mut items = ItemList::from_plain(vec![0; BODY_LEN], Default::default());
    items.set_name(3, b"SyntheticSword");
    items
}

#[test]
fn visible_item_announcements_preserve_localized_formatting_and_routing() {
    let items = items();
    for (record, template, argument, expected) in [
        (
            1462,
            "%s refinou +%s",
            "Synthetic,15",
            "Synthetic refinou +15",
        ),
        (
            1462,
            "%s refined +%s",
            "Synthetic,15",
            "Synthetic refined +15",
        ),
        (
            1562,
            "%s reforcou %s +%s",
            "3,15,Synthetic",
            "Synthetic reforcou SyntheticSword +15",
        ),
        (
            1562,
            "%s reinforced %s +%s",
            "3,15,Synthetic",
            "Synthetic reinforced SyntheticSword +15",
        ),
        (
            1584,
            "%s recebeu %s no %s",
            "Synthetic,SyntheticSword,SyntheticNpc",
            "Synthetic recebeu SyntheticSword no SyntheticNpc",
        ),
        (
            1584,
            "%s received %s from %s",
            "Synthetic,SyntheticSword,SyntheticNpc",
            "Synthetic received SyntheticSword from SyntheticNpc",
        ),
    ] {
        let strings = translated(&[(record, template)], "");
        for field in [false, true] {
            for kind in [0, 3, 4] {
                let announced = prepare_announcement(
                    &strings,
                    &items,
                    &message(record, kind, argument),
                    field,
                    false,
                );
                assert_eq!(announced.record, record);
                assert_eq!(announced.text, expected);
                assert_eq!(
                    announced.banners,
                    vec![
                        (expected.to_string(), 4000);
                        if record != 1562 && kind == 3 { 2 } else { 1 }
                    ]
                );
                assert_eq!(
                    announced.posted,
                    if field && kind == 4 && record != 1562 {
                        vec![(expected.to_string(), 0xFFFF_FFAA)]
                    } else {
                        vec![]
                    }
                );
                assert_eq!(announced.memo, None);
                assert!(!announced.would_have_exited);
                assert!(!announced.world_changed);
            }
        }
    }
}

#[test]
fn hidden_item_announcements_are_selected_by_record_in_every_scene_and_message_kind() {
    let items = items();
    for record in [1462, 1562, 1584] {
        for template in [
            "%s refinou %s",
            "%s received %s",
            "unrelated localized wording %s",
        ] {
            let strings = translated(&[(record, template)], "");
            for field in [false, true] {
                for kind in [0, 2, 3, 4] {
                    let announced = prepare_announcement(
                        &strings,
                        &items,
                        &message(record, kind, "Synthetic,15,Other"),
                        field,
                        true,
                    );
                    assert_eq!(
                        announced,
                        Announced {
                            record,
                            ..Default::default()
                        }
                    );
                }
            }
        }
    }
}

#[test]
fn item_notice_filter_preserves_personal_results_compositions_and_unrelated_notices() {
    let items = items();
    for record in [
        1076, 1176, 1270, 1294, 1388, 1415, 1445, 1489, 1563, 1591, 1614,
    ] {
        let strings = translated(
            &[(record, "Synthetic refinou item +15 e recebeu item no npc")],
            "",
        );
        for field in [false, true] {
            for kind in [0, 2, 3, 4] {
                let message = message(record, kind, "3,15,Synthetic");
                let visible = prepare_announcement(&strings, &items, &message, field, false);
                let filtered = prepare_announcement(&strings, &items, &message, field, true);
                assert!(!visible.text.is_empty());
                assert!(!visible.banners.is_empty());
                assert_eq!(
                    filtered, visible,
                    "record {record}, kind {kind}, field {field}"
                );
            }
        }
    }
}

#[test]
fn item_notice_filter_prevents_memo_delivery_only_for_selected_records() {
    let items = items();
    let strings = translated(
        &[
            (1584, "!%s received %s from %s"),
            (1614, "!Server maintenance %s"),
        ],
        "",
    );
    let reward = message(1584, 4, "Synthetic,SyntheticSword,SyntheticNpc");
    let maintenance = message(1614, 4, "soon");
    for field in [false, true] {
        let visible = prepare_announcement(&strings, &items, &reward, field, false);
        assert_eq!(
            visible.memo.as_deref(),
            Some("Synthetic received SyntheticSword from SyntheticNpc")
        );
        let hidden = prepare_announcement(&strings, &items, &reward, field, true);
        assert_eq!(
            hidden,
            Announced {
                record: 1584,
                ..Default::default()
            }
        );
        let visible = prepare_announcement(&strings, &items, &maintenance, field, false);
        let filtered = prepare_announcement(&strings, &items, &maintenance, field, true);
        assert_eq!(filtered, visible);
        assert_eq!(filtered.memo.as_deref(), Some("Server maintenance soon"));
    }
}

#[test]
fn item_notice_filter_preserves_restart_and_world_event_reports() {
    let items = items();
    let strings = translated(
        &[
            (1036, "Restart required"),
            (1378, "Synthetic world event ended"),
        ],
        "",
    );
    for field in [false, true] {
        for kind in [0, 3, 4] {
            let restart = message(1036, kind, "Synthetic");
            let announced = prepare_announcement(&strings, &items, &restart, field, true);
            assert!(announced.would_have_exited);
            assert_eq!(
                announced,
                prepare_announcement(&strings, &items, &restart, field, false)
            );
            let event = message(1378, kind, "Synthetic");
            assert!(ends_khepra(&event));
            let announced = prepare_announcement(&strings, &items, &event, field, true);
            assert!(!announced.banners.is_empty());
            assert_eq!(
                announced,
                prepare_announcement(&strings, &items, &event, field, false)
            );
        }
    }
}

fn notice_panels() -> Panels {
    let mut panels = Panels::for_test();
    panels.items = items();
    panels.strings = translated(
        &[
            (1462, "%s refined +%s"),
            (1562, "%s reinforced %s +%s"),
            (1584, "%s received %s from %s"),
        ],
        "",
    );
    panels.screens.push(Screen {
        content: notice::CONTENT,
        class: "synthetic",
        script: notice::SCRIPT,
        placed: placed(&[(notice::MESSAGE, ElementType::Static)]),
        open: false,
        page: 0,
        page_of: None,
        cape_flag: None,
        combine_widgets: Default::default(),
        mix_recipe_owners: [None; 24],
        inventory_widgets: Default::default(),
        cargo_widgets: Default::default(),
        refine_widgets: Default::default(),
        sheet_widgets: Default::default(),
        guild_member_list: None,
        guild_popup_base: None,
    });
    panels
}

#[test]
fn hiding_item_announcements_skips_banner_sound_and_chat_without_touching_pending_output() {
    for (record, argument) in [
        (1462, "Synthetic,15"),
        (1562, "3,15,Synthetic"),
        (1584, "Synthetic,SyntheticSword,SyntheticNpc"),
    ] {
        let mut panels = notice_panels();
        panels.chat.outbox.push(vec![1, 2, 3]);
        panels.wire.push(vec![4, 5, 6]);
        let message = message(record, 4, argument);
        let hidden = announce(
            &mut panels,
            &message,
            1000,
            &crate::clock::Calendar::GOLDEN,
            true,
        );
        assert_eq!(
            hidden,
            Announced {
                record,
                ..Default::default()
            }
        );
        assert!(!panels.notice.showing);
        assert!(panels.notice.queue.is_empty());
        assert!(panels.notice.sounds.is_empty());
        assert!(panels.chat.records.is_empty());
        assert!(panels.memo.is_empty());
        let shown = announce(
            &mut panels,
            &message,
            1000,
            &crate::clock::Calendar::GOLDEN,
            false,
        );
        assert!(!shown.banners.is_empty());
        assert!(panels.notice.showing);
        assert_eq!(panels.notice.deadline_ms, 5000);
        assert_eq!(panels.notice.sounds, [0x21]);
        assert_eq!(panels.chat.records.len(), usize::from(record != 1562));
        let notice_before = panels.notice.clone();
        let chat_before = panels.chat.records.len();
        announce(
            &mut panels,
            &message,
            2000,
            &crate::clock::Calendar::GOLDEN,
            true,
        );
        assert_eq!(panels.notice, notice_before);
        assert_eq!(panels.chat.records.len(), chat_before);
        assert_eq!(panels.chat.outbox, [vec![1, 2, 3]]);
        assert_eq!(panels.wire, [vec![4, 5, 6]]);
    }
}

#[test]
#[ignore = "requires game files via GAME_DIR"]
fn field_item_announcement_filter_preserves_server_clocks_and_other_notices() {
    use crate::scene::{Scene, enter::Entered, field, login_static::SceneTime};
    use protocol::server::{CHAT_CONSOLE, ChatLine};

    let game = assets::GameDir::open(std::env::var_os("GAME_DIR").unwrap()).unwrap();
    let mut packet =
        formats::net::enter::parse(&vec![0; formats::net::enter::MINIMUM_LEN]).unwrap();
    packet.client_id = 37;
    packet.x = 8 * 128 + 64;
    packet.z = 13 * 128 + 64;
    let mut field = field::build(
        &game,
        (800, 600),
        &SceneTime::pinned(2000, crate::clock::Calendar::GOLDEN),
        &Entered::from_packet(&packet, 0, None),
        &field::Chrome::bare(),
    )
    .unwrap();
    let mut panels = notice_panels();
    panels.chat.outbox.push(vec![1, 2, 3]);
    panels.wire.push(vec![4, 5, 6]);
    field.panels = Some(panels);
    let mut scene = Scene::Field(Box::new(field));
    let filter = crate::scene::item_announcements::Filter::load(
        &crate::scene::item_announcements::tests::game(),
    );
    let line = ChatLine {
        opcode: CHAT_CONSOLE,
        text: "Synthetic obtained the item Synthetic Sword at the !!".into(),
        ..Default::default()
    };
    assert!(filter.matches(&line));
    assert!(matches!(
        scene.hear_on_channel(&line, 1, true),
        Some(Heard::Dropped(_))
    ));
    for text in ["!!123456", "!#26120"] {
        let command = ChatLine {
            text: text.into(),
            ..line.clone()
        };
        assert!(!filter.matches(&command));
        scene.hear_on_channel(&command, 1, false);
    }
    let Scene::Field(field) = &scene else {
        unreachable!()
    };
    let clock = field.zone_time.get();
    assert_eq!((clock.hours, clock.minutes, clock.seconds), (12, 34, 56));
    assert_eq!((field.affect_time.year, field.affect_time.day), (26, 120));
    let panels = field.panels.as_ref().unwrap();
    assert_eq!(panels.notice, Default::default());
    assert!(panels.chat.records.is_empty());
    assert_eq!(panels.chat.outbox, [vec![1, 2, 3]]);
    assert_eq!(panels.wire, [vec![4, 5, 6]]);

    let maintenance = ChatLine {
        text: "Server maintenance in five minutes".into(),
        ..line.clone()
    };
    assert!(!filter.matches(&maintenance));
    scene.hear_on_channel(&maintenance, 1, false);
    let Scene::Field(field) = &mut scene else {
        unreachable!()
    };
    let panels = field.panels.as_mut().unwrap();
    assert!(panels.notice.showing);
    assert_eq!(panels.notice.sounds, [33]);
    assert_eq!(panels.chat.records.len(), 1);

    notice::clear(panels);
    scene.hear_on_channel(&line, 1, false);
    let Scene::Field(field) = &scene else {
        unreachable!()
    };
    let panels = field.panels.as_ref().unwrap();
    assert!(panels.notice.showing);
    assert_eq!(panels.notice.sounds, [33, 33]);
    assert_eq!(panels.chat.records.len(), 2);
}
