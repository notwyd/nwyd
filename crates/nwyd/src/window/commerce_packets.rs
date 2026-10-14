use super::Client;
use crate::runtime::commerce_packets::{Host, State};

impl Host for Client {
    fn state(&mut self) -> State<'_> {
        State {
            scene: &mut self.runtime.scene,
            prices: &mut self.runtime.retained.item_prices,
            game: self.screens.as_ref().map(|screens| screens.game()),
        }
    }

    fn load_panel_icons(&mut self) {
        Client::load_panel_icons(self);
    }

    fn redraw(&mut self) {
        Client::redraw(self);
    }
}
