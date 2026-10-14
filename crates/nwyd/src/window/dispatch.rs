use super::Client;
use crate::{
    net::Transition,
    runtime::dispatch::{BatchEffects, DispatchHost, PacketFlow},
    scene::Scene,
};
use protocol::transport::Event;

impl DispatchHost for Client {
    fn flush_outbound(&mut self) {
        self.flush_pending_outbox();
    }

    fn receive_event(&mut self, event: Event) -> Option<Transition> {
        self.runtime
            .net
            .receive_event(&mut self.runtime.clock, event)
    }

    fn before_transition(&mut self) {
        if let Scene::Field(scene) = &mut self.runtime.scene {
            if let Some(screens) = &self.screens {
                scene.world.objects.enable_background_held(screens.game());

                scene
                    .world
                    .actors
                    .enable_background_textures(screens.game());
            }

            crate::scene::store::finish_panel_close(scene);

            crate::scene::trade::finish_panel_close(scene);
        }
    }

    fn apply_transition(
        &mut self,
        transition: Transition,
        effects: &mut BatchEffects,
    ) -> PacketFlow {
        self.apply_packet(transition, effects)
    }

    fn upload(&mut self) {
        if let Some(live) = self.live.as_mut()
            && !live.gpu.is_lost()
        {
            self.runtime.scene.upload(&live.gpu, &mut live.renderer);
        }
    }
}
