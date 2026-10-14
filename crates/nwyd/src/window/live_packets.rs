use super::Client;
use crate::runtime::live_packets::{Host, State};

impl Host for Client {
    fn state(&mut self) -> State<'_> {
        State {
            scene: &mut self.runtime.scene,
            warehouse: self.runtime.net.warehouse.as_deref_mut(),
            game: self.screens.as_ref().map(|screens| screens.game()),
        }
    }

    fn broadcast_panels(&mut self) {
        Client::broadcast_panels(self);
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
