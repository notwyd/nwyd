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
        ("effects", ElementType::Button, 25, 140, 16, 16),
        ("effectsOn", ElementType::Button, 25, 140, 16, 16),
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

fn video_panel(language: Language, renderers: &[RendererBackend]) -> Panel {
    let mut panel = panel(language, &[Language::Portuguese, Language::English]);

    panel.set_renderers(renderers, RendererBackend::Vulkan);

    click(&mut panel, "TabVid");

    panel
}

struct SettingsFile(std::path::PathBuf);

impl SettingsFile {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(0);

        let directory = std::env::temp_dir().join(format!(
            "nwyd-renderer-settings-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        std::fs::create_dir(&directory).unwrap();

        Self(directory)
    }

    fn path(&self) -> std::path::PathBuf {
        self.0.join("settings.json")
    }
}

impl Drop for SettingsFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn view_distance_choices_apply_save_and_cancel_without_changing_other_settings() {
    use crate::scene::view_distance::ViewDistance;

    let files = SettingsFile::new();
    let mut panel = video_panel(Language::English, &[RendererBackend::Vulkan]);
    panel.path = files.path();
    let initial = panel.draft;

    assert_eq!(initial.view_distance, ViewDistance::Retail);

    for (index, choice) in ViewDistance::ALL.into_iter().enumerate() {
        panel.apply_requested = false;
        click(&mut panel, "viewDistanceDrop");
        click(&mut panel, &format!("viewDistanceItem{index}"));
        assert_eq!(
            panel.draft,
            Preferences {
                view_distance: choice,
                ..initial
            }
        );
        assert_eq!(panel.apply_requested, panel.draft != panel.applied);
        assert!(!element(&panel, "viewDistanceItem0").visible);
        assert!(!element(&panel, "rendererNotice").visible);
        panel.save_draft().unwrap();
        assert_eq!(Preferences::load(&panel.path).unwrap(), Some(panel.draft));

        panel.close();
        panel.open(Default::default());
        assert_eq!(panel.draft.view_distance, choice);
        assert!(!element(&panel, "viewDistanceItem0").visible);

        click(&mut panel, "viewDistanceDrop");
        click(
            &mut panel,
            &format!("viewDistanceItem{}", (index + 1) % ViewDistance::ALL.len()),
        );
        panel.discard_draft();
        assert_eq!(panel.draft.view_distance, choice);
        assert_eq!(Preferences::load(&panel.path).unwrap(), Some(panel.draft));
    }

    click(&mut panel, "viewDistanceDrop");
    click(&mut panel, "viewDistanceItem0");
    panel.discard_draft();
    assert_eq!(panel.draft.view_distance, ViewDistance::EightTimes);
    assert_eq!(element(&panel, "viewDistanceDrop").display, "8×");

    panel.path = files.0.join("missing/settings.json");
    click(&mut panel, "viewDistanceDrop");
    click(&mut panel, "viewDistanceItem1");
    assert!(panel.save_draft().is_err());
    assert_eq!(panel.draft, panel.applied);
    assert_eq!(panel.draft.view_distance, ViewDistance::EightTimes);
    panel.close();
    panel.open(Default::default());
    assert_eq!(panel.draft.view_distance, ViewDistance::EightTimes);
}

#[test]
fn view_distance_dropdown_localizes_fits_and_owns_its_video_clicks() {
    let mut panel = video_panel(
        Language::English,
        &[RendererBackend::Vulkan, RendererBackend::Gl],
    );
    let mut game = GameDir::from_files("synthetic-view-distance-language", []).unwrap();

    panel.resize((800, 600));
    assert!(panel.placed.origin.0 >= 0 && panel.placed.origin.1 >= 0);
    assert!(panel.placed.origin.0 + panel.placed.ui.panel.width() <= 800);
    assert!(panel.placed.origin.1 + panel.placed.ui.panel.height() <= 600);

    for neural in [false, true] {
        panel.set_neural_rendering_available(neural);

        for (language, label, captions) in [
            (
                Language::Portuguese,
                "Distância de visão",
                [
                    "Original (1×)",
                    "Ampliada (1,5×)",
                    "Distante (2×)",
                    "Muito distante (3×)",
                    "4×",
                    "5×",
                    "6×",
                    "7×",
                    "8×",
                ],
            ),
            (
                Language::English,
                "View distance",
                [
                    "Retail (1×)",
                    "Extended (1.5×)",
                    "Distant (2×)",
                    "Far (3×)",
                    "4×",
                    "5×",
                    "6×",
                    "7×",
                    "8×",
                ],
            ),
        ] {
            game.set_language(language);
            panel.apply_language(&game, &ui::Strings::default());
            assert_eq!(element(&panel, "viewDistanceLabel").display, label);
            assert_eq!(element(&panel, "viewDistanceDrop").display, captions[0]);
            assert!(
                element(&panel, "rendererNotice").rect()[3]
                    <= element(&panel, "viewDistanceLabel").y
            );
            assert!(
                element(&panel, "viewDistanceLabel").rect()[3]
                    <= element(&panel, "viewDistanceDrop").y
            );
            assert!(
                element(&panel, "viewDistanceDrop").rect()[3]
                    <= panel.placed.ui.panel.close_button_location[1]
            );
            assert!(panel.placed.ui.panel.height() <= 600);

            click(&mut panel, "viewDistanceDrop");
            for element in &panel.placed.ui.panel.elements {
                let name = element.name.as_str();
                if !matches!(
                    name,
                    "" | "TabVid" | "TabAud" | "TabIfc" | "viewDistanceDrop"
                ) && !name.starts_with("viewDistanceItem")
                {
                    assert!(!element.visible, "{name} must hide beneath choices");
                }
            }
            for (index, caption) in captions.into_iter().enumerate() {
                let choice = element(&panel, &format!("viewDistanceItem{index}"));
                assert!(choice.visible);
                assert_eq!(choice.display, caption);
                assert!(choice.x >= panel.placed.ui.panel.left);
                assert!(choice.rect()[2] <= panel.placed.ui.panel.right);
                assert!(choice.y >= element(&panel, "TabVid").rect()[3]);
                assert!(choice.rect()[3] <= element(&panel, "viewDistanceDrop").y);
                if index > 0 {
                    assert!(
                        element(&panel, &format!("viewDistanceItem{}", index - 1)).rect()[3]
                            <= choice.y
                    );
                }
            }
            let before = panel.draft;
            let choice = element(&panel, "viewDistanceItem0");
            let point = Some((
                panel.placed.origin.0 + choice.x + 4,
                panel.placed.origin.1 + choice.y + 4,
            ));
            assert!(panel.button(0, true, point, (800, 600), 1.0));
            assert!(panel.button(0, false, point, (800, 600), 1.0));
            assert_eq!(panel.draft, before);
            assert!(!panel.view_distance_open);
            assert!(!panel.apply_requested);
        }
    }

    for name in ["resDrop", "rendererDrop", "TabAud", "TabIfc"] {
        click(&mut panel, name);
        assert!(!element(&panel, "viewDistanceDrop").visible);
        click(&mut panel, "TabVid");
        click(&mut panel, "viewDistanceDrop");
        click(&mut panel, "TabAud");
        assert!(!element(&panel, "viewDistanceItem0").visible);
        click(&mut panel, "TabVid");
        assert!(element(&panel, "viewDistanceDrop").visible);
        assert!(!element(&panel, "viewDistanceItem0").visible);
    }
}

#[test]
fn view_distance_selection_survives_language_changes_and_escape_releases_popup_input() {
    use crate::scene::view_distance::ViewDistance;

    let mut panel = video_panel(Language::English, &[RendererBackend::Vulkan]);
    let mut game = GameDir::from_files("synthetic-selected-view-distance-language", []).unwrap();

    for (index, (distance, captions)) in ViewDistance::ALL
        .into_iter()
        .zip([
            ["Original (1×)", "Retail (1×)"],
            ["Ampliada (1,5×)", "Extended (1.5×)"],
            ["Distante (2×)", "Distant (2×)"],
            ["Muito distante (3×)", "Far (3×)"],
            ["4×", "4×"],
            ["5×", "5×"],
            ["6×", "6×"],
            ["7×", "7×"],
            ["8×", "8×"],
        ])
        .enumerate()
    {
        click(&mut panel, "viewDistanceDrop");
        click(&mut panel, &format!("viewDistanceItem{index}"));
        panel.applied = panel.draft;
        panel.apply_requested = false;

        for (language, caption) in [Language::Portuguese, Language::English]
            .into_iter()
            .zip(captions)
        {
            game.set_language(language);
            panel.apply_language(&game, &ui::Strings::default());
            assert_eq!(panel.draft.view_distance, distance);
            assert_eq!(element(&panel, "viewDistanceDrop").display, caption);
            assert!(!panel.apply_requested);
        }
    }

    click(&mut panel, "viewDistanceDrop");
    let item = element(&panel, "viewDistanceItem2");
    let point = Some((
        panel.placed.origin.0 + item.x + 4,
        panel.placed.origin.1 + item.y + 4,
    ));
    assert!(panel.button(0, true, point, (800, 600), 1.0));
    let escape = winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape);
    assert!(panel.key(&escape, true));
    assert!(panel.key(&escape, false));
    assert!(!panel.visible);
    assert!(!panel.view_distance_open);
    assert!(panel.button(0, false, point, (800, 600), 1.0));
    assert_eq!(panel.draft.view_distance, ViewDistance::EightTimes);
    panel.open(Default::default());
    assert!(!element(&panel, "viewDistanceItem2").visible);
    assert_eq!(element(&panel, "viewDistanceDrop").display, "8×");
}

#[test]
fn neural_rendering_requires_runtime_support_and_preserves_saved_opt_in_when_unavailable() {
    let files = SettingsFile::new();
    let mut panel = video_panel(Language::English, &[RendererBackend::Vulkan]);
    panel.path = files.path();

    assert!(!panel.draft.neural_rendering);
    for name in [
        "neuralRendering",
        "neuralRenderingOn",
        "vNeuralRenderingLbl",
    ] {
        assert!(!element(&panel, name).visible);
    }
    assert_eq!(element(&panel, "rendererLabel").y, 198);

    panel.set_neural_rendering_available(true);
    assert!(element(&panel, "neuralRendering").visible);
    assert!(!element(&panel, "neuralRenderingOn").visible);
    assert!(!panel.apply_requested);

    click(&mut panel, "neuralRendering");
    panel.save_draft().unwrap();
    let saved = std::fs::read(&panel.path).unwrap();
    panel.apply_requested = false;
    panel.set_neural_rendering_available(false);

    for name in [
        "neuralRendering",
        "neuralRenderingOn",
        "vNeuralRenderingLbl",
    ] {
        assert!(!element(&panel, name).visible);
    }
    assert!(panel.draft.neural_rendering);
    assert!(panel.applied.neural_rendering);
    assert!(!panel.apply_requested);
    assert_eq!(element(&panel, "rendererLabel").y, 198);
    assert_eq!(std::fs::read(&panel.path).unwrap(), saved);

    panel.set_neural_rendering_available(true);
    assert!(element(&panel, "neuralRenderingOn").visible);
    assert!(!element(&panel, "neuralRendering").visible);
}

#[test]
fn neural_rendering_toggles_and_persists_without_changing_other_preferences() {
    let files = SettingsFile::new();
    let mut panel = video_panel(Language::English, &[RendererBackend::Vulkan]);
    panel.path = files.path();
    panel.set_neural_rendering_available(true);
    let initial = panel.draft;

    click(&mut panel, "neuralRendering");

    assert_eq!(
        panel.draft,
        Preferences {
            neural_rendering: true,
            ..initial
        }
    );
    assert_eq!(panel.applied, initial);
    assert!(panel.apply_requested);
    assert!(element(&panel, "neuralRenderingOn").visible);
    assert!(!element(&panel, "neuralRendering").visible);
    assert!(!element(&panel, "rendererNotice").visible);

    panel.save_draft().unwrap();
    assert_eq!(Preferences::load(&panel.path).unwrap(), Some(panel.draft));
    panel.close();
    panel.open(Default::default());
    assert!(panel.draft.neural_rendering);

    click(&mut panel, "neuralRenderingOn");
    panel.save_draft().unwrap();
    assert_eq!(panel.applied, initial);
    assert_eq!(Preferences::load(&panel.path).unwrap(), Some(initial));
}

#[test]
fn neural_rendering_failed_save_and_discard_restore_the_applied_state() {
    let files = SettingsFile::new();
    let mut panel = video_panel(Language::English, &[RendererBackend::Vulkan]);
    panel.path = files.0.join("missing/settings.json");
    panel.set_neural_rendering_available(true);

    for save in [false, true] {
        click(&mut panel, "neuralRendering");
        assert!(panel.draft.neural_rendering);

        if save {
            assert!(panel.save_draft().is_err());
        } else {
            panel.discard_draft();
        }

        assert!(!panel.draft.neural_rendering);
        assert!(!panel.applied.neural_rendering);
        assert!(element(&panel, "neuralRendering").visible);
        assert!(!element(&panel, "neuralRenderingOn").visible);
    }
}

#[test]
fn neural_rendering_controls_hide_beneath_dropdowns_and_on_other_tabs() {
    let mut panel = video_panel(
        Language::English,
        &[RendererBackend::Vulkan, RendererBackend::Gl],
    );
    panel.set_neural_rendering_available(true);
    let checkbox = element(&panel, "neuralRendering");
    let point = Some((checkbox.x + 5, checkbox.y + 5));

    for control in ["rendererDrop", "resDrop", "TabAud", "TabIfc"] {
        click(&mut panel, control);

        for name in [
            "neuralRendering",
            "neuralRenderingOn",
            "vNeuralRenderingLbl",
        ] {
            assert!(!element(&panel, name).visible, "{control}: {name}");
        }

        assert!(panel.button(0, true, point, (800, 600), 1.0));
        assert!(panel.button(0, false, point, (800, 600), 1.0));
        assert!(!panel.draft.neural_rendering);

        click(&mut panel, "TabVid");
        assert!(element(&panel, "neuralRendering").visible);
        assert!(element(&panel, "vNeuralRenderingLbl").visible);
    }
}

#[test]
fn neural_rendering_layout_keeps_controls_and_renderer_choices_separate() {
    let mut panel = video_panel(
        Language::English,
        &[RendererBackend::Vulkan, RendererBackend::Gl],
    );
    let mut game = GameDir::from_files("synthetic-neural-rendering-language", []).unwrap();
    panel.set_neural_rendering_available(true);

    for language in [Language::Portuguese, Language::English] {
        game.set_language(language);
        panel.apply_language(&game, &ui::Strings::default());
        assert_eq!(
            element(&panel, "vNeuralRenderingLbl").display,
            "DLSS 5 (experimental)"
        );

        for name in [
            "neuralRendering",
            "neuralRenderingOn",
            "vNeuralRenderingLbl",
        ] {
            let control = element(&panel, name);
            assert!(control.y >= element(&panel, "oldSkillVfx").rect()[3]);
            assert!(control.y >= element(&panel, "vOldSkillVfxLbl").rect()[3]);
            assert!(control.rect()[3] <= element(&panel, "rendererLabel").y);
            assert!(control.rect()[2] <= panel.placed.ui.panel.right);
            assert!(control.x >= panel.placed.ui.panel.left);
        }
        assert!(element(&panel, "rendererLabel").rect()[3] <= element(&panel, "rendererDrop").y);
        assert!(element(&panel, "rendererDrop").rect()[3] <= element(&panel, "rendererNotice").y);
        assert!(
            element(&panel, "rendererNotice").rect()[3]
                <= panel.placed.ui.panel.close_button_location[1]
        );

        let index = panel
            .placed
            .ui
            .panel
            .elements
            .iter()
            .position(|element| element.name == "vNeuralRenderingLbl")
            .unwrap();
        let draws = panel.placed.ui.panel.draw_list(&Interaction::default());
        let labels: Vec<_> = draws
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::Text(text) if text.owner == Owner::Element(index) => {
                    Some(text.text.as_str())
                }
                _ => None,
            })
            .collect();
        assert_eq!(labels, ["DLSS 5 (experimental)"]);
    }

    click(&mut panel, "rendererDrop");
    for choice in panel
        .placed
        .ui
        .panel
        .elements
        .iter()
        .filter(|element| element.name.starts_with("rendererItem"))
    {
        assert!(choice.visible);
        assert!(choice.rect()[3] <= element(&panel, "rendererDrop").y);
    }
    panel.set_neural_rendering_available(false);
    assert!(!panel.renderers_open);
    assert!(!panel.draft.neural_rendering);
    assert!(!panel.apply_requested);
    assert_eq!(element(&panel, "rendererDrop").y, 216);
}

#[test]
fn old_skill_effects_default_to_enabled_and_toggle_without_changing_other_video_settings() {
    let files = SettingsFile::new();

    let mut panel = video_panel(Language::English, &[RendererBackend::Vulkan]);

    panel.path = files.path();

    let initial = panel.draft;

    assert!(initial.old_skill_vfx);

    assert!(element(&panel, "oldSkillVfxOn").visible);

    assert!(!element(&panel, "oldSkillVfx").visible);

    click(&mut panel, "oldSkillVfxOn");

    assert_eq!(
        panel.draft,
        Preferences {
            old_skill_vfx: false,
            ..initial
        }
    );

    assert_eq!(panel.applied, initial);

    assert!(panel.apply_requested);

    assert!(!element(&panel, "oldSkillVfxOn").visible);

    assert!(element(&panel, "oldSkillVfx").visible);

    assert!(!element(&panel, "rendererNotice").visible);

    panel.save_draft().unwrap();

    assert!(
        !Preferences::load(&files.path())
            .unwrap()
            .unwrap()
            .old_skill_vfx
    );

    panel.close();

    panel.open(EffectOptions {
        old_skill_vfx: false,
        ..Default::default()
    });

    assert!(!panel.draft.old_skill_vfx);

    assert!(element(&panel, "oldSkillVfx").visible);

    click(&mut panel, "oldSkillVfx");

    panel.save_draft().unwrap();

    assert!(
        Preferences::load(&files.path())
            .unwrap()
            .unwrap()
            .old_skill_vfx
    );
}

#[test]
fn old_skill_effects_failed_save_and_discard_restore_the_applied_state() {
    let files = SettingsFile::new();

    let mut panel = video_panel(Language::English, &[RendererBackend::Vulkan]);

    panel.path = files.0.join("missing/settings.json");

    for save in [false, true] {
        click(&mut panel, "oldSkillVfxOn");

        assert!(!panel.draft.old_skill_vfx);

        if save {
            assert!(panel.save_draft().is_err());
        } else {
            panel.discard_draft();
        }

        assert!(panel.draft.old_skill_vfx);

        assert!(panel.applied.old_skill_vfx);

        assert!(element(&panel, "oldSkillVfxOn").visible);

        assert!(!element(&panel, "oldSkillVfx").visible);
    }

    panel.close();

    panel.open(EffectOptions {
        old_skill_vfx: false,
        ..Default::default()
    });

    assert!(!panel.applied.old_skill_vfx);

    assert!(!panel.draft.old_skill_vfx);

    assert!(element(&panel, "oldSkillVfx").visible);
}

#[test]
fn old_skill_effect_controls_hide_beneath_dropdowns_and_on_other_tabs() {
    let mut panel = video_panel(
        Language::English,
        &[RendererBackend::Vulkan, RendererBackend::Gl],
    );

    let checkbox = element(&panel, "oldSkillVfxOn");

    let point = Some((checkbox.x + 5, checkbox.y + 5));

    for control in ["rendererDrop", "resDrop", "TabAud", "TabIfc"] {
        click(&mut panel, control);

        for name in ["oldSkillVfx", "oldSkillVfxOn", "vOldSkillVfxLbl"] {
            assert!(!element(&panel, name).visible, "{control}: {name}");
        }

        assert!(panel.button(0, true, point, (800, 600), 1.0));

        assert!(panel.button(0, false, point, (800, 600), 1.0));

        assert!(panel.draft.old_skill_vfx);

        click(&mut panel, "TabVid");

        assert!(element(&panel, "oldSkillVfxOn").visible);

        assert!(element(&panel, "vOldSkillVfxLbl").visible);
    }
}

#[test]
fn old_skill_effects_label_follows_runtime_language_and_fits_between_video_controls() {
    let mut panel = video_panel(Language::English, &[RendererBackend::Vulkan]);

    let mut game = GameDir::from_files("synthetic-old-skill-effects-language", []).unwrap();

    for (language, caption) in [
        (Language::Portuguese, "Efeitos antigos de habilidades"),
        (Language::English, "Old skill effects"),
    ] {
        game.set_language(language);

        panel.apply_language(&game, &ui::Strings::default());

        assert_eq!(element(&panel, "vOldSkillVfxLbl").display, caption);

        for name in ["oldSkillVfx", "oldSkillVfxOn", "vOldSkillVfxLbl"] {
            let control = element(&panel, name);

            assert!(control.y >= element(&panel, "vsync").rect()[3]);

            assert!(control.y >= element(&panel, "vVsyncLbl").rect()[3]);

            assert!(control.rect()[3] <= element(&panel, "rendererLabel").y);

            assert!(control.rect()[2] <= panel.placed.ui.panel.right);

            assert!(control.x >= panel.placed.ui.panel.left);
        }

        assert!(element(&panel, "rendererLabel").rect()[3] <= element(&panel, "rendererDrop").y);

        assert!(element(&panel, "rendererDrop").rect()[3] <= element(&panel, "rendererNotice").y);

        assert!(
            element(&panel, "rendererNotice").rect()[3]
                <= panel.placed.ui.panel.close_button_location[1]
        );

        let index = panel
            .placed
            .ui
            .panel
            .elements
            .iter()
            .position(|element| element.name == "vOldSkillVfxLbl")
            .unwrap();

        let draws = panel.placed.ui.panel.draw_list(&Interaction::default());

        let labels: Vec<_> = draws
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::Text(text) if text.owner == Owner::Element(index) => Some(&text.text),
                _ => None,
            })
            .collect();

        assert_eq!(labels, vec![caption]);

        assert!(panel.draft.old_skill_vfx);
    }
}

#[test]
fn renderer_choices_only_include_unique_supported_backends_and_automatic() {
    let mut panel = video_panel(
        Language::Portuguese,
        &[
            RendererBackend::Vulkan,
            RendererBackend::Gl,
            RendererBackend::Vulkan,
        ],
    );

    assert_eq!(
        panel.renderers,
        [
            None,
            Some(RendererBackend::Vulkan),
            Some(RendererBackend::Gl)
        ]
    );

    assert_eq!(element(&panel, "rendererLabel").display, "Renderizador");

    assert_eq!(
        element(&panel, "rendererDrop").display,
        "Automático (Vulkan)"
    );

    assert_eq!(
        panel
            .placed
            .ui
            .panel
            .elements
            .iter()
            .filter(|e| e.name.starts_with("rendererItem"))
            .count(),
        3
    );

    click(&mut panel, "rendererDrop");

    assert_eq!(element(&panel, "rendererItem0").display, "Automático");

    click(&mut panel, "rendererItem0");

    assert!(!panel.apply_requested);

    assert!(!panel.renderers_open);

    assert!(!element(&panel, "rendererNotice").visible);
}

#[test]
fn renderer_selection_is_saved_and_restart_notice_survives_reopening() {
    let files = SettingsFile::new();

    let mut panel = video_panel(
        Language::English,
        &[RendererBackend::Vulkan, RendererBackend::Gl],
    );

    panel.path = files.path();

    click(&mut panel, "rendererDrop");

    click(&mut panel, "rendererItem2");

    assert_eq!(panel.draft.renderer, Some(RendererBackend::Gl));

    assert_eq!(panel.applied.renderer, None);

    assert!(panel.apply_requested);

    assert!(element(&panel, "rendererNotice").visible);

    panel.save_draft().unwrap();

    assert_eq!(
        Preferences::load(&files.path()).unwrap().unwrap().renderer,
        Some(RendererBackend::Gl)
    );

    panel.close();

    panel.open(Default::default());

    assert_eq!(panel.applied.renderer, Some(RendererBackend::Gl));

    assert_eq!(
        element(&panel, "rendererDrop").display,
        RendererBackend::Gl.label()
    );

    assert!(element(&panel, "rendererNotice").visible);

    click(&mut panel, "rendererDrop");

    click(&mut panel, "rendererItem0");

    panel.save_draft().unwrap();

    assert_eq!(
        Preferences::load(&files.path()).unwrap().unwrap().renderer,
        None
    );

    assert!(!element(&panel, "rendererNotice").visible);
}

#[test]
fn failed_renderer_save_and_discard_restore_the_applied_choice() {
    let files = SettingsFile::new();

    let mut panel = video_panel(
        Language::English,
        &[RendererBackend::Vulkan, RendererBackend::Gl],
    );

    panel.path = files.0.join("missing/settings.json");

    for save in [false, true] {
        click(&mut panel, "rendererDrop");

        click(&mut panel, "rendererItem2");

        assert!(element(&panel, "rendererNotice").visible);

        if save {
            assert!(panel.save_draft().is_err());
        } else {
            panel.discard_draft();
        }

        assert_eq!(panel.draft.renderer, None);

        assert_eq!(panel.applied.renderer, None);

        assert!(!element(&panel, "rendererNotice").visible);
    }
}

#[test]
fn unsupported_saved_renderer_falls_back_without_writing_preferences() {
    let files = SettingsFile::new();

    let mut panel = panel(Language::English, &[]);

    panel.path = files.path();

    panel.applied.renderer = Some(RendererBackend::Dx12);

    panel.draft = panel.applied;

    panel.startup_renderer = panel.applied.renderer;

    panel.applied.save(&files.path()).unwrap();

    panel.set_renderers(&[RendererBackend::Vulkan], RendererBackend::Vulkan);

    click(&mut panel, "TabVid");

    assert_eq!(panel.applied.renderer, None);

    assert_eq!(panel.draft.renderer, None);

    assert!(!panel.apply_requested);

    assert!(!element(&panel, "rendererNotice").visible);

    assert_eq!(
        Preferences::load(&files.path()).unwrap().unwrap().renderer,
        Some(RendererBackend::Dx12)
    );
}

#[test]
fn renderer_popup_hides_overlapping_controls_and_does_not_toggle_vsync() {
    let mut panel = video_panel(
        Language::English,
        &[
            RendererBackend::Vulkan,
            RendererBackend::Dx12,
            RendererBackend::Metal,
            RendererBackend::Gl,
            RendererBackend::BrowserWebGpu,
        ],
    );

    let checkbox = element(&panel, "vsync");

    let point = Some((checkbox.x + 5, checkbox.y + 5));

    click(&mut panel, "rendererDrop");

    for name in [
        "vsync",
        "effects",
        "resDrop",
        "rendererLabel",
        "rendererNotice",
    ] {
        assert!(!element(&panel, name).visible, "{name}");
    }

    let choices: Vec<_> = panel
        .placed
        .ui
        .panel
        .elements
        .iter()
        .filter(|element| element.name.starts_with("rendererItem"))
        .collect();

    assert_eq!(choices.len(), 6);

    for choice in &choices {
        assert!(choice.visible);

        assert!(choice.y >= element(&panel, "TabVid").rect()[3]);

        assert!(choice.rect()[3] <= element(&panel, "rendererDrop").y);

        assert!(choice.rect()[2] <= panel.placed.ui.panel.right);
    }

    for adjacent in choices.windows(2) {
        assert!(adjacent[0].rect()[3] <= adjacent[1].y);
    }

    let draws = panel.placed.ui.panel.draw_list(&Interaction::default());

    for (index, choice) in panel
        .placed
        .ui
        .panel
        .elements
        .iter()
        .enumerate()
        .filter(|(_, element)| element.name.starts_with("rendererItem"))
    {
        let text: Vec<_> = draws
            .commands
            .iter()
            .filter_map(|command| match command {
                Command::Text(text) if text.owner == Owner::Element(index) => Some(text),
                _ => None,
            })
            .collect();

        assert_eq!(text.len(), 1, "{}", choice.name);

        assert_eq!(text[0].text, choice.display);
    }

    assert!(panel.button(0, true, point, (800, 600), 1.0));

    assert!(panel.button(0, false, point, (800, 600), 1.0));

    assert_eq!(panel.draft.renderer, Some(RendererBackend::Metal));

    assert!(!panel.draft.vsync);

    assert!(!panel.renderers_open);

    click(&mut panel, "rendererDrop");

    click(&mut panel, "TabAud");

    assert!(!panel.renderers_open);

    assert!(!element(&panel, "rendererDrop").visible);

    click(&mut panel, "TabVid");

    click(&mut panel, "resDrop");

    assert!(!element(&panel, "rendererDrop").visible);

    panel.close();

    panel.open(Default::default());

    assert!(!panel.resolutions_open);

    assert!(!panel.renderers_open);

    assert!(element(&panel, "rendererDrop").visible);
}

#[test]
fn renderer_labels_and_pending_restart_notice_follow_runtime_language_changes() {
    let mut panel = video_panel(
        Language::English,
        &[RendererBackend::Vulkan, RendererBackend::Gl],
    );

    click(&mut panel, "rendererDrop");

    click(&mut panel, "rendererItem2");

    panel.applied = panel.draft;

    let mut game = GameDir::from_files("synthetic-renderer-language", []).unwrap();

    for (language, label, notice, automatic) in [
        (
            Language::Portuguese,
            "Renderizador",
            "Reinicie o jogo para aplicar.",
            "Automático",
        ),
        (
            Language::English,
            "Renderer",
            "Restart the game to apply.",
            "Automatic",
        ),
    ] {
        game.set_language(language);

        panel.apply_language(&game, &ui::Strings::default());

        assert_eq!(element(&panel, "rendererLabel").display, label);

        assert_eq!(element(&panel, "rendererNotice").display, notice);

        assert!(element(&panel, "rendererNotice").visible);

        assert_eq!(element(&panel, "rendererItem0").display, automatic);

        assert_eq!(panel.applied.renderer, Some(RendererBackend::Gl));
    }
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

        panel.set_renderers(&[RendererBackend::Dx12], RendererBackend::Dx12);

        panel.draft.language = if language == Language::Portuguese {
            Language::English
        } else {
            Language::Portuguese
        };

        panel.refresh();

        for name in [
            "languageLabel",
            "evokLabel",
            "capeRefineLbl",
            "rendererLabel",
            "rendererDrop",
            "rendererNotice",
            "vOldSkillVfxLbl",
            "vNeuralRenderingLbl",
            "viewDistanceLabel",
            "viewDistanceDrop",
            "viewDistanceItem0",
            "viewDistanceItem1",
            "viewDistanceItem2",
            "viewDistanceItem3",
            "viewDistanceItem4",
            "viewDistanceItem5",
            "viewDistanceItem6",
            "viewDistanceItem7",
            "viewDistanceItem8",
        ] {
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
