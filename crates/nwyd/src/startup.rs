use crate::clock::{Calendar, Clock};
use crate::error_context::Operation;
use crate::scene::login_static::{LoginPresentation, SceneTime};
use crate::scene::{self, select_char, server_select};
use crate::window::{self, Client, Settings};
use anyhow::{Context, Result};
use assets::GameDir;
use formats::config::{Config, config_bin};

const CONFIG_FILE: &[u8] = b"Config.bin";

const STATUS_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

pub fn load_client(mut game: GameDir, analytics: crate::analytics::Analytics) -> Result<Client> {
    let profile = client_profile(&game)?;

    let items = crate::scene::item_table::load(&game)?;

    let mut config = load_config(&game);

    let settings_path = crate::user_paths::settings()?;

    let saved = match crate::game_settings::Preferences::load_for_game(&game) {
        Ok(saved) => saved,
        Err(error) => {
            tracing::warn!(% error, "ignoring invalid game settings");

            None
        }
    };

    if let Some(saved) = saved {
        saved.apply_audio_and_input(&mut config);
    }

    let mut preferences = saved.unwrap_or_else(|| {
        let mut preferences = crate::game_settings::Preferences::from_config(&config);

        preferences.fullscreen = config.fullscreen().unwrap_or(true);

        preferences
    });

    preferences.language = crate::game_settings::resolve_language(
        preferences.language,
        &crate::game_settings::available_languages(&game),
    );

    game.set_language(preferences.language);

    let backbuffer = (preferences.resolution[0], preferences.resolution[1]);

    let presentation = LoginPresentation::from_config(&config);

    let clock = Clock::new();

    let calendar = Calendar::local();

    let time = SceneTime::live(&clock, &calendar);

    let mut scene = scene::Scene::ServerSelect(Box::new(
        server_select::build(&game, backbuffer, &presentation, &time, true, None)
            .context(Operation::Scene)?,
    ));

    scene.allow_status_fetch(STATUS_TIMEOUT);

    let settings = Settings {
        title: "nwyd".to_owned(),
        backbuffer,
        fullscreen: preferences.fullscreen && !cfg!(target_arch = "wasm32"),
        present_mode: preferences.present_mode(),
        renderer: preferences.renderer,
        overlay: false,
        pointers: scene::cursor::Pointers::load(&game, config.cursor_mode()),
    };

    let mut audio = scene::audio::SceneAudio::open(&game, &config, false);

    audio.set_volumes(preferences.sound, preferences.music);

    audio.enter();

    let screens = GameScreens {
        items,
        game: game.clone(),
        backbuffer,
        presentation,
        time,
        captions: true,
        variant: None,
        chrome: scene::field::Chrome {
            hud: true,
            captions: true,
        },
    };

    Ok(Client::new(settings, scene)
        .with_render_quality(config.raw[config_bin::slot::EFFECT_QUALITY], true)
        .with_inverted_mouse(config.invert_mouse())
        .with_audio(audio)
        .with_session(
            crate::net::Session::new(crate::net::Verbosity::Quiet)
                .with_profile(profile)
                .with_analytics(analytics),
        )
        .with_game_settings(preferences, settings_path, &game)
        .with_screens(Box::new(screens)))
}

struct GameScreens {
    game: GameDir,
    items: formats::data::item_list::ItemList,
    backbuffer: (u32, u32),
    presentation: LoginPresentation,
    time: SceneTime,
    captions: bool,
    variant: Option<u32>,
    chrome: scene::field::Chrome,
}

impl window::Screens for GameScreens {
    fn set_backbuffer(&mut self, backbuffer: (u32, u32)) {
        self.backbuffer = backbuffer;
    }

    fn game(&self) -> &GameDir {
        &self.game
    }

    fn game_mut(&mut self) -> &mut GameDir {
        &mut self.game
    }

    fn items(&self) -> &formats::data::item_list::ItemList {
        &self.items
    }

    fn server_select(&self) -> Result<scene::Scene> {
        let scene = server_select::build(
            &self.game,
            self.backbuffer,
            &self.presentation,
            &self.time,
            self.captions,
            self.variant,
        )
        .context("rebuilding the server-select screen")?;

        Ok(scene::Scene::ServerSelect(Box::new(scene)))
    }

    fn select_char(
        &self,
        block: Option<&formats::net::account::Block>,
        lock: u8,
    ) -> Result<scene::Scene> {
        let scene = select_char::build_at_lock(
            &self.game,
            self.backbuffer,
            &self.time,
            self.captions,
            lock,
            block,
        )
        .context("building the character-select screen")?;

        Ok(scene::Scene::SelectChar(Box::new(scene)))
    }

    fn field(&self, entered: &scene::enter::Entered) -> Result<scene::Scene> {
        let scene = scene::field::build_entry(
            &self.game,
            self.backbuffer,
            &self.time,
            entered,
            &self.chrome,
        )
        .context("building the field screen")?;

        let scene = scene::Scene::Field(Box::new(scene));

        Ok(scene)
    }

    fn write_quick_slots(&self, bytes: &[u8]) -> Result<()> {
        write_quick_slots(&self.game, bytes)
    }
}

fn write_quick_slots(game: &GameDir, bytes: &[u8]) -> Result<()> {
    anyhow::ensure!(
        bytes.len() == formats::data::quick_slot::LEN,
        "NQuickSLot_01.bin needs {} bytes, got {}",
        formats::data::quick_slot::LEN,
        bytes.len(),
    );

    crate::user_paths::write_quick_slots(game, bytes)
}

fn client_profile(game: &GameDir) -> Result<protocol::send_state::Profile> {
    let bytes = game
        .read(protocol::client::PROXY_DLL.as_bytes())
        .context("could not read version.dll from the game directory")?;

    let word = protocol::proxy::build_word(&bytes)
        .context("could not discover the login version in version.dll")?;

    let profile = protocol::send_state::Profile::from_build_word(word).with_context(|| {
        format!("version.dll build {word:#06X} uses an unsupported timestamp watermark")
    })?;

    tracing::info!(
        build = format_args!("{word:#06X}"),
        version = format_args!("{:#010X}", profile.login_version()),
        "NWYD: protocol profile resolved from version.dll"
    );

    Ok(profile)
}

fn load_config(game: &GameDir) -> Config {
    match game
        .read(CONFIG_FILE)
        .map_err(|e| e.to_string())
        .and_then(|bytes| config_bin::parse(&bytes).map_err(|e| e.to_string()))
    {
        Ok(config) => config,
        Err(reason) => {
            tracing::info!("NWYD: Config.bin unreadable ({reason}); using the built-in defaults");

            Config::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_missing_proxy_stops_startup_before_loading_other_assets() {
        let game = GameDir::from_files("test-game", []).unwrap();
        let error = match load_client(game, Default::default()) {
            Ok(_) => panic!("startup accepted a missing proxy"),
            Err(error) => error,
        };

        assert!(error.to_string().contains("could not read version.dll"));
    }

    #[test]
    fn an_invalid_proxy_cannot_select_a_fallback_protocol() {
        let game = GameDir::from_files(
            "test-game",
            [("VERSION.DLL".into(), b"not a PE image".to_vec())],
        )
        .unwrap();
        let error = client_profile(&game).unwrap_err();

        assert!(
            error
                .to_string()
                .contains("could not discover the login version")
        );
    }
}
