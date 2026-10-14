use super::Client;
use crate::{
    runtime::departure::{Host, State},
    scene::Scene,
};
use protocol::server::Redirect;

impl Host for Client {
    fn state(&mut self) -> State<'_> {
        State {
            scene: &mut self.runtime.scene,
            session: &mut self.runtime.net,
            retained: &mut self.runtime.retained,
            picked: self.picked.as_mut(),
            screens: self.screens.as_deref(),
            launcher_login: self.launcher_login,
        }
    }

    fn game_time(&self) -> u32 {
        self.runtime.clock.game_time()
    }

    fn reconnect(&mut self, address: &str, ticket: &Redirect) -> Result<(), String> {
        self.runtime
            .net
            .redirect(address, ticket, self.runtime.clock.wire_tick())
    }

    fn disconnect(&mut self) {
        self.runtime.net.disconnect();
    }

    fn show(&mut self, scene: Scene) {
        Client::show(self, scene);
    }

    fn sync_ime(&mut self) {
        Client::sync_ime(self);
    }

    fn redraw(&mut self) {
        Client::redraw(self);
    }

    fn show_message(&mut self, record: usize) {
        Client::show_message(self, record);
    }
}
