use super::Client;
use crate::{
    game_settings::{Panel, Preferences},
    scene::{Scene, lod::EffectOptions},
};

impl Client {
    pub fn with_game_settings(
        mut self,
        preferences: Preferences,
        path: std::path::PathBuf,
        game: &assets::GameDir,
    ) -> Self {
        let mut initial = EffectOptions::default();

        initial.detail.effects = self.render_quality >= 2;

        let options = preferences.effect_options(initial);

        self.runtime.scene.set_effect_options(options);

        self.runtime.retained.effect_options = Some(options);

        if let Scene::Field(scene) = &mut self.runtime.scene {
            apply_interface(scene, game, preferences.interface);
        }

        match Panel::new(preferences, path, game, self.settings.backbuffer) {
            Ok(panel) => self.runtime.game_settings = Some(panel),
            Err(error) => tracing::warn!(% error, "NWYD: settings panel unavailable"),
        }

        self.runtime.sync_costume_settings();

        self
    }

    pub(super) fn settings_visible(&self) -> bool {
        self.runtime
            .game_settings
            .as_ref()
            .is_some_and(|p| p.visible)
    }

    pub(super) fn settings_blocks_pointer(&self) -> bool {
        self.runtime
            .game_settings
            .as_ref()
            .is_some_and(|p| p.blocks_pointer())
    }

    pub(super) fn open_requested_game_settings(&mut self) {
        let requested = match &mut self.runtime.scene {
            Scene::LoginPanels(scene) => std::mem::take(&mut scene.settings_requested),
            Scene::ServerSelect(scene) => std::mem::take(&mut scene.panels.settings_requested),
            Scene::Field(scene) => scene
                .panels
                .as_mut()
                .is_some_and(|p| std::mem::take(&mut p.settings_requested)),
            _ => false,
        };

        if requested {
            self.open_game_settings();
        }
    }

    pub(super) fn open_game_settings(&mut self) {
        if self.settings_visible() {
            return;
        }

        self.runtime.sync_costume_settings();

        let options = self
            .runtime
            .scene
            .effect_options()
            .or(self.runtime.retained.effect_options)
            .unwrap_or_default();

        let Some(panel) = &mut self.runtime.game_settings else {
            return;
        };

        if let Scene::Field(scene) = &self.runtime.scene {
            panel.reset_costumes(scene.entered.wearer.flags);
        }

        panel.open(options);

        self.runtime.scene.cancel_pointer_input();

        self.runtime.input.mouse = Default::default();

        self.runtime.ime.cancel(&mut self.runtime.scene);

        if let Some(live) = &self.live {
            live.window.set_ime_allowed(false);
        }

        self.redraw();
    }

    pub(super) fn apply_game_settings(&mut self) {
        let Some(panel) = &mut self.runtime.game_settings else {
            return;
        };

        let sounds = panel.take_sounds();

        if let Scene::Field(scene) = &mut self.runtime.scene {
            scene.unconditional_sounds.extend(
                sounds
                    .into_iter()
                    .map(crate::scene::audio::UnconditionalSound::new),
            );
        } else if let Some(audio) = &self.audio {
            for slot in sounds {
                audio.play_unconditional(crate::scene::audio::UnconditionalSound::new(slot));
            }
        }

        if !std::mem::take(&mut panel.apply_requested) {
            return;
        }

        let previous = panel.applied;

        let next = match self.commit_game_settings() {
            Ok(next) => next,
            Err(error) => {
                let panel = self.runtime.game_settings.as_ref().unwrap();

                tracing::warn!(
                    % error, path = % panel.path.display(), "NWYD: applying settings failed"
                );

                let portuguese = cfg!(target_arch = "wasm32")
                    || self.screens.as_ref().is_none_or(|screens| {
                        screens.game().language() == formats::config::Language::Portuguese
                    });

                let message = if portuguese {
                    "Não foi possível aplicar as configurações. As configurações anteriores foram mantidas."
                } else {
                    "Could not apply settings. Your previous settings have been kept."
                };

                #[cfg(target_arch = "wasm32")]
                crate::web::report_notice(message);

                #[cfg(not(target_arch = "wasm32"))]
                if self.error_dialogs {
                    let _ = rfd::MessageDialog::new()
                        .set_title("NWYD")
                        .set_description(format!(
                            "{message}\n\n{}\n\n{error:#}",
                            panel.path.display()
                        ))
                        .set_level(rfd::MessageLevel::Error)
                        .set_buttons(rfd::MessageButtons::Ok)
                        .show();
                }

                return;
            }
        };

        if let Scene::Field(scene) = &mut self.runtime.scene {
            next.interface
                .request_costume_changes(previous.interface, scene);
        }

        if let Some(audio) = &self.audio {
            audio.set_volumes(next.sound, next.music);
        }

        if previous.interface != next.interface {
            if let Scene::Field(scene) = &mut self.runtime.scene
                && let Some(screens) = &self.screens
            {
                apply_interface(scene, screens.game(), next.interface);
            }

            if let Some(live) = &mut self.live
                && !live.gpu.is_lost()
            {
                self.runtime.scene.upload(&live.gpu, &mut live.renderer);
            }
        }

        self.runtime.apply_game_preferences(next);

        self.settings.fullscreen = next.fullscreen;

        let size = (next.resolution[0], next.resolution[1]);

        if size != self.settings.backbuffer {
            self.resize_game(size);
        }

        if previous.vsync != next.vsync {
            self.settings.present_mode = next.present_mode();
        }

        if let Some(live) = &mut self.live {
            if previous.fullscreen != next.fullscreen {
                live.window.set_fullscreen(
                    next.fullscreen
                        .then_some(winit::window::Fullscreen::Borderless(None)),
                );

                #[cfg(not(target_arch = "wasm32"))]
                if !next.fullscreen {
                    let _ = live
                        .window
                        .request_inner_size(winit::dpi::PhysicalSize::new(
                            self.settings.backbuffer.0,
                            self.settings.backbuffer.1,
                        ));
                }
            }

            if previous.vsync != next.vsync {
                let capabilities = live.surface.get_capabilities(&live.gpu.adapter);

                live.config.present_mode =
                    super::surface_present_mode(self.settings.present_mode, &capabilities);

                tracing::info!(
                    requested = ? self.settings.present_mode, resolved = ? live.config
                    .present_mode, available = ? capabilities.present_modes,
                    "NWYD: presentation mode changed"
                );

                live.stale = true;
            }
        }

        tracing::info!(
            resolution = ? next.resolution, "NWYD: game settings applied and saved"
        );
    }

    fn commit_game_settings(&mut self) -> anyhow::Result<Preferences> {
        use anyhow::Context;

        let panel = self
            .runtime
            .game_settings
            .as_ref()
            .context("settings panel unavailable")?;

        let language = panel.draft.language;

        let change = if language != panel.applied.language {
            let prepare = || -> anyhow::Result<_> {
                let screens = self.screens.as_ref().context("game files unavailable")?;

                let resources = crate::language::Resources::load(screens.game(), language)?;

                let scene = self
                    .runtime
                    .scene
                    .prepare_language(&resources.game, &resources.strings)?;

                Ok((resources, scene))
            };

            match prepare() {
                Ok(change) => Some(change),
                Err(error) => {
                    self.runtime.game_settings.as_mut().unwrap().discard_draft();

                    return Err(error);
                }
            }
        } else {
            None
        };

        let panel = self.runtime.game_settings.as_mut().unwrap();

        let next = panel.save_draft()?;

        if let Some((resources, update)) = change {
            self.runtime.scene.apply_language(update);

            *self.screens.as_mut().unwrap().game_mut() = resources.game.clone();

            panel.apply_language(&resources.game, &resources.strings);
        }

        Ok(next)
    }

    fn resize_game(&mut self, size: (u32, u32)) {
        if let Some(panel) = &self.runtime.game_settings {
            self.runtime.scene.resize(&panel.chains, size);
        }

        if let Some(screens) = &mut self.screens {
            screens.set_backbuffer(size);
        }

        if let Some(panel) = &mut self.runtime.game_settings {
            panel.resize(size);
        }

        self.settings.backbuffer = size;

        self.fit = None;

        self.fit_warned = false;

        self.resize_attempts = super::RESIZE_ATTEMPTS;

        if let Some(live) = &mut self.live {
            if !live.gpu.is_lost() {
                live.renderer = super::upload_scene_renderer(
                    &live.gpu,
                    &self.runtime.scene,
                    self.overlay.as_ref(),
                );

                live.back_buffer = render::Offscreen::new(&live.gpu, size.0, size.1);

                live.source = live.blitter.source(&live.gpu, &live.back_buffer);
            }

            #[cfg(not(target_arch = "wasm32"))]
            if !self.settings.fullscreen {
                let _ = live
                    .window
                    .request_inner_size(winit::dpi::PhysicalSize::new(size.0, size.1));
            }
        }

        self.last_frame_at = None;

        self.redraw();
    }
}

#[cfg(test)]
mod tests;

pub(super) fn apply_interface(
    scene: &mut crate::scene::field::FieldScene,
    game: &assets::GameDir,
    preferences: crate::game_settings::Interface,
) {
    let redress = scene.interface.appearance() != preferences.appearance();

    preferences.apply(scene);

    if redress
        && let Some(index) = scene
            .world
            .actors
            .actors
            .iter()
            .position(|actor| actor.entity == scene.entered.client_id)
        && let Err(error) = crate::scene::live::redress_player(scene, game, index)
    {
        tracing::warn!(% error, "NWYD: refreshing interface appearance failed");
    }
}
