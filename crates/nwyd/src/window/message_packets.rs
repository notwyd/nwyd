use super::Client;
use crate::{runtime::message_packets::Host, scene::Scene};

impl Host for Client {
    fn scene(&mut self) -> &mut Scene {
        &mut self.runtime.scene
    }

    fn launcher_login(&self) -> bool {
        self.launcher_login
    }

    fn hide_item_announcements(&self) -> bool {
        self.runtime
            .game_settings
            .as_ref()
            .is_some_and(|panel| panel.applied.interface.hide_item_announcements)
    }

    fn hide_chat_item_announcement(&self, line: &protocol::server::ChatLine) -> bool {
        self.hide_item_announcements() && self.runtime.item_announcements.matches(line)
    }

    fn channel(&self) -> i32 {
        self.picked.as_ref().map_or(0, |p| p.channel as i32)
    }

    fn log_message(&mut self, record: usize) {
        Client::show_message(self, record);
    }

    fn retain_memo(&mut self, body: &str) {
        self.runtime
            .retained
            .memo_inbox
            .receive(body, &crate::clock::Calendar::local());
    }

    fn sync_ime(&mut self) {
        Client::sync_ime(self);
    }

    fn redraw(&mut self) {
        Client::redraw(self);
    }
}

#[cfg(test)]
mod tests;
