use super::Client;
use crate::runtime::session_packets::{Host, State};

impl Host for Client {
    fn state(&mut self) -> State<'_> {
        State {
            scene: &mut self.runtime.scene,
            session: &mut self.runtime.net,
        }
    }

    fn sync_ime(&mut self) {
        Client::sync_ime(self);
    }

    fn redraw(&mut self) {
        Client::redraw(self);
    }
}
