use super::Client;
use crate::runtime::object_packets::{Host, State};

impl Host for Client {
    fn state(&mut self) -> State<'_> {
        State {
            scene: &mut self.runtime.scene,
            game: self.screens.as_ref().map(|screens| screens.game()),
        }
    }

    fn refresh_screens(&mut self) {
        Client::refresh_screens(self);
    }

    fn redraw(&mut self) {
        Client::redraw(self);
    }
}
