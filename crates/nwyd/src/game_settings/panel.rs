use super::*;
use crate::{
    frame::{PlannedQuad, TextureKey},
    scene::{
        login_panels::{GuiTexture, PlacedPanel},
        panels,
        view_distance::ViewDistance,
    },
    ui::PanelUi,
};
use assets::GameDir;
use formats::{config::Language, ui::pane::ElementType};
use render::device::RendererBackend;
use ui::{
    draw::{Interaction, Owner},
    input::{EventId, Manager, Mouse, PanelMut},
};

const CAPTIONS: u32 = 0x0002_0000;

#[derive(Clone, Copy)]
enum Slider {
    Music,
    Sound,
    Evok,
}

fn extend_layout(ui: &mut PanelUi, languages: &[Language]) -> Result<()> {
    ui.panel.bottom = ui.panel.top + 365;

    ui.panel.close_button_location[1] = 333;

    if let Some(close) = &mut ui.panel.close_button {
        close.y = 333;
    }

    for element in &mut ui.panel.elements {
        if element.name.is_empty() && element.element_type == ElementType::Image {
            element.height = 364;
        }
    }

    for (source, name, kind, y) in [
        ("aMusLbl", "evokLabel", ElementType::Static, 158),
        ("musFill", "evokFill", ElementType::Image, 182),
        ("musBar", "evokBar", ElementType::Image, 180),
        ("musMark", "evokMark", ElementType::Image, 178),
    ] {
        let mut element = ui
            .panel
            .element(source, kind)
            .with_context(|| format!("missing settings template {source}"))?
            .clone_template();

        element.name = name.into();

        element.name_hash = ui::hash::name_hash(name);

        ui.panel.next_element_id += 1;

        element.id = ui.panel.next_element_id;

        element.y = y;

        if kind == ElementType::Static {
            element.width = 207;
        }

        ui.panel.elements.push(element);
    }

    for (index, [width, height]) in RESOLUTIONS.iter().enumerate().skip(9) {
        let mut element = ui
            .panel
            .element("resItem0", ElementType::Button)
            .context("missing resolution template")?
            .clone_template();

        element.name = format!("resItem{index}");

        element.name_hash = ui::hash::name_hash(&element.name);

        ui.panel.next_element_id += 1;

        element.id = ui.panel.next_element_id;

        element.x = 25 + (index % 3) as i32 * 71;

        element.y = 96 + (index / 3) as i32 * 16;

        element.set_display(&format!("{width} x {height}"));

        ui.panel.elements.push(element);
    }

    for (source, name, kind, x, y, caption) in [
        ("effects", "vsync", ElementType::Button, 25, 162, ""),
        ("effectsOn", "vsyncOn", ElementType::Button, 25, 162, ""),
        (
            "vEfx1Lbl",
            "vVsyncLbl",
            ElementType::Static,
            46,
            162,
            "VSync",
        ),
        ("effects", "oldSkillVfx", ElementType::Button, 25, 180, ""),
        (
            "effectsOn",
            "oldSkillVfxOn",
            ElementType::Button,
            25,
            180,
            "",
        ),
        (
            "vEfx1Lbl",
            "vOldSkillVfxLbl",
            ElementType::Static,
            46,
            180,
            "",
        ),
        (
            "effects",
            "neuralRendering",
            ElementType::Button,
            25,
            198,
            "",
        ),
        (
            "effectsOn",
            "neuralRenderingOn",
            ElementType::Button,
            25,
            198,
            "",
        ),
        (
            "vEfx1Lbl",
            "vNeuralRenderingLbl",
            ElementType::Static,
            46,
            198,
            "",
        ),
        ("effects", "capeRefine", ElementType::Button, 25, 209, ""),
        (
            "effectsOn",
            "capeRefineOn",
            ElementType::Button,
            25,
            209,
            "",
        ),
        (
            "vEfx1Lbl",
            "capeRefineLbl",
            ElementType::Static,
            46,
            209,
            "Refinação Capa",
        ),
        (
            "effects",
            "hideItemAnnouncements",
            ElementType::Button,
            25,
            287,
            "",
        ),
        (
            "effectsOn",
            "hideItemAnnouncementsOn",
            ElementType::Button,
            25,
            287,
            "",
        ),
        (
            "vEfx1Lbl",
            "hideItemAnnouncementsLbl",
            ElementType::Static,
            46,
            287,
            "",
        ),
    ] {
        let mut element = ui
            .panel
            .element(source, kind)
            .context("missing settings checkbox template")?
            .clone_template();

        element.name = name.into();

        element.name_hash = ui::hash::name_hash(name);

        ui.panel.next_element_id += 1;

        element.id = ui.panel.next_element_id;

        element.x = x;

        element.y = y;

        if kind == ElementType::Static {
            element.width = 185;

            element.set_display(caption);
        }

        ui.panel.elements.push(element);
    }

    for (source, name, kind, y, height) in [
        ("aMusLbl", "rendererLabel", ElementType::Static, 198, 14),
        ("resDrop", "rendererDrop", ElementType::Button, 216, 20),
        ("aMusLbl", "rendererNotice", ElementType::Static, 244, 18),
        ("aMusLbl", "viewDistanceLabel", ElementType::Static, 284, 14),
        ("resDrop", "viewDistanceDrop", ElementType::Button, 302, 20),
        ("aMusLbl", "languageLabel", ElementType::Static, 233, 14),
        ("resDrop", "languageDrop", ElementType::Button, 251, 20),
    ] {
        let mut element = ui
            .panel
            .element(source, kind)
            .with_context(|| format!("missing language settings template {source}"))?
            .clone_template();

        element.name = name.into();

        element.name_hash = ui::hash::name_hash(name);

        ui.panel.next_element_id += 1;

        element.id = ui.panel.next_element_id;

        element.x = 25;

        element.y = y;

        element.width = 207;

        element.height = height;

        element.multiline = false;

        element.set_display("");

        ui.panel.elements.push(element);
    }

    let view_distance_choices_top = ui
        .panel
        .element("viewDistanceDrop", ElementType::Button)
        .context("missing view distance selector")?
        .y
        - ViewDistance::ALL.len() as i32 * 18
        - 2;

    for index in 0..ViewDistance::ALL.len() {
        let mut element = ui
            .panel
            .element("resItem0", ElementType::Button)
            .context("missing view distance choice template")?
            .clone_template();

        element.name = format!("viewDistanceItem{index}");
        element.name_hash = ui::hash::name_hash(&element.name);
        ui.panel.next_element_id += 1;
        element.id = ui.panel.next_element_id;
        element.x = 25;
        element.y = view_distance_choices_top + index as i32 * 18;
        element.width = 207;
        element.height = 18;
        element.multiline = false;
        element.msg_receive = true;
        element.set_display("");
        ui.panel.elements.push(element);
    }

    let language_choices_top = 223 - languages.len() as i32 * 18;

    for (index, &language) in languages.iter().enumerate() {
        let mut element = ui
            .panel
            .element("resItem0", ElementType::Button)
            .context("missing language choice template")?
            .clone_template();

        element.name = format!("languageItem{index}");

        element.name_hash = ui::hash::name_hash(&element.name);

        ui.panel.next_element_id += 1;

        element.id = ui.panel.next_element_id;

        element.x = 25;

        element.y = language_choices_top + index as i32 * 18;

        element.width = 207;

        element.height = 18;

        element.set_display(language_label(language));

        ui.panel.elements.push(element);
    }

    Ok(())
}

fn renderer_choices(ui: &mut PanelUi, count: usize, neural_rendering_available: bool) {
    ui.panel
        .elements
        .retain(|element| !element.name.starts_with("rendererItem"));

    let Some(template) = ui.panel.element("resItem0", ElementType::Button).cloned() else {
        return;
    };

    let top = 214 + i32::from(neural_rendering_available) * 18 - count as i32 * 18;

    for index in 0..count {
        let mut element = template.clone_template();

        element.name = format!("rendererItem{index}");

        element.name_hash = ui::hash::name_hash(&element.name);

        ui.panel.next_element_id += 1;

        element.id = ui.panel.next_element_id;

        element.x = 25;

        element.y = top + index as i32 * 18;

        element.width = 207;

        element.height = 18;

        element.multiline = false;

        element.msg_receive = true;

        element.set_display("");

        ui.panel.elements.push(element);
    }
}

fn renderer_caption(
    renderer: Option<RendererBackend>,
    active: Option<RendererBackend>,
    portuguese: bool,
) -> String {
    match renderer {
        Some(renderer) => renderer.label().into(),
        None => {
            let automatic = if portuguese {
                "Automático"
            } else {
                "Automatic"
            };

            active.map_or_else(
                || automatic.into(),
                |active| format!("{automatic} ({})", active.label()),
            )
        }
    }
}

fn view_distance_caption(distance: ViewDistance, portuguese: bool) -> &'static str {
    match (distance, portuguese) {
        (ViewDistance::Retail, true) => "Original (1×)",
        (ViewDistance::Retail, false) => "Retail (1×)",
        (ViewDistance::Extended, true) => "Ampliada (1,5×)",
        (ViewDistance::Extended, false) => "Extended (1.5×)",
        (ViewDistance::Distant, true) => "Distante (2×)",
        (ViewDistance::Distant, false) => "Distant (2×)",
        (ViewDistance::Far, true) => "Muito distante (3×)",
        (ViewDistance::Far, false) => "Far (3×)",
        (ViewDistance::Maximum, _) => "4×",
        (ViewDistance::FiveTimes, _) => "5×",
        (ViewDistance::SixTimes, _) => "6×",
        (ViewDistance::SevenTimes, _) => "7×",
        (ViewDistance::EightTimes, _) => "8×",
    }
}

pub struct Panel {
    pub(crate) chains: crate::fonts::Chains,
    placed: PlacedPanel,
    textures: Vec<GuiTexture>,
    manager: Manager,
    scale: f32,
    tab: usize,
    resolutions_open: bool,
    renderers_open: bool,
    view_distance_open: bool,
    renderers: Vec<Option<RendererBackend>>,
    startup_renderer: Option<RendererBackend>,
    active_renderer: Option<RendererBackend>,
    neural_rendering_available: bool,
    languages_open: bool,
    languages: Vec<Language>,
    language: Language,
    slider_drag: Option<Slider>,
    pub(crate) visible: bool,
    pub(crate) draft: Preferences,
    pub(crate) applied: Preferences,
    pub(crate) path: std::path::PathBuf,
    owned: [bool; 3],
    escape_owned: bool,
    pub(crate) apply_requested: bool,
    observed_costume_flags: Option<u8>,
}

impl Panel {
    #[cfg(test)]
    pub(crate) fn for_test(preferences: Preferences) -> Self {
        let chains = crate::fonts::Chains::default();

        let mut ui = PanelUi::from_resource(
            ui::Panel::default(),
            ui::resource::Resource::default(),
            &chains,
            1.0,
        );

        ui.panel.visible = false;

        Self {
            chains,
            placed: PlacedPanel { ui, origin: (0, 0) },
            textures: Vec::new(),
            manager: Manager {
                screen: [0, 0, 800, 600],
                ..Default::default()
            },
            scale: 1.0,
            tab: 0,
            resolutions_open: false,
            renderers_open: false,
            view_distance_open: false,
            renderers: vec![None],
            startup_renderer: preferences.renderer,
            active_renderer: None,
            neural_rendering_available: false,
            languages_open: false,
            languages: vec![preferences.language],
            language: preferences.language,
            slider_drag: None,
            visible: false,
            draft: preferences,
            applied: preferences,
            path: "unused-test-settings.json".into(),
            owned: [false; 3],
            escape_owned: false,
            apply_requested: false,
            observed_costume_flags: None,
        }
    }

    pub fn new(
        preferences: Preferences,
        path: std::path::PathBuf,
        game: &GameDir,
        size: (u32, u32),
    ) -> Result<Self> {
        let layout = crate::scene::login_static::Layout::new(size.0, size.1);

        let scale = layout.ui_scale();

        let chains = crate::fonts::load(game, crate::fonts::system_font_dir().as_deref());

        let mut ui = PanelUi::build(
            game,
            b"ui/Panel/game/GameConfig.pane",
            &crate::ui::strings(game),
            &chains,
            scale,
        )?;

        let languages = available_languages(game);

        extend_layout(&mut ui, &languages)?;

        let textures = ui
            .referenced_textures(&Interaction::default())
            .into_iter()
            .map(|slot| crate::scene::login_panels::load_guimat_texture(game, &ui, slot))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();

        for element in &mut ui.panel.elements {
            if element.element_type == ElementType::Button {
                element.msg_receive = true;
            }
        }

        let screen = layout.virtual_screen();

        let origin = ui.origin(screen);

        let mut panel = Self {
            chains,
            placed: PlacedPanel { ui, origin },
            scale,
            textures,
            manager: Manager {
                screen,
                ..Default::default()
            },
            tab: 0,
            resolutions_open: false,
            renderers_open: false,
            view_distance_open: false,
            renderers: vec![None],
            startup_renderer: preferences.renderer,
            active_renderer: None,
            neural_rendering_available: false,
            languages_open: false,
            languages,
            language: game.language(),
            slider_drag: None,
            visible: false,
            draft: preferences,
            applied: preferences,
            path,
            owned: [false; 3],
            escape_owned: false,
            apply_requested: false,
            observed_costume_flags: None,
        };

        panel.refresh();

        Ok(panel)
    }

    pub(crate) fn set_renderers(&mut self, available: &[RendererBackend], active: RendererBackend) {
        self.renderers = vec![None];

        for &renderer in available {
            if !self.renderers.contains(&Some(renderer)) {
                self.renderers.push(Some(renderer));
            }
        }

        if !self.renderers.contains(&self.applied.renderer) {
            self.applied.renderer = None;
        }

        if !self.renderers.contains(&self.draft.renderer) {
            self.draft.renderer = None;
        }

        if !self.renderers.contains(&self.startup_renderer) {
            self.startup_renderer = None;
        }

        self.active_renderer = Some(active);

        self.renderers_open = false;

        self.view_distance_open = false;

        self.cancel_input();

        renderer_choices(
            &mut self.placed.ui,
            self.renderers.len(),
            self.neural_rendering_available,
        );

        self.refresh();
    }

    pub(crate) fn set_neural_rendering_available(&mut self, available: bool) {
        if self.neural_rendering_available == available {
            return;
        }

        self.neural_rendering_available = available;

        self.renderers_open = false;

        self.resolutions_open = false;

        self.view_distance_open = false;

        self.cancel_input();

        renderer_choices(&mut self.placed.ui, self.renderers.len(), available);

        self.refresh();
    }

    fn refresh(&mut self) {
        let portuguese = self.language == Language::Portuguese;

        let interface_visible = self.tab == 2 && !self.languages_open;

        let video_visible = self.tab == 0
            && !self.resolutions_open
            && !self.renderers_open
            && !self.view_distance_open;

        let neural_rendering_visible = video_visible && self.neural_rendering_available;

        let renderer_offset = i32::from(self.neural_rendering_available) * 18;

        let restart_required = self.draft.renderer != self.startup_renderer;

        for element in &mut self.placed.ui.panel.elements {
            let name = element.name.as_str();

            element.visible = match name {
                "TabVid" | "TabAud" | "TabIfc" | "" => true,
                n if n.starts_with("resItem") => self.tab == 0 && self.resolutions_open,
                "resDrop" | "vResLbl" => {
                    self.tab == 0 && !self.renderers_open && !self.view_distance_open
                }
                n if n.starts_with("rendererItem") => self.tab == 0 && self.renderers_open,
                "rendererLabel" => video_visible,
                "rendererDrop" => {
                    self.tab == 0 && !self.resolutions_open && !self.view_distance_open
                }
                "rendererNotice" => video_visible && restart_required,
                "viewDistanceLabel" => video_visible,
                "viewDistanceDrop" => {
                    self.tab == 0 && !self.resolutions_open && !self.renderers_open
                }
                n if n.starts_with("viewDistanceItem") => self.tab == 0 && self.view_distance_open,
                n if n.starts_with("languageItem") => self.tab == 2 && self.languages_open,
                "languageLabel" | "languageDrop" => self.tab == 2,
                "windowMode" => video_visible && self.draft.fullscreen,
                "windowModeOn" => video_visible && !self.draft.fullscreen,
                "fsMode" => video_visible && !self.draft.fullscreen,
                "fsModeOn" => video_visible && self.draft.fullscreen,
                "effects" => video_visible && !self.draft.effects,
                "effectsOn" => video_visible && self.draft.effects,
                "vsync" => video_visible && !self.draft.vsync,
                "vsyncOn" => video_visible && self.draft.vsync,
                "oldSkillVfx" => video_visible && !self.draft.old_skill_vfx,
                "oldSkillVfxOn" => video_visible && self.draft.old_skill_vfx,
                "neuralRendering" => neural_rendering_visible && !self.draft.neural_rendering,
                "neuralRenderingOn" => neural_rendering_visible && self.draft.neural_rendering,
                "vNeuralRenderingLbl" => neural_rendering_visible,
                "vModeLbl" | "vEfxHdr" | "vWinLbl" | "vFsLbl" | "vEfx1Lbl" | "vVsyncLbl"
                | "vOldSkillVfxLbl" => video_visible,
                "aMusLbl" | "aSfxLbl" | "musFill" | "musBar" | "musMark" | "sfxFill" | "sfxBar"
                | "sfxMark" => self.tab == 1,
                "helmoChk" => interface_visible && !self.draft.interface.hide_helmet,
                "helmoChkOn" => interface_visible && self.draft.interface.hide_helmet,
                "costumeChk" => interface_visible && !self.draft.interface.hide_costume,
                "costumeChkOn" => interface_visible && self.draft.interface.hide_costume,
                "mountChk" => interface_visible && !self.draft.interface.hide_mount_costume,
                "mountChkOn" => interface_visible && self.draft.interface.hide_mount_costume,
                "hudChk" => interface_visible && !self.draft.interface.hide_hud,
                "hudChkOn" => interface_visible && self.draft.interface.hide_hud,
                "zoomChk" => interface_visible && !self.draft.interface.zoom,
                "zoomChkOn" => interface_visible && self.draft.interface.zoom,
                "capeRefine" => interface_visible && !self.draft.interface.cape_refinement,
                "capeRefineOn" => interface_visible && self.draft.interface.cape_refinement,
                "hideItemAnnouncements" => {
                    interface_visible && !self.draft.interface.hide_item_announcements
                }
                "hideItemAnnouncementsOn" => {
                    interface_visible && self.draft.interface.hide_item_announcements
                }
                _ => interface_visible,
            };

            if let Some(tab) = ["TabVid", "TabAud", "TabIfc"]
                .iter()
                .position(|n| *n == name)
                && let Some(button) = element.widget.button_mut()
            {
                button.checked = self.tab == tab;
            }

            match name {
                "vOldSkillVfxLbl" => element.set_display(if portuguese {
                    "Efeitos antigos de habilidades"
                } else {
                    "Old skill effects"
                }),
                "vNeuralRenderingLbl" => element.set_display("DLSS 5 (experimental)"),
                "viewDistanceLabel" => element.set_display(if portuguese {
                    "Distância de visão"
                } else {
                    "View distance"
                }),
                "viewDistanceDrop" => {
                    element.set_display(view_distance_caption(self.draft.view_distance, portuguese))
                }
                n if n.starts_with("viewDistanceItem") => {
                    if let Some(&distance) = n
                        .strip_prefix("viewDistanceItem")
                        .and_then(|index| index.parse::<usize>().ok())
                        .and_then(|index| ViewDistance::ALL.get(index))
                    {
                        element.set_display(view_distance_caption(distance, portuguese));
                    }
                }
                "rendererLabel" => {
                    element.y = 198 + renderer_offset;

                    element.set_display(if portuguese {
                        "Renderizador"
                    } else {
                        "Renderer"
                    });
                }
                "rendererDrop" => {
                    element.y = 216 + renderer_offset;

                    element.enabled = self.renderers.len() > 1;

                    element.set_display(&renderer_caption(
                        self.draft.renderer,
                        self.active_renderer,
                        portuguese,
                    ));
                }
                "rendererNotice" => {
                    element.y = 244 + renderer_offset;

                    element.set_display(if portuguese {
                        "Reinicie o jogo para aplicar."
                    } else {
                        "Restart the game to apply."
                    });
                }
                n if n.starts_with("rendererItem") => {
                    if let Some(&renderer) = n
                        .strip_prefix("rendererItem")
                        .and_then(|index| index.parse::<usize>().ok())
                        .and_then(|index| self.renderers.get(index))
                    {
                        element.set_display(&renderer_caption(renderer, None, portuguese));
                    }
                }
                "evokLabel" => element.set_display(&format!(
                    "{}: {}%",
                    if portuguese {
                        "Tamanho dos Evoks"
                    } else {
                        "Evok size"
                    },
                    self.draft.interface.evok_size
                )),
                "capeRefineLbl" => element.set_display(if portuguese {
                    "Refinação Capa"
                } else {
                    "Cape refinement"
                }),
                "hideItemAnnouncementsLbl" => element.set_display(if portuguese {
                    "Ocultar anúncios de itens"
                } else {
                    "Hide item announcements"
                }),
                "languageLabel" => {
                    element.set_display(if portuguese { "Idioma" } else { "Language" })
                }
                "languageDrop" => {
                    element.enabled = !self.languages.is_empty();

                    element.set_display(language_label(self.draft.language));
                }
                "evokFill" => {
                    element.width =
                        198 * i32::from(self.draft.interface.evok_size.saturating_sub(50)) / 50;
                }
                "evokMark" => {
                    element.x = 30
                        + 171 * i32::from(self.draft.interface.evok_size.saturating_sub(50)) / 50;
                }
                "musFill" => element.width = 198 * i32::from(self.draft.music) / 100,
                "sfxFill" => element.width = 198 * i32::from(self.draft.sound) / 100,
                "musMark" => element.x = 30 + 171 * i32::from(self.draft.music) / 100,
                "sfxMark" => element.x = 30 + 171 * i32::from(self.draft.sound) / 100,
                "resDrop" => element.set_display(&format!(
                    "{} x {}",
                    self.draft.resolution[0], self.draft.resolution[1]
                )),
                _ => {}
            }
        }
    }

    pub(crate) fn sync_costumes(&mut self, flags: u8) {
        let flags = flags & 7;

        let changed = self.observed_costume_flags.map_or(7, |old| old ^ flags);

        self.observed_costume_flags = Some(flags);

        if changed & 4 != 0 {
            self.applied.interface.hide_helmet = flags & 4 != 0;

            self.draft.interface.hide_helmet = flags & 4 != 0;
        }

        if changed & 1 != 0 {
            self.applied.interface.hide_costume = flags & 1 != 0;

            self.draft.interface.hide_costume = flags & 1 != 0;
        }

        if changed & 2 != 0 {
            self.applied.interface.hide_mount_costume = flags & 2 != 0;

            self.draft.interface.hide_mount_costume = flags & 2 != 0;
        }

        if changed != 0 {
            self.refresh();
        }
    }

    pub(crate) fn reset_costumes(&mut self, flags: u8) {
        self.observed_costume_flags = None;

        self.sync_costumes(flags);
    }

    pub(crate) fn open(&mut self, effects: EffectOptions) {
        self.applied.effects = effects.detail.effects;

        self.applied.old_skill_vfx = effects.old_skill_vfx;

        self.applied.grass = effects.grass;

        self.draft = self.applied;

        self.visible = true;

        self.placed.ui.panel.visible = true;

        self.refresh();
    }

    pub(crate) fn resize(&mut self, size: (u32, u32)) {
        let layout = crate::scene::login_static::Layout::new(size.0, size.1);

        self.scale = layout.ui_scale();

        self.placed.ui.scale = self.scale;

        self.manager.screen = layout.virtual_screen();

        self.placed.origin = self.placed.ui.reposition(self.manager.screen);

        self.cancel_input();
    }

    pub(crate) fn close(&mut self) {
        self.visible = false;

        self.resolutions_open = false;

        self.renderers_open = false;

        self.view_distance_open = false;

        self.languages_open = false;

        self.slider_drag = None;

        self.manager = Manager {
            screen: self.manager.screen,
            button_sounds: std::mem::take(&mut self.manager.button_sounds),
            ..Default::default()
        };
    }

    pub(crate) fn cancel_input(&mut self) {
        self.owned = [false; 3];

        self.escape_owned = false;

        self.slider_drag = None;

        self.manager = Manager {
            screen: self.manager.screen,
            button_sounds: std::mem::take(&mut self.manager.button_sounds),
            ..Default::default()
        };
    }

    pub(crate) fn blocks_pointer(&self) -> bool {
        self.visible || self.owned.iter().any(|&v| v)
    }

    pub(crate) fn take_sounds(&mut self) -> Vec<i64> {
        std::mem::take(&mut self.manager.button_sounds)
    }

    pub(crate) fn key(&mut self, key: &winit::keyboard::Key, down: bool) -> bool {
        let escape = *key == winit::keyboard::Key::Named(winit::keyboard::NamedKey::Escape);

        if escape && self.escape_owned {
            if !down {
                self.escape_owned = false;
            }

            return true;
        }

        if !self.visible {
            return false;
        }

        if down && escape {
            self.escape_owned = true;

            self.close();
        }

        true
    }

    fn route(&mut self, event: Mouse, point: (i32, i32)) {
        let events = self.manager.mouse(
            &mut [PanelMut {
                panel: &mut self.placed.ui.panel,
                origin: &mut self.placed.origin,
            }],
            event,
            point.0,
            point.1,
        );

        for event in events {
            if event.id != EventId::Click {
                continue;
            }

            if event.owner == Some(Owner::Close) {
                self.close();

                break;
            }

            let before = self.draft;

            match event.name.as_str() {
                "TabVid" => {
                    self.tab = 0;

                    self.resolutions_open = false;

                    self.renderers_open = false;

                    self.view_distance_open = false;

                    self.languages_open = false;
                }
                "TabAud" => {
                    self.tab = 1;

                    self.resolutions_open = false;

                    self.renderers_open = false;

                    self.view_distance_open = false;

                    self.languages_open = false;
                }
                "TabIfc" => {
                    self.tab = 2;

                    self.resolutions_open = false;

                    self.renderers_open = false;

                    self.view_distance_open = false;

                    self.languages_open = false;
                }
                "helmoChk" | "helmoChkOn" => self.draft.interface.hide_helmet ^= true,
                "costumeChk" | "costumeChkOn" => {
                    self.draft.interface.hide_costume ^= true;
                }
                "mountChk" | "mountChkOn" => {
                    self.draft.interface.hide_mount_costume ^= true;
                }
                "hudChk" | "hudChkOn" => self.draft.interface.hide_hud ^= true,
                "zoomChk" | "zoomChkOn" => self.draft.interface.zoom ^= true,
                "capeRefine" | "capeRefineOn" => {
                    self.draft.interface.cape_refinement ^= true;
                }
                "hideItemAnnouncements" | "hideItemAnnouncementsOn" => {
                    self.draft.interface.hide_item_announcements ^= true;
                }
                "resDrop" => {
                    self.resolutions_open = !self.resolutions_open;

                    self.renderers_open = false;

                    self.view_distance_open = false;
                }
                "rendererDrop" => {
                    self.renderers_open = !self.renderers_open;

                    self.resolutions_open = false;

                    self.view_distance_open = false;
                }
                "viewDistanceDrop" => {
                    self.view_distance_open = !self.view_distance_open;
                    self.resolutions_open = false;
                    self.renderers_open = false;
                }
                "languageDrop" => self.languages_open = !self.languages_open,
                "windowMode" | "windowModeOn" => self.draft.fullscreen = false,
                "fsMode" | "fsModeOn" => self.draft.fullscreen = true,
                "effects" | "effectsOn" => self.draft.effects = !self.draft.effects,
                "vsync" | "vsyncOn" => self.draft.vsync = !self.draft.vsync,
                "oldSkillVfx" | "oldSkillVfxOn" => {
                    self.draft.old_skill_vfx = !self.draft.old_skill_vfx;
                }
                "neuralRendering" | "neuralRenderingOn" if self.neural_rendering_available => {
                    self.draft.neural_rendering = !self.draft.neural_rendering;
                }
                name => {
                    if let Some(index) = name
                        .strip_prefix("resItem")
                        .and_then(|n| n.parse::<usize>().ok())
                        .filter(|i| *i < RESOLUTIONS.len())
                    {
                        self.draft.resolution = RESOLUTIONS[index];

                        self.resolutions_open = false;
                    } else if let Some(&renderer) = name
                        .strip_prefix("rendererItem")
                        .and_then(|index| index.parse::<usize>().ok())
                        .and_then(|index| self.renderers.get(index))
                    {
                        self.draft.renderer = renderer;

                        self.renderers_open = false;
                    } else if let Some(&distance) = name
                        .strip_prefix("viewDistanceItem")
                        .and_then(|index| index.parse::<usize>().ok())
                        .and_then(|index| ViewDistance::ALL.get(index))
                    {
                        self.draft.view_distance = distance;
                        self.view_distance_open = false;
                    } else if let Some(&language) = name
                        .strip_prefix("languageItem")
                        .and_then(|n| n.parse::<usize>().ok())
                        .and_then(|index| self.languages.get(index))
                    {
                        self.draft.language = language;

                        self.languages_open = false;
                    }
                }
            }

            self.apply_requested |= self.draft != before;

            self.refresh();
        }
    }

    fn slide(&mut self, point: (i32, i32), slider: Slider) {
        let offset = (point.0 - self.placed.origin.0 - 30).clamp(0, 198);

        let (target, minimum, span) = match slider {
            Slider::Music => (&mut self.draft.music, 0, 100),
            Slider::Sound => (&mut self.draft.sound, 0, 100),
            Slider::Evok => (&mut self.draft.interface.evok_size, 50, 50),
        };

        let value = (minimum + offset * span / 198) as u8;

        self.apply_requested |= *target != value;

        *target = value;

        self.refresh();
    }

    pub(crate) fn motion(&mut self, point: Option<(i32, i32)>) {
        if !self.visible {
            return;
        }

        if let Some(point) = self.virtual_point(point) {
            if let Some(slider) = self.slider_drag {
                self.slide(point, slider);
            } else {
                self.route(Mouse::Move, point);
            }
        }
    }

    pub(crate) fn button(
        &mut self,
        button: usize,
        down: bool,
        point: Option<(i32, i32)>,
        _size: (u32, u32),
        _scale: f32,
    ) -> bool {
        let point = self.virtual_point(point);

        if button >= 3 {
            return self.visible;
        }

        if !down {
            let owned = std::mem::take(&mut self.owned[button]);

            if owned && button == 0 {
                if self.slider_drag.take().is_none() && self.visible {
                    self.route(Mouse::LeftUp, point.unwrap_or((-1, -1)));
                }
            }

            return owned;
        }

        if !self.visible {
            return false;
        }

        self.owned[button] = true;

        if button != 0 {
            return true;
        }

        if let Some(point) = point {
            if self.tab == 1 || self.tab == 2 {
                let (x, y) = (
                    point.0 - self.placed.origin.0,
                    point.1 - self.placed.origin.1,
                );

                for (name, slider) in [
                    ("musBar", Slider::Music),
                    ("sfxBar", Slider::Sound),
                    ("evokBar", Slider::Evok),
                ] {
                    let Some(bar) = self.placed.ui.panel.element(name, ElementType::Image) else {
                        continue;
                    };

                    if bar.visible
                        && (bar.x..bar.x + bar.width).contains(&x)
                        && (bar.y - 5..bar.y + bar.height + 5).contains(&y)
                    {
                        self.slider_drag = Some(slider);

                        self.slide(point, slider);

                        return true;
                    }
                }
            }

            self.route(Mouse::Move, point);

            self.route(Mouse::LeftDown, point);
        }

        true
    }

    pub fn draws(&mut self) -> Vec<PlannedQuad> {
        let mut draws = Vec::new();

        if self.visible {
            let interaction =
                self.manager
                    .interaction_for(0, &self.placed.ui.panel, self.placed.origin);

            panels::append_panel(
                &mut draws,
                &mut self.placed,
                &interaction,
                CAPTIONS,
                true,
                None,
            );
        }

        draws
    }

    fn virtual_point(&self, point: Option<(i32, i32)>) -> Option<(i32, i32)> {
        point.map(|(x, y)| {
            (
                (x as f32 / self.scale) as i32,
                (y as f32 / self.scale) as i32,
            )
        })
    }

    pub(crate) fn queue(
        &mut self,
        gpu: &render::Gpu,
        renderer: &mut crate::frame::SceneRenderer,
        size: (u32, u32),
        _scale: f32,
        pointers: &crate::scene::cursor::Pointers,
        position: Option<(i32, i32)>,
    ) {
        if !self.visible {
            return;
        }

        let draws = self.draws();

        for texture in &self.textures {
            renderer.upload_once(
                gpu,
                TextureKey::GuiMat(texture.slot),
                &texture.texture,
                render::texture::AlphaPolicy::Stored,
                &texture.path,
            );
        }

        renderer.sync_captions(gpu, CAPTIONS, &self.placed.ui);

        for draw in &draws {
            renderer.push(draw);
        }

        pointers.queue(
            gpu,
            renderer,
            size,
            crate::scene::cursor::FRAME_NORMAL,
            position,
        );
    }

    pub(crate) fn save_draft(&mut self) -> Result<Preferences> {
        if let Err(error) = self.draft.save(&self.path) {
            self.draft = self.applied;

            self.refresh();

            return Err(error);
        }

        self.applied = self.draft;

        Ok(self.applied)
    }

    pub(crate) fn discard_draft(&mut self) {
        self.draft = self.applied;

        self.refresh();
    }

    pub(crate) fn apply_language(&mut self, game: &GameDir, strings: &ui::Strings) {
        self.placed.ui.relocalize(game, strings);

        self.language = game.language();

        self.refresh();
    }
}

#[cfg(test)]
mod tests;
