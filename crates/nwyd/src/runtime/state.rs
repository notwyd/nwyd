use super::lifecycle::Retained;
use crate::{
    clock::Clock,
    net::Session,
    scene::{Scene, rng::Lcg},
};
use ui::text_entry::Mods;

#[derive(Default)]
pub struct InputState {
    pub(crate) mods: Mods,
    pub(crate) pointer_state: crate::scene::cursor::PointerState,
    pub(crate) mouse: crate::scene::camera::MouseInput,
    pub(crate) invert_mouse: bool,
}

pub struct RuntimeState {
    pub(crate) game_settings: Option<crate::game_settings::Panel>,
    pub(crate) scene: Scene,
    pub(crate) retained: Retained,
    pub(crate) clock: Clock,
    pub(crate) net: Session,
    pub(crate) rng: Lcg,
    pub(crate) ime: crate::scene::ime::Bridge,
    pub(crate) input: InputState,
}

impl RuntimeState {
    pub(crate) fn sync_costume_settings(&mut self) {
        if let Scene::Field(scene) = &mut self.scene {
            let flags = scene.entered.wearer.flags;

            scene.interface = scene.interface.with_costume_flags(flags);

            if let Some(panel) = &mut self.game_settings {
                panel.sync_costumes(flags);
            }
        }
    }

    pub(crate) fn apply_game_preferences(
        &mut self,
        preferences: crate::game_settings::Preferences,
    ) {
        self.input.invert_mouse = preferences.invert_mouse;

        if let Scene::Field(scene) = &mut self.scene {
            preferences.interface.apply(scene);
        }

        let effects = preferences.effect_options(
            self.scene
                .effect_options()
                .or(self.retained.effect_options)
                .unwrap_or_default(),
        );

        self.scene.set_effect_options(effects);

        self.retained.effect_options = Some(effects);

        self.scene.set_view_distance(preferences.view_distance);

        self.retained.view_distance = preferences.view_distance;
    }

    pub fn new(scene: Scene, clock: Clock, rng: Lcg) -> Self {
        let retained = Retained::new(&scene);

        Self {
            game_settings: None,
            scene,
            clock,
            rng,
            retained,
            net: Session::default(),
            ime: Default::default(),
            input: Default::default(),
        }
    }

    pub fn sample_time(
        &mut self,
        lod: crate::scene::lod::Inputs,
        quality: i16,
    ) -> crate::clock::GameTime {
        let sample = self.clock.sample();

        self.scene.set_render_lod(lod, quality);

        self.scene.set_now_ms(sample.game.as_millis());

        if let Scene::Field(field) = &mut self.scene {
            field.time.raw_ms = sample.raw.as_millis();
        }

        sample.game
    }
}
