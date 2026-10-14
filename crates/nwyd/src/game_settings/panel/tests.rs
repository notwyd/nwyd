use super::*;
use ui::{Element, draw::Command, material::Material, resource::Resource};

fn panel(language: Language, languages: &[Language]) -> Panel {
    let mut preferences = Preferences::from_config(&Config::default());

    preferences.language = language;

    let mut panel = Panel::for_test(preferences);

    let resource = Resource::default();

    let mut source = ui::Panel {
        right: 255,
        bottom: 265,
        movable: false,
        ..Default::default()
    };

    for (name, kind, x, y, width, height) in [
        ("TabVid", ElementType::Button, 10, 35, 70, 20),
        ("TabAud", ElementType::Button, 85, 35, 70, 20),
        ("TabIfc", ElementType::Button, 160, 35, 70, 20),
        ("resDrop", ElementType::Button, 25, 75, 125, 20),
        ("resItem0", ElementType::Button, 25, 96, 68, 14),
        ("aMusLbl", ElementType::Static, 25, 70, 80, 10),
        ("musFill", ElementType::Image, 30, 94, 198, 4),
        ("musBar", ElementType::Image, 30, 92, 198, 8),
        ("musMark", ElementType::Image, 30, 90, 12, 12),
        ("effects", ElementType::Button, 25, 140, 14, 14),
        ("effectsOn", ElementType::Button, 25, 140, 14, 14),
        ("vEfx1Lbl", ElementType::Static, 46, 140, 185, 10),
        ("iHelmoLbl", ElementType::Static, 46, 82, 185, 10),
    ] {
        let mut element = Element::new(kind, "synthetic", name, &resource);

        source.next_element_id += 1;

        element.id = source.next_element_id;

        element.x = x;

        element.y = y;

        element.width = width;

        element.height = height;

        element.layers =
            vec![Material::widget_seed(); if kind == ElementType::Button { 3 } else { 1 }];

        element.set_display(name);

        source.elements.push(element);
    }

    let mut ui = PanelUi::from_resource(source, resource, &panel.chains, 1.0);

    extend_layout(&mut ui, languages).unwrap();

    panel.placed.ui = ui;

    panel.languages = languages.to_vec();

    panel.tab = 2;

    panel.open(Default::default());

    panel
}

fn element<'a>(panel: &'a Panel, name: &str) -> &'a Element {
    panel
        .placed
        .ui
        .panel
        .elements
        .iter()
        .find(|e| e.name == name)
        .unwrap()
}

fn click(panel: &mut Panel, name: &str) {
    let target = element(panel, name);

    assert!(target.visible, "{name} is hidden");

    let point = Some((
        panel.placed.origin.0 + target.x + target.width / 2,
        panel.placed.origin.1 + target.y + target.height / 2,
    ));

    assert!(panel.button(0, true, point, (800, 600), 1.0));

    assert!(panel.button(0, false, point, (800, 600), 1.0));
}

#[test]
fn language_selection_requests_apply_without_a_restart_notice() {
    let mut panel = panel(
        Language::Portuguese,
        &[Language::Portuguese, Language::English],
    );

    assert!(
        !panel
            .placed
            .ui
            .panel
            .elements
            .iter()
            .any(|element| element.name == "languageNotice")
    );

    click(&mut panel, "languageDrop");

    assert!(panel.languages_open);

    assert!(!element(&panel, "evokBar").visible);

    click(&mut panel, "languageItem1");

    assert_eq!(panel.draft.language, Language::English);

    assert_eq!(panel.applied.language, Language::Portuguese);

    assert!(panel.apply_requested);

    assert!(!panel.languages_open);

    assert!(element(&panel, "evokBar").visible);

    assert_eq!(
        element(&panel, "languageDrop").display,
        language_label(Language::English)
    );

    panel.applied = panel.draft;

    panel.close();

    panel.open(Default::default());

    assert_eq!(element(&panel, "languageLabel").display, "Idioma");
}

#[test]
fn dropdown_only_contains_supplied_languages_and_same_choice_does_not_request_save() {
    let mut panel = panel(
        Language::Portuguese,
        &[Language::Portuguese, Language::Polish],
    );

    assert_eq!(
        panel
            .placed
            .ui
            .panel
            .elements
            .iter()
            .filter(|e| e.name.starts_with("languageItem"))
            .count(),
        2
    );

    click(&mut panel, "languageDrop");

    assert_eq!(element(&panel, "languageItem0").y, 187);

    assert_eq!(element(&panel, "languageItem1").rect()[3], 223);

    assert_eq!(
        element(&panel, "languageItem1").display,
        language_label(Language::Polish)
    );

    click(&mut panel, "languageItem0");

    assert!(!panel.apply_requested);

    assert!(!panel.languages_open);
}

#[test]
fn switching_tabs_and_closing_reset_the_language_dropdown() {
    let mut panel = panel(
        Language::English,
        &[Language::English, Language::Portuguese],
    );

    click(&mut panel, "languageDrop");

    click(&mut panel, "TabAud");

    assert!(!panel.languages_open);

    assert!(element(&panel, "aMusLbl").visible);

    assert!(!element(&panel, "languageDrop").visible);

    assert!(!element(&panel, "languageItem0").visible);

    click(&mut panel, "TabIfc");

    assert!(element(&panel, "languageDrop").visible);

    assert!(!element(&panel, "aMusLbl").visible);

    assert!(!element(&panel, "languageItem0").visible);

    click(&mut panel, "languageDrop");

    panel.close();

    panel.open(Default::default());

    assert!(!panel.languages_open);

    assert!(!element(&panel, "languageItem0").visible);
}

#[test]
fn all_language_choices_fit_above_the_selector_without_overlapping_visible_controls() {
    let languages = [
        Language::Korean,
        Language::English,
        Language::Portuguese,
        Language::TraditionalChinese,
        Language::SimplifiedChinese,
        Language::Spanish,
        Language::Turkish,
        Language::Polish,
    ];

    let mut panel = panel(Language::English, &languages);

    click(&mut panel, "languageDrop");

    let choices: Vec<_> = panel
        .placed
        .ui
        .panel
        .elements
        .iter()
        .filter(|e| e.name.starts_with("languageItem"))
        .collect();

    assert_eq!(choices.len(), 8);

    assert_eq!(choices.first().unwrap().y, 79);

    assert_eq!(choices.last().unwrap().rect()[3], 223);

    for (index, choice) in choices.iter().enumerate() {
        assert!(choice.visible);

        assert!(choice.y >= element(&panel, "TabIfc").rect()[3]);

        assert!(choice.rect()[3] <= element(&panel, "languageLabel").y);

        assert!(choice.rect()[2] <= panel.placed.ui.panel.right);

        if let Some(next) = choices.get(index + 1) {
            assert!(choice.rect()[3] <= next.y);
        }
    }

    assert!(!element(&panel, "iHelmoLbl").visible);

    assert!(!element(&panel, "capeRefineLbl").visible);

    assert!(
        element(&panel, "languageDrop").rect()[3] < panel.placed.ui.panel.close_button_location[1]
    );

    assert!(panel.placed.ui.panel.bottom <= 600);
}

#[test]
fn cloned_language_controls_preserve_materials_and_draw_each_label_once() {
    let mut panel = panel(
        Language::English,
        &[Language::English, Language::Portuguese],
    );

    for (source, cloned) in [
        ("resDrop", "languageDrop"),
        ("resItem0", "languageItem0"),
        ("aMusLbl", "languageLabel"),
    ] {
        assert_eq!(
            element(&panel, source).layers,
            element(&panel, cloned).layers
        );

        assert_ne!(
            element(&panel, source).instance,
            element(&panel, cloned).instance
        );
    }

    click(&mut panel, "languageDrop");

    let draws = panel.placed.ui.panel.draw_list(&Interaction::default());

    for name in [
        "languageLabel",
        "languageDrop",
        "languageItem0",
        "languageItem1",
    ] {
        let index = panel
            .placed
            .ui
            .panel
            .elements
            .iter()
            .position(|e| e.name == name)
            .unwrap();

        let text: Vec<_> = draws
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::Text(text) if text.owner == Owner::Element(index) => Some(text),
                _ => None,
            })
            .collect();

        assert_eq!(text.len(), 1, "{name}");

        assert_eq!(text[0].text, element(&panel, name).display);
    }
}

#[test]
fn custom_labels_use_session_language_and_empty_language_list_disables_selection() {
    for (language, label, evok, cape) in [
        (
            Language::Portuguese,
            "Idioma",
            "Tamanho dos Evoks",
            "Refinação Capa",
        ),
        (
            Language::English,
            "Language",
            "Evok size",
            "Cape refinement",
        ),
        (Language::Korean, "Language", "Evok size", "Cape refinement"),
    ] {
        let panel = panel(language, &[]);

        assert_eq!(element(&panel, "languageLabel").display, label);

        assert!(element(&panel, "evokLabel").display.starts_with(evok));

        assert_eq!(element(&panel, "capeRefineLbl").display, cape);

        assert!(!element(&panel, "languageDrop").enabled);

        assert!(
            !panel
                .placed
                .ui
                .panel
                .elements
                .iter()
                .any(|e| e.name.starts_with("languageItem"))
        );
    }
}

#[test]
fn localized_settings_labels_fit_using_the_bundled_font() {
    struct Measure(render::font::Font);

    impl ui::layout::Measure for Measure {
        fn advance(&mut self, _font: i32, unit: u16) -> Option<i32> {
            Some(self.0.advance(unit))
        }

        fn height(&mut self, _font: i32) -> Option<i32> {
            Some(self.0.size() as i32)
        }
    }

    let face = render::font::Font::face(crate::fonts::FALLBACK, 0).unwrap();

    let mut measure = Measure(render::font::Font::new(12, vec![face]));

    for language in [Language::Portuguese, Language::English] {
        let mut panel = panel(language, &[Language::Portuguese, Language::English]);

        panel.draft.language = if language == Language::Portuguese {
            Language::English
        } else {
            Language::Portuguese
        };

        panel.refresh();

        for name in ["languageLabel", "evokLabel", "capeRefineLbl"] {
            let label = element(&panel, name);

            let width: i32 = label
                .display
                .encode_utf16()
                .map(|unit| measure.0.advance(unit))
                .sum();

            assert!(width <= label.width, "{name}");
        }
    }
}

#[test]
fn applying_language_keeps_settings_open_on_the_selected_tab_and_preserves_other_preferences() {
    struct Directory(std::path::PathBuf);

    impl Drop for Directory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    let directory = Directory(std::env::temp_dir().join(format!(
            "nwyd-panel-language-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        )));

    std::fs::create_dir_all(&directory.0).unwrap();

    let mut game = GameDir::open(&directory.0).unwrap();

    let mut panel = panel(
        Language::Portuguese,
        &[Language::Portuguese, Language::English],
    );

    panel.placed.origin = (43, 61);

    panel.draft.music = 17;

    panel.draft.interface.hide_hud = true;

    let identity = panel.placed.ui.identity();

    for (language, label) in [
        (Language::English, "Language"),
        (Language::Portuguese, "Idioma"),
        (Language::English, "Language"),
    ] {
        game.set_language(language);

        panel.draft.language = language;

        panel.applied = panel.draft;

        panel.apply_language(&game, &ui::Strings::default());

        assert_eq!(element(&panel, "languageLabel").display, label);

        assert_eq!(
            element(&panel, "languageDrop").display,
            language_label(language)
        );

        assert_eq!(panel.language, language);

        assert_eq!(panel.placed.ui.identity(), identity);

        assert_eq!(panel.placed.origin, (43, 61));

        assert_eq!(panel.tab, 2);

        assert!(panel.visible);

        assert_eq!(panel.draft.music, 17);

        assert!(panel.draft.interface.hide_hud);

        panel.close();

        panel.open(Default::default());

        assert_eq!(element(&panel, "languageLabel").display, label);
    }
}
