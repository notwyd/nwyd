use crate::fonts::Chains;
use crate::frame::{PanelId, PlannedQuad, SceneRenderer, TextureKey};
use crate::ui::{PanelUi, Source};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::ui::{guimat, pane::ElementType};
use render::device::Gpu;
use ui::input::Target;
use ui::{Balloon, Panel, Resource};

pub const RESOURCE: &[u8] = br"UI\balloon.guimat";

pub const PANEL_ID: PanelId = 0xF2;

pub const HUD_OFFSET: (i16, i16) = (-5, 1);

pub struct Tip {
    ui: PanelUi,
    balloon: Balloon,
    pub raised_by: Option<Target>,
}

pub fn build(game: &GameDir, chains: &Chains, scale: f32) -> Result<Tip> {
    let raw = game.read(RESOURCE).context("reading UI/balloon.guimat")?;

    let parsed = guimat::parse(&raw).context("parsing UI/balloon.guimat")?;

    let resource = Resource::build(&parsed);

    let balloon = Balloon::new(&resource);

    let mut panel = Panel::default();

    panel.resource_path = String::from_utf8_lossy(RESOURCE).into_owned();

    let mut ui = PanelUi::from_resource(panel, resource, chains, scale);

    ui.set_icon_source(game);

    Ok(Tip {
        ui,
        balloon,
        raised_by: None,
    })
}

impl Tip {
    #[cfg(test)]

    pub(crate) fn for_test() -> Self {
        let resource = Resource::default();

        Self {
            balloon: Balloon::new(&resource),
            ui: PanelUi::from_resource(Panel::default(), resource, &Chains::default(), 1.0),
            raised_by: None,
        }
    }

    pub fn set_font_color(&mut self, argb: u32) {
        self.balloon.set_font_color(argb);
    }

    pub fn show(
        &mut self,
        owner: Target,
        text: &str,
        source: [i32; 4],
        screen: [i32; 4],
        offset: Option<(i16, i16)>,
    ) -> bool {
        let shown = {
            let (ui, balloon) = (&mut self.ui, &mut self.balloon);

            let mut measure = ui.measure();

            balloon.show(text, source, screen, offset, &mut measure)
        };

        self.raised_by = shown.then_some(owner);

        shown
    }

    pub fn hide(&mut self) {
        self.balloon.hide();

        self.raised_by = None;
    }

    pub fn visible(&self) -> bool {
        self.balloon.is_visible()
    }

    pub fn text(&self) -> &str {
        self.balloon.text()
    }

    pub fn rect(&self) -> [i32; 4] {
        self.balloon.rect()
    }

    pub fn set_scale(&mut self, scale: f32) {
        self.ui.scale = scale;
    }

    pub fn referenced_textures(&self) -> Vec<i32> {
        self.ui
            .resource
            .layers(ElementType::BalloonBox)
            .filter(|layer| layer.layer == 0)
            .map(|layer| layer.material.texture_index)
            .collect()
    }

    pub fn ui(&self) -> &PanelUi {
        &self.ui
    }

    pub(super) fn for_each_ui(&mut self, visit: &mut impl FnMut(&mut PanelUi)) {
        visit(&mut self.ui);
    }

    pub fn frame_draws(&mut self, captions: bool) -> Vec<PlannedQuad> {
        let Some(target) = self.raised_by else {
            return Vec::new();
        };

        let list = self.balloon.draw_list(target.owner);

        let origin = self.balloon.position();

        self.ui
            .frame_of(&list, origin)
            .into_iter()
            .filter_map(|draw| {
                if !captions && matches!(draw.source, Source::Caption(..)) {
                    return None;
                }

                let texture = match draw.source {
                    Source::Texture(slot) => TextureKey::HoverGuiMat(slot),
                    Source::Caption(owner, run) => TextureKey::Caption(PANEL_ID, owner, run),
                    Source::NamedIcon(owner) => TextureKey::NamedIcon(PANEL_ID, owner),
                    Source::Solid => TextureKey::White,
                    Source::ItemIcon(sheet) => TextureKey::ItemIcon(sheet),
                    Source::Amul(sheet) => TextureKey::Amul(sheet),
                };

                Some(PlannedQuad {
                    mode: draw.mode(),
                    texture,
                    quad: draw.quad,
                    uv: draw.uv,
                })
            })
            .collect()
    }

    pub fn sync(&mut self, gpu: &Gpu, renderer: &mut SceneRenderer, captions: bool) {
        if self.visible() {
            renderer.sync_named_icons(gpu, PANEL_ID, &self.ui);
        }

        if captions && self.visible() {
            renderer.sync_captions(gpu, PANEL_ID, &self.ui);
        }
    }
}
