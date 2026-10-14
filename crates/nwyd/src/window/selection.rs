use super::Client;
use crate::runtime::selection::{self, Packet, Presentation, Resources, State};

impl Client {
    pub(super) fn apply_selection_packet(&mut self, packet: Packet) {
        let resources = self.screens.as_ref().map(|screens| Resources {
            game: screens.game(),
            items: screens.items(),
        });

        let presentation = selection::apply(
            State {
                scene: &mut self.runtime.scene,
                session: &mut self.runtime.net,
                retained: &mut self.runtime.retained,
                rng: &mut self.runtime.rng,
                resources,
                now_ms: self.runtime.clock.game_now(),
            },
            packet,
        );

        if presentation == Presentation::Redraw {
            self.redraw();
        }
    }
}
