use super::Client;
use crate::{runtime::combat_packets::Host, scene::Scene};

impl Host for Client {
    fn scene(&mut self) -> &mut Scene {
        &mut self.runtime.scene
    }

    fn refresh_screens(&mut self) {
        Client::refresh_screens(self);
    }

    fn redraw(&mut self) {
        Client::redraw(self);
    }
}
