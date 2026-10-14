use super::Client;
use crate::{runtime::environment_packets::Host, scene::Scene};

impl Host for Client {
    fn scene(&mut self) -> &mut Scene {
        &mut self.runtime.scene
    }

    fn game_time(&mut self) -> crate::clock::GameTime {
        self.runtime.clock.game_now()
    }

    fn log_message(&mut self, record: usize) {
        Client::show_message(self, record);
    }

    fn redraw(&mut self) {
        Client::redraw(self);
    }
}
