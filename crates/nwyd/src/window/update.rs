use super::Client;
use crate::runtime::{MusicOrder, UpdateHost};
use std::ops::ControlFlow;

impl UpdateHost for Client {
    type Error = anyhow::Error;

    fn sample_time(&mut self) -> crate::clock::GameTime {
        self.tick()
    }

    fn camera_input(&mut self) {
        self.camera_frame();
    }

    fn poll_status(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        self.poll_release_update();

        if self.runtime.scene.poll_status() {
            self.redraw();
        }
    }

    fn poll_network(&mut self) -> ControlFlow<()> {
        if self.poll_net() {
            ControlFlow::Break(())
        } else {
            ControlFlow::Continue(())
        }
    }

    fn tick_scene(&mut self) -> ControlFlow<()> {
        if let Some(transition) = crate::runtime::outbound::tick(
            &mut self.runtime.scene,
            &mut self.runtime.net,
            &self.runtime.clock,
        ) {
            if self.transition(transition) {
                return ControlFlow::Break(());
            }

            self.redraw();
        }

        if matches!(self.runtime.scene, crate::scene::Scene::Field(_)) {
            self.runtime
                .net
                .advance_field_clock(self.runtime.clock.game_now());
        }

        ControlFlow::Continue(())
    }

    fn music_order(&mut self) -> MusicOrder {
        if self.runtime.scene.music_follows_outbox() {
            MusicOrder::AfterOutbox
        } else {
            MusicOrder::BeforeOutbox
        }
    }

    fn flush_music(&mut self) {
        Client::flush_music(self);
    }

    fn stream_zones(&mut self) {
        self.poll_background_resources();

        Client::stream_zones(self);
    }

    fn flush_outbox(&mut self) {
        Client::flush_outbox(self);
    }

    fn flush_browser(&mut self) {
        self.flush_notice_browser();
    }

    fn save_quick_slots(&mut self) {
        self.flush_quick_slot_save();
    }

    fn sync_ime(&mut self) {
        Client::sync_ime(self);
    }
}

impl Client {
    fn poll_background_resources(&mut self) {
        let crate::scene::Scene::Field(scene) = &mut self.runtime.scene else {
            return;
        };

        let Some(screens) = &self.screens else {
            return;
        };

        scene.world.objects.enable_background_held(screens.game());

        scene
            .world
            .actors
            .enable_background_textures(screens.game());

        let textures = scene.world.actors.poll_background_textures();

        let meshes = scene.world.objects.poll_texture_cache(
            screens.game(),
            &scene.world.actors,
            &scene.drops,
            &scene.gates,
        );

        let held = scene
            .world
            .objects
            .poll_held_resources(&mut scene.world.actors, scene.player);

        if !held && !textures && !meshes {
            return;
        }

        if let Some(live) = self.live.as_mut().filter(|live| !live.gpu.is_lost()) {
            live.renderer.upload_world(&live.gpu, &scene.world);
        }

        self.redraw();
    }
}
