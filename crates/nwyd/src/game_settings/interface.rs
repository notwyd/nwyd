use super::*;
use crate::scene::{actors::Doll, field::FieldScene};
use formats::mesh::appearance;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Interface {
    #[serde(skip_serializing)]
    pub hide_helmet: bool,
    #[serde(skip_serializing)]
    pub hide_costume: bool,
    #[serde(skip_serializing)]
    pub hide_mount_costume: bool,
    pub cape_refinement: bool,
    pub hide_hud: bool,
    pub zoom: bool,
    pub evok_size: u8,
}

impl Default for Interface {
    fn default() -> Self {
        Self {
            hide_helmet: false,
            hide_costume: false,
            hide_mount_costume: false,
            cape_refinement: false,
            hide_hud: false,
            zoom: false,
            evok_size: 100,
        }
    }
}

impl Interface {
    pub(crate) fn appearance(self) -> bool {
        self.hide_helmet
    }

    pub(crate) fn with_costume_flags(mut self, flags: u8) -> Self {
        self.hide_helmet = flags & 4 != 0;

        self.hide_costume = flags & appearance::FLAG_NO_COSTUME != 0;

        self.hide_mount_costume = flags & appearance::FLAG_MOUNT_AS_WORN != 0;

        self
    }

    pub(crate) fn request_costume_changes(self, previous: Self, scene: &mut FieldScene) {
        if self.hide_helmet != previous.hide_helmet {
            scene.outbox.push(crate::scene::chat::command_packet(
                scene.entered.client_id,
                "helmohidecmd",
            ));
        }

        for (changed, mount) in [
            (self.hide_costume != previous.hide_costume, false),
            (self.hide_mount_costume != previous.hide_mount_costume, true),
        ] {
            if changed {
                scene
                    .outbox
                    .push(crate::scene::chat::costume_command_packet(
                        scene.entered.client_id,
                        mount,
                    ));
            }
        }
    }

    pub(crate) fn camera_max(self, low_ceiling: bool) -> f32 {
        if low_ceiling {
            crate::scene::camera::VIEW_DIST_NEAR
        } else {
            crate::scene::camera::FIELD_MAX_DIST * if self.zoom { 2.0 } else { 1.0 }
        }
    }

    pub(crate) fn apply(self, scene: &mut FieldScene) {
        let previous = scene.interface;

        if previous.hide_hud != self.hide_hud
            && let Some(hud) = &mut scene.hud
        {
            hud.bar.ui.panel.visible = !self.hide_hud;

            hud.plate.ui.panel.visible = !self.hide_hud;

            if let Some(panels) = &mut scene.panels {
                crate::scene::chat::set_hud_visible(panels, hud, !self.hide_hud);
            }

            if self.hide_hud {
                hud.target.ui.panel.visible = false;
            }
        }

        if previous.zoom != self.zoom {
            scene.camera.max_dist = self.camera_max(scene.low_ceiling);

            scene.camera.dist = scene.camera.dist.min(scene.camera.max_dist);

            scene.camera.dist_goal = scene.camera.dist_goal.min(scene.camera.max_dist);
        }

        scene.world.actors.set_evok_size(self.evok_size);

        scene.world.actors.cape_refinement = self.cape_refinement;

        scene.interface = self.with_costume_flags(scene.entered.wearer.flags);
    }

    pub(crate) fn project(_scene: &FieldScene, _doll: &mut Doll) {}
}
