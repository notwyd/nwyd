use super::Client;
use crate::{
    scene::Scene,
    update_panel::{self, Panel},
    updates::{Checker, Status},
};

fn dialogs_visible(
    boxes: &crate::scene::msgbox::Boxes,
    panel: &crate::scene::msgbox::BoxPanel,
) -> bool {
    boxes.topmost().is_some() || panel.visible().next().is_some()
}

#[derive(Default)]
pub(super) struct State {
    checker: Option<Checker>,
    pub(super) panel: Option<Panel>,
    download_url: Option<String>,
    status: Option<Status>,
    prepared: bool,
}

impl Client {
    pub(crate) fn with_update_check(mut self) -> Self {
        self.updates.checker = Some(Checker::start());
        self
    }

    pub(super) fn poll_release_update(&mut self) {
        if self.updates.prepared && self.updates.panel.is_none() {
            return;
        }

        let Some(checker) = &mut self.updates.checker else {
            return;
        };

        let status = checker.poll();

        if self.updates.panel.is_some() && self.updates.status.as_ref() == Some(status) {
            return;
        }

        let status = status.clone();

        if !self.updates.prepared {
            let (Some(screens), Some(settings)) = (&self.screens, &self.runtime.game_settings)
            else {
                return;
            };

            self.updates.prepared = true;

            match Panel::new(screens.game(), &settings.chains, self.settings.backbuffer) {
                Ok(panel) => self.updates.panel = Some(panel),
                Err(error) => {
                    tracing::warn!(% error, "NWYD: update notification unavailable");
                    return;
                }
            }
        }

        self.set_release_status(status);
    }

    fn set_release_status(&mut self, status: Status) {
        let panel_status = match &status {
            Status::Checking => update_panel::Status::Checking,
            Status::UpToDate => update_panel::Status::UpToDate,
            Status::Available(_) => update_panel::Status::Available,
            Status::Unavailable => update_panel::Status::Unavailable,
            Status::Development => update_panel::Status::Development,
        };

        self.updates.download_url = match &status {
            Status::Available(update) => Some(update.download_url.clone()),
            _ => None,
        };

        if let Some(panel) = &mut self.updates.panel {
            panel.set_status(panel_status);
        }

        self.updates.status = Some(status);
        self.update_notice_motion(self.pointer_in_backbuffer());
        self.redraw();
    }

    pub(super) fn update_notice_visible(&self) -> bool {
        if self.updates.panel.is_none()
            || self.settings_visible()
            || self.runtime.scene.legacy_modal()
            || self.nell_browser_visible()
            || self.destination_pending()
        {
            return false;
        }

        match &self.runtime.scene {
            Scene::LoginPanels(scene) => !dialogs_visible(&scene.boxes, &scene.msgbox),
            Scene::ServerSelect(scene) => {
                !dialogs_visible(&scene.panels.boxes, &scene.panels.msgbox)
            }
            Scene::SelectChar(scene) => {
                !scene.name_dialog.visible && !dialogs_visible(&scene.boxes, &scene.panels.msgbox)
            }
            Scene::Field(scene) => scene
                .panels
                .as_ref()
                .is_none_or(|panels| !dialogs_visible(&panels.boxes, &panels.msgbox)),
            _ => true,
        }
    }

    pub(super) fn update_notice_blocks_pointer(&self, point: Option<(i32, i32)>) -> bool {
        self.update_notice_visible()
            && self
                .updates
                .panel
                .as_ref()
                .is_some_and(|panel| panel.contains(point) || panel.blocks_pointer())
    }

    pub(super) fn update_notice_motion(&mut self, point: Option<(i32, i32)>) -> bool {
        let visible = self.update_notice_visible();

        let Some(panel) = &mut self.updates.panel else {
            return false;
        };

        panel.motion(if visible { point } else { None });

        let blocked = visible && (panel.contains(point) || panel.blocks_pointer());

        if blocked {
            self.runtime.scene.cancel_pointer_input();
            self.runtime.input.mouse = crate::scene::camera::MouseInput {
                alt: self.runtime.input.mods.alt,
                ..Default::default()
            };

            let choice = self.runtime.input.pointer_state.on_move(
                self.settings.pointers.mode,
                None,
                point.is_some(),
            );

            self.apply_pointer(choice);
            self.redraw();
        }

        blocked
    }

    pub(super) fn update_notice_button(
        &mut self,
        button: usize,
        down: bool,
        point: Option<(i32, i32)>,
        open: impl FnOnce(&str) -> std::io::Result<()>,
    ) -> bool {
        let visible = self.update_notice_visible();

        if down && !visible {
            return false;
        }

        let Some(panel) = &mut self.updates.panel else {
            return false;
        };

        if !panel.button(button, down, if visible { point } else { None }) {
            return false;
        }

        let download = panel.take_download_requested();

        self.runtime.scene.cancel_pointer_input();
        self.runtime.input.mouse = crate::scene::camera::MouseInput {
            alt: self.runtime.input.mods.alt,
            ..Default::default()
        };

        if download && let Some(url) = &self.updates.download_url {
            if let Err(error) = open(url) {
                tracing::warn!(% error, "NWYD: could not open update download");
            }
        }

        self.redraw();
        true
    }
}

#[cfg(test)]
mod tests;
