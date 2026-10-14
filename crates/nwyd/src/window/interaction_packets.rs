use super::Client;
use crate::{runtime::interaction_packets::Host, scene::Scene};

impl Host for Client {
    fn scene(&mut self) -> &mut Scene {
        &mut self.runtime.scene
    }

    fn redraw(&mut self) {
        Client::redraw(self);
    }
}
