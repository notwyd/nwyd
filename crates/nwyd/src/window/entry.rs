use super::Client;
use crate::{
    runtime::entry::{self, Packet, Prepared, State},
    scene::Scene,
};

impl Client {
    pub(super) fn apply_entry_packet(&mut self, packet: Packet) {
        let prepared = entry::prepare(
            State {
                scene: &mut self.runtime.scene,
                session: &mut self.runtime.net,
                retained: &mut self.runtime.retained,
                screens: self.screens.as_deref(),
                picked: self.picked.as_ref(),
            },
            packet,
        );

        let scene = match prepared {
            Some(Prepared::Selection(scene)) => scene,
            Some(Prepared::World(mut scene)) => {
                if let Scene::Field(field) = &mut scene {
                    field.marks.start(crate::scene::field::MARK_TIMEOUT);
                }

                scene
            }
            None => return,
        };

        self.show(scene);
    }
}
