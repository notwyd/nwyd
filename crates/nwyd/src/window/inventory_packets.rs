use super::Client;
use crate::{runtime::inventory_packets::Host, scene::Scene};

impl Host for Client {
    fn scene(&mut self) -> &mut Scene {
        &mut self.runtime.scene
    }

    fn redress_player(&mut self) -> Option<anyhow::Result<bool>> {
        let game = self.screens.as_ref()?.game();

        self.runtime.scene.redress_player(game)
    }

    fn refresh_screens(&mut self) {
        Client::refresh_screens(self);
    }

    fn load_panel_icons(&mut self) {
        Client::load_panel_icons(self);
    }

    fn redraw(&mut self) {
        Client::redraw(self);
    }
}
