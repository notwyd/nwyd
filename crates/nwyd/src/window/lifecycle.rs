use super::Client;
use crate::{
    runtime::lifecycle::{SceneHost, SceneState},
    scene::Scene,
};

impl SceneHost for Client {
    fn state(&mut self) -> SceneState<'_> {
        SceneState {
            scene: &mut self.runtime.scene,
            retained: &mut self.runtime.retained,
            session: &mut self.runtime.net,
            ime: &mut self.runtime.ime,
            pointer: &mut self.runtime.input.pointer_state,
            audio: self.audio.as_mut(),
            launcher_login: self.launcher_login,
            launcher_handoff: self.launcher_credentials.is_some(),
        }
    }

    fn disable_host_ime(&mut self) {
        if let Some(live) = &self.live {
            live.window.set_ime_allowed(false);
        }
    }

    fn describe_scene(&mut self, _next: &Scene) {
        if let Some(panel) = &mut self.runtime.game_settings {
            panel.close();
        }
    }

    fn upload_scene(&mut self) -> bool {
        if let Some(panel) = &mut self.runtime.game_settings
            && let Scene::Field(scene) = &self.runtime.scene
        {
            panel.reset_costumes(scene.entered.wearer.flags);
        }

        if let Some(panel) = &self.runtime.game_settings
            && let Scene::Field(scene) = &mut self.runtime.scene
            && let Some(screens) = &self.screens
            && scene.interface != panel.applied.interface
        {
            super::game_settings::apply_interface(scene, screens.game(), panel.applied.interface);
        }

        if let Some(live) = self.live.as_mut()
            && !live.gpu.is_lost()
        {
            self.runtime.scene.upload(&live.gpu, &mut live.renderer);

            true
        } else {
            false
        }
    }

    fn game_time(&self) -> crate::clock::GameTime {
        self.runtime.clock.game_now()
    }

    fn redraw(&mut self) {
        Client::redraw(self);
    }
}
