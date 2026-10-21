use crate::{
    fonts::Chains,
    frame::{PlannedQuad, SceneRenderer, TextureKey},
    scene::{
        login_panels::{GuiTexture, PlacedPanel, load_guimat_texture},
        login_static::Layout,
        panels,
    },
    ui::PanelUi,
};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::{config::Language, ui::pane::ElementType};
use ui::{
    draw::Interaction,
    input::{EventId, Manager, Mouse, PanelMut},
};

const CAPTIONS: u32 = 0x0003_0000;
const WIDTH: i32 = 180;
const HEIGHT: i32 = 26;
const MARGIN: i32 = 12;
const BUTTON: &str = "downloadUpdate";
const BUTTON_SKINS: [&str; 3] = ["UI.ButtonN", "UI.ButtonD", "UI.ButtonO"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum Status {
    #[default]
    Checking,
    UpToDate,
    Available,
    Unavailable,
    Development,
}

#[derive(Debug)]
pub(crate) struct Panel {
    placed: PlacedPanel,
    textures: Vec<GuiTexture>,
    manager: Manager,
    owned: [bool; 3],
    download_requested: bool,
    status: Status,
    language: Language,
}

impl Panel {
    pub(crate) fn new(game: &GameDir, chains: &Chains, size: (u32, u32)) -> Result<Self> {
        let mut ui = PanelUi::build(
            game,
            b"ui/Panel/game/SystemMenu.pane",
            &crate::ui::strings(game),
            chains,
            Layout::new(size.0, size.1).ui_scale(),
        )?;

        let mut button = ui
            .panel
            .element("Exit", ElementType::Button)
            .context("missing system menu button template")?
            .clone_template();

        button.name = BUTTON.into();
        button.name_hash = ui::hash::name_hash(BUTTON);
        button.x = 0;
        button.y = 0;
        button.width = WIDTH;
        button.height = HEIGHT;
        button.visible = true;
        button.enabled = true;
        button.msg_receive = true;
        button.accel_key = 0;
        button.is_default = false;

        for (layer, alias) in BUTTON_SKINS.iter().enumerate() {
            ui.resource
                .alias(alias)
                .context("missing update button skin")?;
            button.set_layer_texture_alias(layer, alias, &ui.resource);
        }

        ui.panel = ui::Panel {
            right: WIDTH,
            bottom: HEIGHT,
            movable: false,
            allow_focus: false,
            enter_defaults: false,
            next_element_id: button.id,
            elements: vec![button],
            ..Default::default()
        };

        let textures = ui
            .referenced_textures(&Interaction::default())
            .into_iter()
            .map(|slot| load_guimat_texture(game, &ui, slot))
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .flatten()
            .collect();

        Ok(Self::from_ui(ui, textures, game.language(), size))
    }

    fn from_ui(
        ui: PanelUi,
        textures: Vec<GuiTexture>,
        language: Language,
        size: (u32, u32),
    ) -> Self {
        let mut panel = Self {
            placed: PlacedPanel { ui, origin: (0, 0) },
            textures,
            manager: Manager::default(),
            owned: [false; 3],
            download_requested: false,
            status: Status::Checking,
            language,
        };

        panel.set_language(language);
        panel.resize(size);
        panel
    }

    pub(crate) fn set_language(&mut self, language: Language) {
        self.language = language;
        self.refresh();
    }

    pub(crate) fn set_status(&mut self, status: Status) {
        if self.status == status {
            return;
        }

        self.cancel_input();
        self.status = status;
        self.refresh();
    }

    fn refresh(&mut self) {
        let label = match (self.status, self.language == Language::Portuguese) {
            (Status::Checking, true) => "Verificando versão…",
            (Status::Checking, false) => "Checking updates…",
            (Status::UpToDate, true) => "Versão atualizada",
            (Status::UpToDate, false) => "Up to date",
            (Status::Available, true) => "Nova versão disponível",
            (Status::Available, false) => "New version available",
            (Status::Unavailable, true) => "Falha ao verificar versão",
            (Status::Unavailable, false) => "Update check failed",
            (Status::Development, true) => "Versão de desenvolvimento",
            (Status::Development, false) => "Development build",
        };

        if let Some(button) = self
            .placed
            .ui
            .panel
            .element_mut(BUTTON, ElementType::Button)
        {
            button.set_display(label);
            button.enabled = self.status == Status::Available;

            if let Some(face) = button.layers.first_mut() {
                face.texture_color.disabled = face.texture_color.normal;
                face.font_color.disabled = face.font_color.normal;
            }
        }
    }

    pub(crate) fn resize(&mut self, size: (u32, u32)) {
        let layout = Layout::new(size.0, size.1);
        self.placed.ui.scale = layout.ui_scale();
        self.manager.screen = layout.virtual_screen();
        self.placed.origin = (
            (self.manager.screen[2] - WIDTH - MARGIN).max(0),
            (self.manager.screen[3] - HEIGHT - MARGIN).max(0),
        );
        self.cancel_input();
    }

    pub(crate) fn cancel_input(&mut self) {
        self.owned = [false; 3];
        self.download_requested = false;
        ui::input::cancel_panel_pointer(&mut self.placed.ui.panel);
        self.manager = Manager {
            screen: self.manager.screen,
            ..Default::default()
        };
    }

    fn virtual_point(&self, point: (i32, i32)) -> (i32, i32) {
        (
            (point.0 as f32 / self.placed.ui.scale).floor() as i32,
            (point.1 as f32 / self.placed.ui.scale).floor() as i32,
        )
    }

    pub(crate) fn contains(&self, point: Option<(i32, i32)>) -> bool {
        point.is_some_and(|point| {
            let point = self.virtual_point(point);
            let origin = self.placed.origin;
            (origin.0..origin.0 + WIDTH).contains(&point.0)
                && (origin.1..origin.1 + HEIGHT).contains(&point.1)
        })
    }

    pub(crate) fn blocks_pointer(&self) -> bool {
        self.owned.iter().any(|&owned| owned)
    }

    fn route(&mut self, event: Mouse, point: Option<(i32, i32)>) {
        let point = point
            .map(|point| self.virtual_point(point))
            .unwrap_or((-1, -1));
        let events = self.manager.mouse(
            &mut [PanelMut {
                panel: &mut self.placed.ui.panel,
                origin: &mut self.placed.origin,
            }],
            event,
            point.0,
            point.1,
        );

        self.download_requested |= self.status == Status::Available
            && events
                .iter()
                .any(|event| event.id == EventId::Click && event.name == BUTTON);
        self.manager.button_sounds.clear();
    }

    pub(crate) fn motion(&mut self, point: Option<(i32, i32)>) {
        self.route(Mouse::Move, point);
    }

    pub(crate) fn button(&mut self, button: usize, down: bool, point: Option<(i32, i32)>) -> bool {
        if button >= self.owned.len() {
            return false;
        }

        if down {
            if !self.contains(point) {
                return false;
            }

            self.owned[button] = true;

            if button == 0 {
                self.route(Mouse::Move, point);
                self.route(Mouse::LeftDown, point);
            }

            return true;
        }

        let owned = std::mem::take(&mut self.owned[button]);

        if owned && button == 0 {
            self.route(Mouse::LeftUp, point);
        }

        owned || self.contains(point)
    }

    pub(crate) fn take_download_requested(&mut self) -> bool {
        std::mem::take(&mut self.download_requested) && self.status == Status::Available
    }

    fn draws(&mut self) -> Vec<PlannedQuad> {
        let interaction =
            self.manager
                .interaction_for(0, &self.placed.ui.panel, self.placed.origin);
        let mut draws = Vec::new();
        panels::append_panel(
            &mut draws,
            &mut self.placed,
            &interaction,
            CAPTIONS,
            true,
            None,
        );
        draws
    }

    pub(crate) fn queue(&mut self, gpu: &render::Gpu, renderer: &mut SceneRenderer) {
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
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        let mut resource = ui::Resource::default();
        resource.fonts.push(formats::ui::guimat::FontDef {
            index: 2,
            face: 0,
            height: 12,
            width_adjust: 0,
            field4: 0,
            bold: false,
        });
        let mut chains = Chains::default();
        chains.chains[0].faces.push(std::sync::Arc::new(
            render::font::Face::parse(crate::fonts::FALLBACK, 0).unwrap(),
        ));
        let mut button = ui::Element::new(ElementType::Button, "Button", BUTTON, &resource);
        button.width = WIDTH;
        button.height = HEIGHT;
        button.msg_receive = true;
        button.layers = vec![
            ui::material::Material {
                font_index: 2,
                ..ui::material::Material::widget_seed()
            };
            3
        ];
        let source = ui::Panel {
            right: WIDTH,
            bottom: HEIGHT,
            movable: false,
            allow_focus: false,
            enter_defaults: false,
            elements: vec![button],
            ..Default::default()
        };
        let mut panel = Self::from_ui(
            PanelUi::from_resource(source, resource, &chains, 1.0),
            Vec::new(),
            Language::Portuguese,
            (800, 600),
        );
        panel.set_status(Status::Available);
        panel
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn center(panel: &Panel) -> Option<(i32, i32)> {
        Some((
            ((panel.placed.origin.0 + WIDTH / 2) as f32 * panel.placed.ui.scale) as i32,
            ((panel.placed.origin.1 + HEIGHT / 2) as f32 * panel.placed.ui.scale) as i32,
        ))
    }

    #[test]
    fn bottom_right_anchor_preserves_edge_margin_and_scaled_hit_target() {
        let mut panel = Panel::for_test();

        for size in [(800, 600), (1920, 1080), (2560, 1440), (3440, 1440)] {
            panel.resize(size);
            let screen = Layout::new(size.0, size.1).virtual_screen();
            assert_eq!(panel.placed.origin.0 + WIDTH + MARGIN, screen[2]);
            assert_eq!(panel.placed.origin.1 + HEIGHT + MARGIN, screen[3]);
            let inside = center(&panel);
            assert!(panel.contains(inside));
            assert!(!panel.contains(None));
            assert!(!panel.contains(Some((size.0 as i32 - 1, size.1 as i32 - 1))));
            assert!(panel.button(0, true, inside));
            assert!(panel.blocks_pointer());
            assert!(panel.button(0, false, inside));
            assert!(!panel.blocks_pointer());
            assert!(panel.take_download_requested());
            assert!(!panel.take_download_requested());
        }
    }

    #[test]
    fn release_outside_or_without_press_never_downloads() {
        let mut panel = Panel::for_test();
        let inside = center(&panel);

        for outside in [None, Some((0, 0))] {
            assert!(panel.button(0, true, inside));
            panel.motion(outside);
            assert!(panel.button(0, false, outside));
            assert!(!panel.take_download_requested());
            assert!(!panel.blocks_pointer());
            assert!(!panel.button(0, true, outside));
            assert!(panel.button(0, false, inside));
            assert!(!panel.take_download_requested());
        }
    }

    #[test]
    fn secondary_buttons_consume_matching_release_without_downloading() {
        let mut panel = Panel::for_test();
        let inside = center(&panel);

        for button in [1, 2] {
            assert!(panel.button(button, true, inside));
            assert!(panel.blocks_pointer());
            assert!(panel.button(button, false, None));
            assert!(!panel.blocks_pointer());
            assert!(!panel.take_download_requested());
        }
    }

    #[test]
    fn cancelling_or_resizing_clears_press_and_download_request() {
        let mut panel = Panel::for_test();
        let inside = center(&panel);
        assert!(panel.button(0, true, inside));
        panel.cancel_input();
        assert!(!panel.blocks_pointer());
        assert!(panel.button(0, false, inside));
        assert!(!panel.take_download_requested());
        assert!(panel.button(0, true, inside));
        assert!(panel.button(0, false, inside));
        panel.resize((1920, 1080));
        assert!(!panel.take_download_requested());
    }

    #[test]
    fn language_changes_caption_without_changing_geometry() {
        let mut panel = Panel::for_test();
        let original = panel.placed.origin;

        for (language, label) in [
            (Language::Portuguese, "Nova versão disponível"),
            (Language::English, "New version available"),
            (Language::Portuguese, "Nova versão disponível"),
        ] {
            panel.set_language(language);
            let text = panel.placed.ui.draw_list(&Interaction::default());
            assert!(text.texts().any(|text| text.text == label));
            assert_eq!(panel.placed.origin, original);
            assert_eq!(panel.placed.ui.panel.width(), WIDTH);
            assert_eq!(panel.placed.ui.panel.height(), HEIGHT);
        }
    }

    #[test]
    fn all_statuses_have_localized_labels_and_only_available_enables_downloads() {
        let mut panel = Panel::for_test();

        for (status, portuguese, english) in [
            (Status::Checking, "Verificando versão…", "Checking updates…"),
            (Status::UpToDate, "Versão atualizada", "Up to date"),
            (
                Status::Available,
                "Nova versão disponível",
                "New version available",
            ),
            (
                Status::Unavailable,
                "Falha ao verificar versão",
                "Update check failed",
            ),
            (
                Status::Development,
                "Versão de desenvolvimento",
                "Development build",
            ),
        ] {
            panel.set_status(status);

            for (language, label) in [
                (Language::Portuguese, portuguese),
                (Language::English, english),
            ] {
                panel.set_language(language);
                let button = &panel.placed.ui.panel.elements[0];
                assert_eq!(button.display, label);
                assert_eq!(button.enabled, status == Status::Available);
                let text = panel.placed.ui.draw_list(&Interaction::default());
                assert!(text.texts().any(|text| text.text == label));
                assert!(text.texts().all(|text| text.color == 0xFFFF_FFFF));
                let inside = center(&panel);

                for button in 0..3 {
                    assert!(panel.button(button, true, inside));
                    assert!(panel.blocks_pointer());
                    assert!(panel.button(button, false, inside));
                    assert!(!panel.blocks_pointer());
                    assert_eq!(
                        panel.take_download_requested(),
                        status == Status::Available && button == 0,
                    );
                }
            }
        }
    }

    #[test]
    fn status_changes_cancel_pending_clicks_and_downloads() {
        let mut panel = Panel::for_test();
        let inside = center(&panel);

        for status in [
            Status::Checking,
            Status::UpToDate,
            Status::Unavailable,
            Status::Development,
        ] {
            panel.set_status(Status::Available);
            assert!(panel.button(0, true, inside));
            panel.set_status(status);
            assert!(!panel.blocks_pointer());
            assert!(panel.button(0, false, inside));
            assert!(!panel.take_download_requested());

            assert!(panel.button(0, true, inside));
            panel.set_status(Status::Available);
            assert!(!panel.blocks_pointer());
            assert!(panel.button(0, false, inside));
            assert!(!panel.take_download_requested());

            assert!(panel.button(0, true, inside));
            assert!(panel.button(0, false, inside));
            panel.set_status(status);
            panel.set_status(Status::Available);
            assert!(!panel.take_download_requested());
        }
    }

    #[test]
    fn unchanged_status_does_not_cancel_a_download_click() {
        let mut panel = Panel::for_test();
        let inside = center(&panel);
        assert!(panel.button(0, true, inside));
        panel.set_status(Status::Available);
        assert!(panel.blocks_pointer());
        assert!(panel.button(0, false, inside));
        assert!(panel.take_download_requested());
    }

    #[test]
    #[ignore = "requires game assets"]
    fn shipped_button_style_draws_and_fits_localized_labels() {
        let game = GameDir::open(std::env::var_os("GAME_DIR").expect("GAME_DIR")).unwrap();
        let chains = crate::fonts::load(&game, None);
        let mut panel = Panel::new(&game, &chains, (800, 600)).unwrap();
        assert_eq!(panel.status, Status::Checking);
        assert!(!panel.placed.ui.panel.elements[0].enabled);
        assert!(!panel.textures.is_empty());

        for (material, alias) in panel.placed.ui.panel.elements[0]
            .layers
            .iter()
            .zip(BUTTON_SKINS)
        {
            let (texture, uv) = panel.placed.ui.resource.alias_rect(alias);
            assert_eq!((material.texture_index, material.uv), (texture, uv));
        }

        for language in [Language::Portuguese, Language::English] {
            panel.set_language(language);

            for status in [
                Status::Checking,
                Status::UpToDate,
                Status::Available,
                Status::Unavailable,
                Status::Development,
            ] {
                panel.set_status(status);

                for size in [(800, 600), (1920, 1080), (2560, 1440)] {
                    panel.resize(size);
                    let draws = panel.draws();
                    assert!(
                        draws
                            .iter()
                            .any(|draw| matches!(draw.texture, TextureKey::GuiMat(_)))
                    );
                    assert!(
                        draws.iter().any(|draw| matches!(
                            draw.texture,
                            TextureKey::Caption(CAPTIONS, _, _)
                        ))
                    );

                    for (_, cached) in panel.placed.ui.cache.iter() {
                        assert!(cached.bitmap.measured <= cached.rect[2] - cached.rect[0]);
                    }

                    let inside = center(&panel);
                    assert!(panel.button(0, true, inside));
                    assert!(panel.button(0, false, inside));
                    assert_eq!(panel.take_download_requested(), status == Status::Available);
                }
            }
        }
    }
}
