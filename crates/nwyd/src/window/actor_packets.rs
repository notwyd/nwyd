use super::Client;
use crate::runtime::actor_packets::{Host, State};

impl Host for Client {
    fn state(&mut self) -> State<'_> {
        State {
            scene: &mut self.runtime.scene,
            game: self.screens.as_ref().map(|screens| screens.game()),
        }
    }

    fn refresh_party(&mut self) {
        Client::refresh_party(self);
    }

    fn redraw(&mut self) {
        Client::redraw(self);
    }
}
