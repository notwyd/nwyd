use crate::fonts::Chains;
use crate::frame::{PanelId, PlannedQuad, SceneRenderer, TextureKey};
use crate::ui::{PanelUi, Source};
use anyhow::{Context, Result};
use assets::GameDir;
use ui::draw::Interaction;
use ui::{Carried, Strings};

pub const CARRY_PANEL_ID: PanelId = 0xF1;

pub struct Carry {
    ui: PanelUi,
    labelled: bool,
}

pub fn build(game: &GameDir, strings: &Strings, chains: &Chains, scale: f32) -> Result<Carry> {
    let ui = PanelUi::build(game, super::tooltip::WINDOW_SCRIPT, strings, chains, scale)
        .context("building the carried item's font host")?;

    Ok(Carry {
        ui,
        labelled: false,
    })
}

impl Carry {
    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self {
            ui: PanelUi::from_resource(
                ui::Panel::default(),
                ui::Resource::default(),
                &Chains::default(),
                1.0,
            ),
            labelled: false,
        }
    }

    pub fn frame_draws(
        &mut self,
        carried: Option<&Carried>,
        cursor: (i32, i32),
        captions: bool,
    ) -> Vec<PlannedQuad> {
        self.labelled = false;

        let Some(carried) = carried else {
            return Vec::new();
        };

        let list = ui::draw::carried_draws(carried, cursor);

        let mut out = Vec::new();

        for draw in self.ui.frame_of(&list, (0, 0)) {
            if matches!(draw.source, Source::Caption(..)) {
                self.labelled = true;

                if !captions {
                    continue;
                }
            }

            let texture = match draw.source {
                Source::Texture(slot) => TextureKey::GuiMat(slot),
                Source::ItemIcon(sheet) => TextureKey::ItemIcon(sheet),
                Source::Amul(sheet) => TextureKey::Amul(sheet),
                Source::Caption(owner, run) => TextureKey::Caption(CARRY_PANEL_ID, owner, run),
                Source::NamedIcon(owner) => TextureKey::NamedIcon(CARRY_PANEL_ID, owner),
                Source::Solid => TextureKey::White,
            };

            out.push(PlannedQuad {
                mode: draw.mode(),
                texture,
                quad: draw.quad,
                uv: draw.uv,
            });
        }

        out
    }

    pub fn sync(&mut self, gpu: &render::Gpu, renderer: &mut SceneRenderer, captions: bool) {
        renderer.sync_named_icons(gpu, CARRY_PANEL_ID, &self.ui);

        if captions && self.labelled {
            renderer.sync_captions(gpu, CARRY_PANEL_ID, &self.ui);
        }
    }

    pub fn ui(&self) -> &PanelUi {
        &self.ui
    }

    pub(super) fn for_each_ui(&mut self, visit: &mut impl FnMut(&mut PanelUi)) {
        visit(&mut self.ui);
    }

    pub fn referenced_textures(&mut self) -> Vec<i32> {
        self.ui.referenced_textures(&Interaction::default())
    }
}
