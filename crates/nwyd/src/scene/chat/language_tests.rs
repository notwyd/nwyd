use super::*;
use crate::scene::login_panels::language_tests::{placed, translated};

#[test]
fn runtime_language_preserves_chat_draft_history_cooldowns_and_mode() {
    let mut panels = Panels::for_test();

    let mut placed = placed(&[
        ("ChatEditBox", ElementType::ImeEditBox),
        ("BtnMenu", ElementType::Button),
        ("TabBtn1", ElementType::Button),
    ]);

    let edit = placed.ui.panel.elements[0].widget.edit_mut().unwrap();

    edit.text = "/shout Synthetic draft".into();

    edit.caret = 13;

    edit.anchor = 8;

    edit.scroll = 2;

    let draft = edit.clone();

    placed.ui.panel.elements[2].caption_key = "uu_245".into();

    panels.screens.push(Screen {
        content: CONTENT_CHAT,
        class: "synthetic",
        script: CHAT_INPUT,
        placed,
        open: true,
        page: 2,
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

    panels.chat.mode = SHOUT;

    panels.chat.shout_word = "shout".into();

    panels.chat.whisper = "Synthetic target".into();

    panels.chat.history = vec!["Existing sent message".into()];

    panels.chat.history_at = 3;

    panels.chat.shouts = vec!["Synthetic target".into()];

    panels.chat.shouts_at = 2;

    panels.chat.records.push_back(Line {
        text: "Existing message".into(),
        color: 0xFFABCDEF,
        speaker: None,
    });

    panels.chat.sent_at = [123, 122, 121, 120];

    panels.chat.kingdom_at = 119;

    panels.chat.outbox.push(vec![1, 2, 3]);

    panels.chat.focused = true;

    panels.chat.ramp = 0.75;

    panels.chat.size_step = 2;

    panels.chat.bright_step = 1;

    let before = panels.chat.clone();

    let strings = translated(&[(SHOUT_WORD_RECORD, "gritar")], "252 Gritar\n245 Geral\n");

    relocalize(&mut panels, &strings);

    assert_eq!(panels.chat.mode, before.mode);

    assert_eq!(panels.chat.shout_word, "gritar");

    assert_eq!(
        mode_of(&panels.chat, &strings, "/shout Synthetic draft"),
        SHOUT
    );

    assert_eq!(mode_of(&panels.chat, &strings, "/gritar New draft"), SHOUT);

    assert_eq!(panels.chat.whisper, before.whisper);

    assert_eq!(panels.chat.history, before.history);

    assert_eq!(panels.chat.history_at, before.history_at);

    assert_eq!(panels.chat.shouts, before.shouts);

    assert_eq!(panels.chat.shouts_at, before.shouts_at);

    assert_eq!(panels.chat.records, before.records);

    assert_eq!(panels.chat.sent_at, before.sent_at);

    assert_eq!(panels.chat.kingdom_at, before.kingdom_at);

    assert_eq!(panels.chat.outbox, before.outbox);

    assert_eq!(panels.chat.focused, before.focused);

    assert_eq!(panels.chat.ramp, before.ramp);

    assert_eq!(panels.chat.size_step, before.size_step);

    assert_eq!(panels.chat.bright_step, before.bright_step);

    let panel = &panels.screens[0].placed.ui.panel;

    assert_eq!(panel.elements[0].widget.edit().unwrap(), &draft);

    assert_eq!(panel.elements[1].display, "Gritar");

    assert_eq!(panel.elements[2].display, "Geral");
}

#[test]
fn runtime_language_shout_submission_preserves_command_intent() {
    let mut panels = Panels::for_test();

    panels.strings = translated(&[(SHOUT_WORD_RECORD, "shout")], "");

    panels.chat.shout_word = "shout".into();

    let reserved = Reserved::default();

    let before = route_with_move_alpha(
        &mut panels,
        1,
        "Synthetic",
        50,
        1.0,
        &reserved,
        10000,
        "/shout Synthetic draft",
    );

    assert!(matches!(before, Enter::Command(ref word) if word == "shout"));

    assert!(panels.chat.outbox.is_empty());

    let strings = translated(&[(SHOUT_WORD_RECORD, "gritar")], "");

    panels.strings = strings.clone();

    relocalize(&mut panels, &strings);

    let after = route_with_move_alpha(
        &mut panels,
        1,
        "Synthetic",
        50,
        1.0,
        &reserved,
        20000,
        "/shout Synthetic draft",
    );

    assert!(
        matches!(after, Enter::Command(ref word) if word == "gritar"),
        "old shout draft must remain a command, received {after:?}"
    );

    assert!(panels.chat.outbox.is_empty());

    let current = route_with_move_alpha(
        &mut panels,
        1,
        "Synthetic",
        50,
        1.0,
        &reserved,
        30000,
        "/gritar New draft",
    );

    assert!(matches!(current, Enter::Command(ref word) if word == "gritar"));

    assert!(panels.chat.outbox.is_empty());

    let ordinary = route_with_move_alpha(
        &mut panels,
        1,
        "Synthetic",
        50,
        1.0,
        &reserved,
        40000,
        "/Example Synthetic message",
    );

    assert!(matches!(ordinary, Enter::Sent(ref sent) if sent.mode == WHISPER));

    assert_eq!(panels.chat.outbox.len(), 1);
}

#[test]
fn runtime_language_prior_command_drafts_keep_semantic_packets() {
    for (record, old, new) in [
        (61, "answer", "responder"),
        (518, "costumehide", "esconder"),
        (1184, "kingdom", "reino"),
    ] {
        let mut panels = Panels::for_test();

        let before_strings = translated(&[(record, old)], "");

        panels.strings = before_strings.clone();

        bind(&mut panels, &before_strings);

        let text = format!("/{old} Synthetic message");

        let before = route_with_move_alpha(
            &mut panels,
            1,
            "Synthetic",
            50,
            1.0,
            &Reserved::default(),
            10000,
            &text,
        );

        let strings = translated(&[(record, new)], "");

        panels.strings = strings.clone();

        relocalize(&mut panels, &strings);

        let after = route_with_move_alpha(
            &mut panels,
            1,
            "Synthetic",
            50,
            1.0,
            &Reserved::default(),
            20000,
            &text,
        );

        match (before, after) {
            (Enter::Sent(before), Enter::Sent(after)) => {
                assert_eq!(
                    after.packet, before.packet,
                    "record {record} changed command target"
                );

                assert_eq!(after.analytics, before.analytics);

                assert_eq!(after.mode, before.mode);
            }
            (Enter::PendingCommand(before), Enter::PendingCommand(after)) => {
                assert_eq!(after, before)
            }
            (before, after) => {
                panic!("record {record} changed command intent: {before:?} -> {after:?}")
            }
        }
    }
}

#[test]
fn runtime_language_prior_local_command_draft_stays_local() {
    let mut panels = Panels::for_test();

    let before_strings = translated(&[(GUILD_BOARD_RECORD, "guildboard")], "");

    panels.strings = before_strings.clone();

    bind(&mut panels, &before_strings);

    let before = route_with_move_alpha(
        &mut panels,
        1,
        "Synthetic",
        50,
        1.0,
        &Reserved::default(),
        10000,
        "guildboard",
    );

    assert!(matches!(before, Enter::Local(command) if command.what == LocalWhat::GuildBoard));

    let strings = translated(&[(GUILD_BOARD_RECORD, "forum")], "");

    panels.strings = strings.clone();

    relocalize(&mut panels, &strings);

    let after = route_with_move_alpha(
        &mut panels,
        1,
        "Synthetic",
        50,
        1.0,
        &Reserved::default(),
        20000,
        "guildboard",
    );

    assert_eq!(after, before);

    assert!(panels.chat.outbox.is_empty());
}

#[test]
fn runtime_language_default_helm_command_remains_command_after_translation() {
    let mut panels = Panels::for_test();

    bind(&mut panels, &Strings::default());

    let before = route_with_move_alpha(
        &mut panels,
        1,
        "Synthetic",
        50,
        1.0,
        &Reserved::default(),
        10000,
        "/HeLmOhIdE",
    );

    assert!(
        matches!(before, Enter::Sent(ref sent) if sent.mode == NORMAL && sent.analytics.is_none())
    );

    let strings = translated(&[(1951, "capacete")], "");

    panels.strings = strings.clone();

    relocalize(&mut panels, &strings);

    let after = route_with_move_alpha(
        &mut panels,
        1,
        "Synthetic",
        50,
        1.0,
        &Reserved::default(),
        20000,
        "/HeLmOhIdE",
    );

    assert_eq!(after, before);

    assert_eq!(panels.chat.outbox.len(), 2);

    assert_eq!(panels.chat.outbox[0], panels.chat.outbox[1]);
}

#[test]
fn runtime_language_current_commands_precede_historical_alias_collisions() {
    for previous_record in [518, 1951, 1184, 234, 61] {
        for word in [
            "tab",
            "r",
            "re",
            "srv",
            "relo",
            "Relocate",
            "relocate",
            "helmohide",
            "´ä",
            "¤§",
            "responder",
        ] {
            let before_strings = translated(&[(previous_record, word)], "");

            let current_strings = translated(
                &[
                    (518, "esconder"),
                    (1184, "reino"),
                    (234, "mover"),
                    (61, "responder"),
                ],
                "",
            );

            let mut switched = Panels::for_test();

            switched.strings = before_strings.clone();

            bind(&mut switched, &before_strings);

            switched.strings = current_strings.clone();

            relocalize(&mut switched, &current_strings);

            let mut fresh = Panels::for_test();

            fresh.strings = current_strings.clone();

            bind(&mut fresh, &current_strings);

            let text = format!("/{word} Example");

            let expected = route_with_move_alpha(
                &mut fresh,
                1,
                "Synthetic",
                50,
                1.0,
                &Reserved::default(),
                10000,
                &text,
            );

            let actual = route_with_move_alpha(
                &mut switched,
                1,
                "Synthetic",
                50,
                1.0,
                &Reserved::default(),
                10000,
                &text,
            );

            assert_eq!(
                actual, expected,
                "historic record {previous_record} stole current /{word}"
            );

            assert_eq!(switched.chat.tab_request, fresh.chat.tab_request);

            assert_eq!(switched.chat.outbox, fresh.chat.outbox);
        }
    }
}

#[test]
fn runtime_language_current_commands_precede_historical_shout_mode() {
    for word in [
        "tab",
        "r",
        "re",
        "srv",
        "helmohide",
        "responder",
        "esconder",
    ] {
        let old_strings = translated(&[(SHOUT_WORD_RECORD, word)], "");

        let current_strings = translated(
            &[
                (SHOUT_WORD_RECORD, "gritar"),
                (61, "responder"),
                (518, "esconder"),
            ],
            "",
        );

        let mut switched = Panels::for_test();

        switched.strings = old_strings.clone();

        bind(&mut switched, &old_strings);

        switched.strings = current_strings.clone();

        relocalize(&mut switched, &current_strings);

        let mut fresh = Panels::for_test();

        fresh.strings = current_strings.clone();

        bind(&mut fresh, &current_strings);

        let draft = format!("/{word} Example");

        assert_eq!(
            mode_of(&switched.chat, &current_strings, &draft),
            mode_of(&fresh.chat, &current_strings, &draft),
            "historic shout alias stole current /{word} mode"
        );
    }
}

#[test]
fn runtime_language_relocalize_refreshes_draft_mode_and_preserves_empty_selection() {
    for (draft, expected_mode, expected_label) in [
        ("/tab Example", WHISPER, "WhisperNew"),
        ("", SHOUT, "ShoutNew"),
    ] {
        let mut panels = Panels::for_test();

        let old_strings = translated(
            &[(SHOUT_WORD_RECORD, "tab")],
            "246 WhisperOld\n252 ShoutOld\n",
        );

        panels.strings = old_strings.clone();

        panels.screens.push(Screen {
            content: CONTENT_CHAT,
            class: "synthetic",
            script: CHAT_INPUT,
            placed: placed(&[
                ("ChatEditBox", ElementType::ImeEditBox),
                ("BtnMenu", ElementType::Button),
            ]),
            open: true,
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

        bind(&mut panels, &old_strings);

        set_mode(&mut panels, SHOUT, false, &old_strings);

        let edit = panels.screens[0].placed.ui.panel.elements[0]
            .widget
            .edit_mut()
            .unwrap();

        edit.text = draft.into();

        edit.caret = draft.chars().count() as i32;

        edit.anchor = 0;

        let original_edit = edit.clone();

        assert_eq!(panels.chat.mode, SHOUT);

        assert_eq!(
            panels.screens[0].placed.ui.panel.elements[1].display,
            "ShoutOld"
        );

        let current_strings = translated(
            &[(SHOUT_WORD_RECORD, "gritar")],
            "246 WhisperNew\n252 ShoutNew\n",
        );

        panels.strings = current_strings.clone();

        relocalize(&mut panels, &current_strings);

        assert_eq!(
            panels.chat.mode, expected_mode,
            "draft {draft:?} retained stale mode"
        );

        assert_eq!(
            panels.screens[0].placed.ui.panel.elements[1].display,
            expected_label
        );

        assert_eq!(
            panels.screens[0].placed.ui.panel.elements[0]
                .widget
                .edit()
                .unwrap(),
            &original_edit
        );

        assert!(panels.chat.outbox.is_empty());
    }
}
