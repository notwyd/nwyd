use super::*;

#[test]
fn characterization_legacy_prompt_cp1252_split_boundary_and_explicit_lines() {
    let mut dialogs = Dialogs::for_test();

    let prefix = "é".repeat(33);

    for (count, expected) in [
        (34, ["é".repeat(34), String::new(), String::new()]),
        (35, [prefix.clone(), "éé".into(), String::new()]),
    ] {
        dialogs.set_text(Owner::Question, &"é".repeat(count), 42, None);

        assert_eq!(dialogs.states[0].lines, expected);

        assert_eq!(dialogs.states[0].context, 42);
    }

    dialogs.set_text(
        Owner::Question,
        &format!("{prefix}éé"),
        7,
        Some("Explicit second"),
    );

    assert_eq!(
        dialogs.states[0].lines,
        [
            format!("{prefix}éé"),
            "Explicit second".into(),
            String::new()
        ]
    );

    dialogs.set_text(Owner::Question, "Before\0After", 8, None);

    assert_eq!(
        dialogs.states[0].lines,
        ["Before".into(), String::new(), String::new()]
    );
}

#[test]
fn runtime_language_legacy_prompt_wraps_unicode_without_replacing_characters() {
    let mut dialogs = Dialogs::for_test();

    for glyph in ["설", "設", "ğ", "🦀"] {
        let prompt = glyph.repeat(36);

        dialogs.set_text(Owner::Question, &prompt, 23, None);

        assert_eq!(dialogs.states[0].lines[0], glyph.repeat(33));

        assert_eq!(dialogs.states[0].lines[1], glyph.repeat(3));

        assert_eq!(dialogs.states[0].lines.concat(), prompt);

        assert_eq!(dialogs.states[0].context, 23);
    }
}
