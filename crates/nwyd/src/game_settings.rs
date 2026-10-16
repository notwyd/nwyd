use crate::scene::lod::EffectOptions;
use anyhow::{Context, Result, ensure};
use formats::config::{Config, Language, config_bin::slot};
use serde::{Deserialize, Serialize};
#[cfg(not(target_arch = "wasm32"))]
use std::io::Write;
use std::path::Path;

pub const FILE: &str = "nwyd-settings.json";

pub const RESOLUTIONS: [[u32; 2]; 18] = [
    [800, 600],
    [1024, 768],
    [1280, 720],
    [1280, 800],
    [1280, 960],
    [1280, 1024],
    [1440, 900],
    [1600, 900],
    [1600, 1024],
    [1680, 1050],
    [1920, 1080],
    [1920, 1200],
    [2560, 1080],
    [2560, 1440],
    [2560, 1600],
    [3440, 1440],
    [3840, 1600],
    [3840, 2160],
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preferences {
    #[serde(default = "language::default_language", with = "language::serde")]
    pub language: Language,
    #[serde(default)]
    pub renderer: Option<render::device::RendererBackend>,
    pub resolution: [u32; 2],
    pub fullscreen: bool,
    #[serde(default)]
    pub vsync: bool,
    pub music: u8,
    pub sound: u8,
    pub effects: bool,
    pub grass: bool,
    pub invert_mouse: bool,
    #[serde(default)]
    pub interface: Interface,
}

impl Preferences {
    pub fn from_config(config: &Config) -> Self {
        let preset = config.preset();

        Self {
            language: language::default_language(),
            renderer: None,
            resolution: [preset.width, preset.height],
            fullscreen: config.fullscreen().unwrap_or(false),
            vsync: false,
            music: config.music_volume().clamp(0, 100) as u8,
            sound: config.sfx_volume().clamp(0, 100) as u8,
            effects: config.raw[slot::EFFECT_QUALITY] >= 2,
            grass: true,
            invert_mouse: config.invert_mouse(),
            interface: Interface::default(),
        }
    }

    fn validate(&self) -> Result<()> {
        ensure!(
            (640..=8192).contains(&self.resolution[0])
                && (480..=8192).contains(&self.resolution[1]),
            "invalid settings resolution"
        );

        ensure!(
            self.music <= 100 && self.sound <= 100,
            "invalid settings volume"
        );

        ensure!(
            (50..=100).contains(&self.interface.evok_size),
            "invalid Evok size"
        );

        Ok(())
    }

    pub fn load(path: &Path) -> Result<Option<Self>> {
        let bytes = match crate::user_paths::read(path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };

        Self::from_bytes(&bytes).map(Some)
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self> {
        let mut preferences: Self =
            serde_json::from_slice(bytes).context("reading game settings")?;

        if (10..50).contains(&preferences.interface.evok_size) {
            preferences.interface.evok_size = 50;
        }

        preferences.interface.hide_costume = false;

        preferences.interface.hide_mount_costume = false;

        preferences.validate()?;

        Ok(preferences)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&self, path: &Path) -> Result<()> {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(0);

        self.validate()?;

        let temp = path.with_extension(format!(
            "json.{}.{}.tmp",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));

        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp)?;

        let result = (|| {
            file.write_all(&serde_json::to_vec_pretty(self)?)?;

            file.sync_all()?;

            drop(file);

            std::fs::rename(&temp, path)?;

            Ok(())
        })();

        if result.is_err() {
            let _ = std::fs::remove_file(&temp);
        }

        result
    }

    #[cfg(target_arch = "wasm32")]
    pub fn save(&self, path: &Path) -> Result<()> {
        self.validate()?;

        crate::user_paths::write(path, &serde_json::to_vec(self)?)?;

        Ok(())
    }

    pub fn load_for_game(game: &assets::GameDir) -> Result<Option<Self>> {
        let path = crate::user_paths::settings()?;

        Self::load_for_game_at(&path, game)
    }

    fn load_for_game_at(path: &Path, game: &assets::GameDir) -> Result<Option<Self>> {
        if let Some(preferences) = Self::load(path)? {
            return Ok(Some(preferences));
        }

        let bytes = match game.read(FILE.as_bytes()) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(error.into()),
        };

        let preferences = Self::from_bytes(&bytes)?;

        preferences.save(path)?;

        Ok(Some(preferences))
    }

    pub fn load_or_import(path: &Path, legacy: &Path) -> Result<Option<Self>> {
        if let Some(preferences) = Self::load(path)? {
            return Ok(Some(preferences));
        }

        let preferences = Self::load(legacy)?;

        if let Some(preferences) = preferences {
            preferences.save(path)?;
        }

        Ok(preferences)
    }

    pub(crate) fn effect_options(self, mut current: EffectOptions) -> EffectOptions {
        if current.detail.effects != self.effects {
            current.toggle(false);
        }

        current.grass = self.grass;

        current
    }

    pub fn apply_audio_and_input(self, config: &mut Config) {
        config.raw[slot::MUSIC_VOLUME] = i16::from(self.music);

        config.raw[slot::SFX_VOLUME] = i16::from(self.sound);

        config.raw[slot::INVERT_MOUSE] = i16::from(self.invert_mouse);
    }

    pub fn present_mode(self) -> wgpu::PresentMode {
        if self.vsync {
            wgpu::PresentMode::Fifo
        } else {
            wgpu::PresentMode::Immediate
        }
    }
}

mod panel;

pub use panel::Panel;

mod interface;

pub use interface::Interface;

mod language;

pub use language::{available_languages, language_label, resolve_language};

#[cfg(test)]
mod tests {

    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestSettings(PathBuf);

    impl TestSettings {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);

            loop {
                let path = std::env::temp_dir().join(format!(
                    "nwyd-settings-test-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));

                match std::fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("creating test directory: {error}"),
                }
            }
        }

        fn path(&self) -> PathBuf {
            self.0.join(FILE)
        }
    }

    impl Drop for TestSettings {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn preferences() -> Preferences {
        Preferences::from_config(&Config::default())
    }

    #[test]
    fn config_import_clamps_volume_and_preserves_control_choices() {
        let mut config = Config::default();

        config.raw[slot::MUSIC_VOLUME] = -10;

        config.raw[slot::SFX_VOLUME] = 150;

        config.raw[slot::RESOLUTION] = 1;

        config.raw[slot::WINDOW_MODE] = 0;

        config.raw[slot::INVERT_MOUSE] = 1;

        config.raw[slot::EFFECT_QUALITY] = 1;

        let prefs = Preferences::from_config(&config);

        assert_eq!((prefs.music, prefs.sound), (0, 100));

        assert_eq!(prefs.resolution, [800, 600]);

        assert!(prefs.fullscreen);

        assert!(prefs.invert_mouse);

        assert!(!prefs.effects);

        assert_eq!(prefs.interface, Interface::default());

        assert_eq!(prefs.language, Language::Portuguese);
    }

    #[test]
    fn validation_checks_resolution_volume_and_character_size_boundaries() {
        for resolution in [[640, 480], [8192, 8192]] {
            let mut prefs = preferences();

            prefs.resolution = resolution;

            assert!(prefs.validate().is_ok());
        }

        for resolution in [[639, 480], [640, 479], [8193, 480], [640, 8193]] {
            let mut prefs = preferences();

            prefs.resolution = resolution;

            assert!(prefs.validate().is_err());
        }

        for (music, sound) in [(101, 0), (0, 101)] {
            let mut prefs = preferences();

            prefs.music = music;

            prefs.sound = sound;

            assert!(prefs.validate().is_err());
        }

        for size in [0, 49, 101, u8::MAX] {
            let mut prefs = preferences();

            prefs.interface.evok_size = size;

            assert!(prefs.validate().is_err());
        }

        for size in [50, 100] {
            let mut prefs = preferences();

            prefs.interface.evok_size = size;

            assert!(prefs.validate().is_ok());
        }
    }

    #[test]
    fn settings_schema_defaults_optional_fields_and_rejects_unknown_fields() {
        let prefs = preferences();

        let mut json = serde_json::to_value(prefs).unwrap();

        let object = json.as_object_mut().unwrap();

        object.remove("vsync");

        object.remove("interface");

        object.remove("language");

        object.remove("renderer");

        assert_eq!(
            serde_json::from_value::<Preferences>(json.clone()).unwrap(),
            prefs
        );

        json["unknown_setting"] = true.into();

        assert!(serde_json::from_value::<Preferences>(json).is_err());

        let mut json = serde_json::to_value(prefs).unwrap();

        json["interface"]["unknown_setting"] = true.into();

        assert!(serde_json::from_value::<Preferences>(json).is_err());
    }

    #[test]
    fn settings_save_round_trips_and_replaces_without_temporary_files() {
        let files = TestSettings::new();

        let path = files.path();

        assert_eq!(Preferences::load(&path).unwrap(), None);

        let mut prefs = preferences();

        prefs.save(&path).unwrap();

        assert_eq!(Preferences::load(&path).unwrap(), Some(prefs));

        prefs.sound = 17;

        prefs.vsync = true;

        prefs.resolution = [1920, 1080];

        prefs.interface.zoom = true;

        prefs.language = Language::English;

        prefs.renderer = Some(render::device::RendererBackend::Vulkan);

        prefs.save(&path).unwrap();

        assert_eq!(Preferences::load(&path).unwrap(), Some(prefs));

        assert_eq!(std::fs::read_dir(&files.0).unwrap().count(), 1);
    }

    #[test]
    fn legacy_preferences_are_imported_without_changing_game_files() {
        let game = TestSettings::new();

        let user = TestSettings::new();

        let mut prefs = preferences();

        prefs.music = 23;

        let mut json = serde_json::to_value(prefs).unwrap();

        json.as_object_mut().unwrap().remove("language");

        let original = serde_json::to_vec(&json).unwrap();

        std::fs::write(game.path(), &original).unwrap();

        let imported = Preferences::load_or_import(&user.path(), &game.path()).unwrap();

        assert_eq!(imported, Some(prefs));

        assert_eq!(Preferences::load(&user.path()).unwrap(), Some(prefs));

        assert_eq!(std::fs::read(game.path()).unwrap(), original);

        assert_eq!(std::fs::read_dir(&game.0).unwrap().count(), 1);

        prefs.language = Language::English;

        prefs.music = 61;

        prefs.save(&user.path()).unwrap();

        assert_eq!(
            Preferences::load_or_import(&user.path(), &game.path()).unwrap(),
            Some(prefs)
        );

        assert_eq!(std::fs::read(game.path()).unwrap(), original);
    }

    #[test]
    fn a_missing_legacy_file_does_not_create_a_settings_file() {
        let game = TestSettings::new();

        let user = TestSettings::new();

        assert_eq!(
            Preferences::load_or_import(&user.path(), &game.path()).unwrap(),
            None
        );

        assert!(!user.path().exists());

        assert!(!game.path().exists());
    }

    #[test]
    fn legacy_size_migrates_and_server_owned_costume_flags_are_reset() {
        let files = TestSettings::new();

        let mut json = serde_json::to_value(preferences()).unwrap();

        json["interface"]["evok_size"] = 10.into();

        json["interface"]["hide_costume"] = true.into();

        json["interface"]["hide_mount_costume"] = true.into();

        std::fs::write(files.path(), serde_json::to_vec(&json).unwrap()).unwrap();

        let loaded = Preferences::load(&files.path()).unwrap().unwrap();

        assert_eq!(loaded.interface.evok_size, 50);

        assert!(!loaded.interface.hide_costume);

        assert!(!loaded.interface.hide_mount_costume);

        json["interface"]["evok_size"] = 9.into();

        std::fs::write(files.path(), serde_json::to_vec(&json).unwrap()).unwrap();

        assert!(Preferences::load(&files.path()).is_err());

        std::fs::write(files.path(), b"not json").unwrap();

        assert!(Preferences::load(&files.path()).is_err());
    }

    #[test]
    fn invalid_save_does_not_replace_existing_settings() {
        let files = TestSettings::new();

        let path = files.path();

        let mut prefs = preferences();

        prefs.save(&path).unwrap();

        let before = std::fs::read(&path).unwrap();

        prefs.music = 101;

        assert!(prefs.save(&path).is_err());

        assert_eq!(std::fs::read(&path).unwrap(), before);

        assert_eq!(std::fs::read_dir(&files.0).unwrap().count(), 1);
    }

    #[test]
    fn audio_and_input_application_preserves_other_config_slots() {
        let mut config = Config::default();

        let before = config;

        let mut prefs = preferences();

        prefs.music = 23;

        prefs.sound = 47;

        prefs.invert_mouse = true;

        prefs.apply_audio_and_input(&mut config);

        for index in 0..config.raw.len() {
            let expected = match index {
                slot::MUSIC_VOLUME => 23,
                slot::SFX_VOLUME => 47,
                slot::INVERT_MOUSE => 1,
                _ => before.raw[index],
            };

            assert_eq!(config.raw[index], expected, "slot {index}");
        }

        assert_eq!(prefs.present_mode(), wgpu::PresentMode::Immediate);

        prefs.vsync = true;

        assert_eq!(prefs.present_mode(), wgpu::PresentMode::Fifo);
    }

    #[test]
    fn transient_appearance_flags_are_not_written_to_settings() {
        let mut prefs = preferences();

        prefs.interface.hide_helmet = true;

        prefs.interface.hide_costume = true;

        prefs.interface.hide_mount_costume = true;

        let json = serde_json::to_value(prefs).unwrap();

        for key in ["hide_helmet", "hide_costume", "hide_mount_costume"] {
            assert!(json["interface"].get(key).is_none());
        }

        let decoded: Preferences = serde_json::from_value(json).unwrap();

        assert_eq!(decoded.interface, Interface::default());
    }

    #[test]
    fn importing_browser_preferences_keeps_game_files_read_only_and_prefers_user_changes() {
        let files = TestSettings::new();

        let original = preferences();

        let bytes = serde_json::to_vec(&original).unwrap();

        let game = assets::GameDir::from_files(
            "selected-game",
            [("NWYD-SETTINGS.JSON".into(), bytes.clone())],
        )
        .unwrap();

        assert_eq!(
            Preferences::load_for_game_at(&files.path(), &game).unwrap(),
            Some(original)
        );

        let mut changed = original;

        changed.music = 17;

        changed.save(&files.path()).unwrap();

        assert_eq!(
            Preferences::load_for_game_at(&files.path(), &game).unwrap(),
            Some(changed)
        );

        assert_eq!(game.read(FILE.as_bytes()).unwrap(), bytes);
    }
}
