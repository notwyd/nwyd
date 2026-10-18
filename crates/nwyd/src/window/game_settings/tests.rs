use super::*;
use crate::{
    clock::Calendar,
    scene::{
        camera::MouseInput,
        cursor::Pointers,
        login_static::{Layout, LoginScene, SceneTime},
    },
    window::Settings,
};
use ui::text_entry::Mods;
use winit::{
    event::ElementState,
    keyboard::{Key, KeyCode, NamedKey, PhysicalKey},
};

fn preferences() -> Preferences {
    Preferences {
        language: formats::config::Language::Portuguese,
        resolution: [800, 600],
        fullscreen: false,
        vsync: false,
        renderer: None,
        music: 60,
        sound: 70,
        effects: true,
        old_skill_vfx: true,
        neural_rendering: false,
        view_distance: Default::default(),
        grass: true,
        invert_mouse: false,
        interface: Default::default(),
    }
}

fn client() -> Client {
    let scene = Scene::LoginStatic(LoginScene {
        layout: Layout::new(800, 600),
        time: SceneTime::pinned(1000, Calendar::GOLDEN),
        draws: Vec::new(),
        textures: Default::default(),
        missing: Vec::new(),
        logo_width: None,
    });

    let mut client = Client::new(
        Settings {
            title: "Settings test".into(),
            backbuffer: (800, 600),
            fullscreen: false,
            present_mode: wgpu::PresentMode::Immediate,
            renderer: None,
            overlay: false,
            pointers: Pointers::none(),
        },
        scene,
    );

    client.runtime.game_settings = Some(Panel::for_test(preferences()));

    client.runtime.retained.effect_options = Some(preferences().effect_options(Default::default()));

    client
}

fn shortcut_modifiers() -> Mods {
    Mods {
        ctrl: true,
        shift: true,
        alt: false,
    }
}

fn key(client: &mut Client, text: &str, state: ElementState, repeat: bool) {
    client.route_key(
        &PhysicalKey::Code(KeyCode::KeyS),
        &Key::Character(text.into()),
        Some(text),
        state,
        repeat,
    );
}

#[test]
fn shortcut_opens_settings_before_login_without_a_window_or_connection() {
    let mut client = client();

    client.runtime.input.mods = shortcut_modifiers();

    assert!(client.live.is_none());

    assert!(!client.runtime.net.is_connected());

    assert!(!client.settings_visible());

    for text in ["s", "S"] {
        key(&mut client, text, ElementState::Pressed, false);

        assert!(client.settings_visible());

        assert!(matches!(client.runtime.scene, Scene::LoginStatic(_)));

        assert!(!client.runtime.net.is_connected());

        client.runtime.game_settings.as_mut().unwrap().close();
    }
}

#[test]
fn shortcut_uses_the_logical_character_for_keyboard_layouts() {
    let mut client = client();

    client.runtime.input.mods = shortcut_modifiers();

    client.route_key(
        &PhysicalKey::Code(KeyCode::KeyQ),
        &Key::Character("S".into()),
        Some("S"),
        ElementState::Pressed,
        false,
    );

    assert!(client.settings_visible());
}

#[test]
fn plain_s_and_incomplete_or_alt_modified_shortcuts_do_not_open_settings() {
    let mut client = client();

    for ctrl in [false, true] {
        for shift in [false, true] {
            for alt in [false, true] {
                if ctrl && shift && !alt {
                    continue;
                }

                client.runtime.input.mods = Mods { ctrl, shift, alt };

                key(&mut client, "s", ElementState::Pressed, false);

                assert!(
                    !client.settings_visible(),
                    "ctrl={ctrl} shift={shift} alt={alt}"
                );
            }
        }
    }
}

#[test]
fn shortcut_repeats_and_releases_do_not_open_or_cancel_input() {
    let mut client = client();

    client.runtime.input.mods = shortcut_modifiers();

    client.runtime.ime.composing = true;

    let mouse = MouseInput {
        dx: 11,
        dy: 12,
        dz: 13,
        mmb: true,
        rmb: true,
        alt: false,
    };

    client.runtime.input.mouse = mouse;

    for (state, repeat) in [
        (ElementState::Pressed, true),
        (ElementState::Released, false),
    ] {
        key(&mut client, "S", state, repeat);

        assert!(!client.settings_visible());

        assert!(client.runtime.ime.composing);

        assert_eq!(client.runtime.input.mouse, mouse);
    }
}

#[test]
fn opening_settings_cancels_active_composition_and_pending_camera_input() {
    let mut client = client();

    client.runtime.input.mods = shortcut_modifiers();

    client.runtime.ime.composing = true;

    client.runtime.input.mouse = MouseInput {
        dx: 3,
        dy: -4,
        dz: 5,
        mmb: true,
        rmb: true,
        alt: true,
    };

    key(&mut client, "s", ElementState::Pressed, false);

    assert!(client.settings_visible());

    assert!(!client.runtime.ime.composing);

    assert_eq!(client.runtime.input.mouse, MouseInput::default());
}

#[test]
fn invoking_the_shortcut_again_preserves_unsaved_settings() {
    let mut client = client();

    client.runtime.input.mods = shortcut_modifiers();

    key(&mut client, "s", ElementState::Pressed, false);

    let panel = client.runtime.game_settings.as_mut().unwrap();

    panel.draft.music = 13;

    panel.draft.sound = 17;

    panel.draft.resolution = [1920, 1080];

    panel.draft.fullscreen = true;

    let draft = panel.draft;

    client.open_game_settings();

    key(&mut client, "S", ElementState::Pressed, false);

    key(&mut client, "S", ElementState::Pressed, true);

    key(&mut client, "S", ElementState::Released, false);

    assert!(client.settings_visible());

    let panel = client.runtime.game_settings.as_ref().unwrap();

    assert_eq!(panel.draft, draft);

    assert_eq!(panel.applied, preferences());

    assert!(!panel.apply_requested);
}

#[test]
fn escape_closes_settings_and_held_shortcut_repeats_do_not_reopen_them() {
    let mut client = client();

    client.runtime.input.mods = shortcut_modifiers();

    key(&mut client, "s", ElementState::Pressed, false);

    client.runtime.game_settings.as_mut().unwrap().draft.music = 5;

    client.route_key(
        &PhysicalKey::Code(KeyCode::Escape),
        &Key::Named(NamedKey::Escape),
        None,
        ElementState::Pressed,
        false,
    );

    assert!(!client.settings_visible());

    key(&mut client, "s", ElementState::Pressed, true);

    key(&mut client, "s", ElementState::Released, false);

    assert!(!client.settings_visible());

    client.route_key(
        &PhysicalKey::Code(KeyCode::Escape),
        &Key::Named(NamedKey::Escape),
        None,
        ElementState::Released,
        false,
    );

    key(&mut client, "s", ElementState::Pressed, false);

    assert!(client.settings_visible());

    let panel = client.runtime.game_settings.as_ref().unwrap();

    assert_eq!(panel.draft, preferences());

    assert!(!panel.apply_requested);
}

#[test]
fn prelogin_settings_use_retained_effect_preferences() {
    let mut client = client();

    client.runtime.input.mods = shortcut_modifiers();

    let mut effects = EffectOptions::default();

    effects.detail.effects = false;

    effects.grass = false;

    effects.old_skill_vfx = false;

    client.runtime.retained.effect_options = Some(effects);

    assert!(client.runtime.scene.effect_options().is_none());

    key(&mut client, "s", ElementState::Pressed, false);

    assert!(client.settings_visible());

    let panel = client.runtime.game_settings.as_ref().unwrap();

    assert!(!panel.draft.effects);

    assert!(!panel.draft.grass);

    assert!(!panel.applied.effects);

    assert!(!panel.applied.grass);

    assert!(!panel.draft.old_skill_vfx);

    assert!(!panel.applied.old_skill_vfx);

    assert_eq!(panel.draft.music, preferences().music);
}

#[test]
fn prelogin_resolution_changes_resize_the_scene_and_keep_settings_open() {
    let mut client = client();

    client.open_game_settings();

    client.resize_game((1280, 720));

    assert_eq!(client.settings.backbuffer, (1280, 720));

    assert_eq!(client.runtime.scene.viewport(), (1280, 720));

    assert!(client.settings_visible());

    assert!(!client.runtime.net.is_connected());
}

#[test]
fn shortcut_is_safe_when_the_settings_panel_is_unavailable() {
    let mut client = client();

    client.runtime.input.mods = shortcut_modifiers();

    client.runtime.game_settings = None;

    key(&mut client, "s", ElementState::Pressed, false);

    assert!(!client.settings_visible());

    assert!(client.failure.is_none());
}

struct SettingsDirectory(std::path::PathBuf);

impl SettingsDirectory {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};

        static NEXT: AtomicU64 = AtomicU64::new(0);

        loop {
            let path = std::env::temp_dir().join(format!(
                "nwyd-live-settings-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));

            match std::fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => panic!("creating settings test directory: {error}"),
            }
        }
    }

    fn settings(&self) -> std::path::PathBuf {
        self.0.join("settings.json")
    }
}

impl Drop for SettingsDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn saving_settings_preserves_session_identity_and_runtime_state() {
    let directory = SettingsDirectory::new();

    let mut client = client();

    client.open_game_settings();

    client.runtime.net.account = "synthetic-account".into();

    client.runtime.net.verbosity = crate::net::Verbosity::Packets;

    let guild_proxy = client.runtime.net.guild_proxy.clone();

    let config = client.runtime.net.client_config.clone();

    client.runtime.retained.account_lock = 19;

    client.frames = 123;

    client.runtime.input.mods = shortcut_modifiers();

    let panel = client.runtime.game_settings.as_mut().unwrap();

    panel.path = directory.settings();

    panel.draft.music = 23;

    panel.draft.sound = 41;

    panel.draft.invert_mouse = true;

    panel.apply_requested = true;

    let expected = panel.draft;

    client.apply_game_settings();

    assert_eq!(
        Preferences::load(&directory.settings()).unwrap(),
        Some(expected)
    );

    let panel = client.runtime.game_settings.as_ref().unwrap();

    assert_eq!(panel.applied, expected);

    assert_eq!(panel.draft, expected);

    assert!(panel.visible);

    assert!(!panel.apply_requested);

    assert_eq!(client.runtime.net.account, "synthetic-account");

    assert_eq!(client.runtime.net.verbosity, crate::net::Verbosity::Packets);

    assert!(std::sync::Arc::ptr_eq(
        &guild_proxy,
        &client.runtime.net.guild_proxy
    ));

    assert!(std::sync::Arc::ptr_eq(
        &config,
        &client.runtime.net.client_config
    ));

    assert_eq!(client.runtime.retained.account_lock, 19);

    assert_eq!(client.frames, 123);

    assert!(client.runtime.input.mods.ctrl && client.runtime.input.mods.shift);

    assert!(client.runtime.input.invert_mouse);

    let Scene::LoginStatic(scene) = &client.runtime.scene else {
        panic!("scene replaced")
    };

    assert_eq!(scene.time.now_ms, 1000);

    assert_eq!(scene.layout.width, 800);
}

#[test]
fn old_skill_vfx_applies_without_restarting_and_survives_scene_changes() {
    let replacement = client().runtime.scene;

    let directory = SettingsDirectory::new();

    let mut client = client();

    client.error_dialogs = false;

    client.open_game_settings();

    let initial = client.runtime.retained.effect_options.unwrap();

    for enabled in [false, true, false] {
        let panel = client.runtime.game_settings.as_mut().unwrap();

        panel.path = directory.settings();

        panel.draft.old_skill_vfx = enabled;

        panel.apply_requested = true;

        client.apply_game_settings();

        let applied = client.runtime.retained.effect_options.unwrap();

        assert_eq!(applied.old_skill_vfx, enabled);

        assert_eq!(applied.detail, initial.detail);

        assert_eq!(applied.grass, initial.grass);

        assert_eq!(
            Preferences::load(&directory.settings())
                .unwrap()
                .unwrap()
                .old_skill_vfx,
            enabled
        );

        assert!(!client.runtime.net.is_connected());
    }

    client.show(replacement);

    client.runtime.game_settings.as_mut().unwrap().close();

    client.open_game_settings();

    let panel = client.runtime.game_settings.as_mut().unwrap();

    assert!(!panel.applied.old_skill_vfx);

    assert!(!panel.draft.old_skill_vfx);

    panel.path = directory.0.join("missing/settings.json");

    panel.draft.old_skill_vfx = true;

    panel.apply_requested = true;

    client.apply_game_settings();

    assert!(
        !client
            .runtime
            .retained
            .effect_options
            .unwrap()
            .old_skill_vfx
    );

    assert!(client.settings_visible());

    assert!(
        !client
            .runtime
            .game_settings
            .as_ref()
            .unwrap()
            .draft
            .old_skill_vfx
    );

    assert!(
        !Preferences::load(&directory.settings())
            .unwrap()
            .unwrap()
            .old_skill_vfx
    );
}

#[test]
fn renderer_selection_is_saved_without_changing_the_running_configuration() {
    use render::device::RendererBackend;

    let directory = SettingsDirectory::new();
    let mut client = client();
    client.settings.renderer = Some(RendererBackend::Vulkan);
    let panel = client.runtime.game_settings.as_mut().unwrap();
    panel.path = directory.settings();
    panel.set_renderers(
        &[RendererBackend::Vulkan, RendererBackend::Dx12],
        RendererBackend::Vulkan,
    );
    panel.draft.renderer = Some(RendererBackend::Dx12);
    panel.apply_requested = true;

    client.apply_game_settings();

    let saved = Preferences::load(&directory.settings()).unwrap().unwrap();
    assert_eq!(saved.renderer, Some(RendererBackend::Dx12));
    assert_eq!(
        client.runtime.game_settings.as_ref().unwrap().applied,
        saved
    );
    assert_eq!(client.settings.renderer, Some(RendererBackend::Vulkan));
    assert_eq!(client.settings.backbuffer, (800, 600));
    assert!(matches!(client.runtime.scene, Scene::LoginStatic(_)));
}

#[test]
fn view_distance_applies_live_and_survives_scene_changes_and_failed_saves() {
    use crate::scene::view_distance::ViewDistance;

    let replacement = client().runtime.scene;
    let directory = SettingsDirectory::new();
    let mut client = client();
    client.error_dialogs = false;
    client.open_game_settings();

    assert_eq!(client.runtime.retained.view_distance, ViewDistance::Retail);

    for distance in ViewDistance::ALL
        .into_iter()
        .chain([ViewDistance::Retail, ViewDistance::EightTimes])
    {
        let panel = client.runtime.game_settings.as_mut().unwrap();
        panel.path = directory.settings();
        panel.draft.view_distance = distance;
        panel.apply_requested = true;

        client.apply_game_settings();

        assert_eq!(client.runtime.retained.view_distance, distance);
        assert_eq!(
            Preferences::load(&directory.settings())
                .unwrap()
                .unwrap()
                .view_distance,
            distance
        );
        assert!(!client.runtime.net.is_connected());
    }

    client.show(replacement);
    client.open_game_settings();

    assert_eq!(
        client.runtime.retained.view_distance,
        ViewDistance::EightTimes
    );
    let panel = client.runtime.game_settings.as_mut().unwrap();
    assert_eq!(panel.applied.view_distance, ViewDistance::EightTimes);
    assert_eq!(panel.draft.view_distance, ViewDistance::EightTimes);
    panel.path = directory.0.join("missing/settings.json");
    panel.draft.view_distance = ViewDistance::Retail;
    panel.apply_requested = true;

    client.apply_game_settings();

    assert_eq!(
        client.runtime.retained.view_distance,
        ViewDistance::EightTimes
    );
    assert_eq!(
        client
            .runtime
            .game_settings
            .as_ref()
            .unwrap()
            .draft
            .view_distance,
        ViewDistance::EightTimes
    );
    assert_eq!(
        Preferences::load(&directory.settings())
            .unwrap()
            .unwrap()
            .view_distance,
        ViewDistance::EightTimes
    );
}

#[test]
fn startup_applies_view_distance_even_when_settings_panel_assets_are_missing() {
    use crate::scene::view_distance::ViewDistance;

    let directory = SettingsDirectory::new();
    let game = assets::GameDir::from_files("empty-settings-test", Vec::<(String, Vec<u8>)>::new())
        .unwrap();
    let mut preferences = preferences();
    preferences.view_distance = ViewDistance::EightTimes;

    let mut client = client();
    client.runtime.game_settings = None;
    let client = client.with_game_settings(preferences, directory.settings(), &game);

    assert_eq!(
        client.runtime.retained.view_distance,
        ViewDistance::EightTimes
    );
    assert!(client.runtime.game_settings.is_none());
}

#[test]
fn failed_settings_save_rolls_back_draft_and_keeps_previous_file() {
    let directory = SettingsDirectory::new();

    let mut panel = Panel::for_test(preferences());

    panel.path = directory.settings();

    panel.applied.save(&panel.path).unwrap();

    let original = std::fs::read(&panel.path).unwrap();

    panel.open(Default::default());

    let previous = panel.applied;

    panel.draft.music = 101;

    panel.draft.language = formats::config::Language::English;

    assert!(panel.save_draft().is_err());

    assert_eq!(panel.applied, previous);

    assert_eq!(panel.draft, previous);

    assert!(panel.visible);

    assert_eq!(std::fs::read(&panel.path).unwrap(), original);

    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);

    panel.path = directory.0.join("missing/settings.json");

    panel.draft.language = formats::config::Language::English;

    panel.draft.renderer = Some(render::device::RendererBackend::Dx12);

    assert!(panel.save_draft().is_err());

    assert_eq!(panel.applied, previous);

    assert_eq!(panel.draft, previous);
}

#[test]
fn neural_rendering_can_be_saved_with_resolution_changes_before_a_window_exists() {
    let directory = SettingsDirectory::new();
    let mut client = client();
    let panel = client.runtime.game_settings.as_mut().unwrap();
    panel.path = directory.settings();
    panel.set_neural_rendering_available(true);
    panel.draft.neural_rendering = true;
    panel.draft.resolution = [1280, 720];
    panel.apply_requested = true;

    client.apply_game_settings();

    assert!(client.live.is_none());
    assert_eq!(client.settings.backbuffer, (1280, 720));
    assert!(
        Preferences::load(&directory.settings())
            .unwrap()
            .unwrap()
            .neural_rendering
    );
    assert!(matches!(client.runtime.scene, Scene::LoginStatic(_)));

    let panel = client.runtime.game_settings.as_mut().unwrap();
    panel.draft.neural_rendering = false;
    panel.apply_requested = true;

    client.apply_game_settings();

    assert!(
        !Preferences::load(&directory.settings())
            .unwrap()
            .unwrap()
            .neural_rendering
    );
    assert_eq!(client.settings.backbuffer, (1280, 720));
    assert!(matches!(client.runtime.scene, Scene::LoginStatic(_)));
}

struct LanguageScreens {
    game: assets::GameDir,
    items: formats::data::item_list::ItemList,
}

impl crate::runtime::screens::Screens for LanguageScreens {
    fn set_backbuffer(&mut self, _: (u32, u32)) {}

    fn game(&self) -> &assets::GameDir {
        &self.game
    }

    fn game_mut(&mut self) -> &mut assets::GameDir {
        &mut self.game
    }

    fn items(&self) -> &formats::data::item_list::ItemList {
        &self.items
    }

    fn server_select(&self) -> anyhow::Result<Scene> {
        panic!("language switch must not enter server selection")
    }

    fn select_char(
        &self,
        _: Option<&formats::net::account::Block>,
        _: u8,
    ) -> anyhow::Result<Scene> {
        panic!("language switch must not enter character selection")
    }

    fn field(&self, _: &crate::scene::enter::Entered) -> anyhow::Result<Scene> {
        panic!("language switch must not enter the world")
    }

    fn write_quick_slots(&self, _: &[u8]) -> anyhow::Result<()> {
        panic!("language switch must not write game data")
    }
}

fn language_client(directory: &SettingsDirectory) -> Client {
    for code in ["PT", "EN"] {
        std::fs::create_dir_all(directory.0.join("game/Lang").join(code)).unwrap();

        std::fs::write(
            directory.0.join(format!("game/Lang/{code}/UIString.txt")),
            format!("1 {code}_label\n"),
        )
        .unwrap();
    }

    std::fs::write(
        directory.0.join("game/Lang/EN/Caption.txt"),
        b"FFFFFF English fallback\r\n",
    )
    .unwrap();

    let mut game = assets::GameDir::open(directory.0.join("game")).unwrap();

    game.set_language(formats::config::Language::Portuguese);

    let mut client = client().with_screens(Box::new(LanguageScreens {
        game,
        items: formats::data::item_list::ItemList::from_plain(
            vec![0; formats::data::item_list::BODY_LEN],
            formats::text::Codepage::Windows1252,
        ),
    }));

    client.error_dialogs = false;

    client.open_game_settings();

    client.runtime.game_settings.as_mut().unwrap().path = directory.settings();

    preferences().save(&directory.settings()).unwrap();

    client
}

fn request_language(client: &mut Client, language: formats::config::Language) {
    let panel = client.runtime.game_settings.as_mut().unwrap();

    panel.draft.language = language;

    panel.apply_requested = true;
}

#[test]
fn live_language_changes_keep_the_existing_socket_and_session_without_sending_packets() {
    use formats::config::Language::{English, Portuguese};
    use std::io::Read;

    let directory = SettingsDirectory::new();

    let mut client = language_client(&directory);

    let listener = std::net::TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();

    let connection = protocol::transport::Connection::connect(
        "127.0.0.1",
        listener.local_addr().unwrap().port(),
        std::time::Duration::from_secs(2),
        123,
    )
    .unwrap();

    let (mut peer, _) = listener.accept().unwrap();

    peer.set_read_timeout(Some(std::time::Duration::from_secs(2)))
        .unwrap();

    let mut hello = [0; protocol::codec::HELLO.len()];

    peer.read_exact(&mut hello).unwrap();

    assert_eq!(hello, protocol::codec::HELLO);

    peer.set_nonblocking(true).unwrap();

    client.runtime.net = crate::net::Session::connected_for_test(connection);

    client.runtime.retained.account_lock = 37;

    let connection = client.runtime.net.describe();

    for language in [English, Portuguese, English, English] {
        request_language(&mut client, language);

        client.apply_game_settings();

        let panel = client.runtime.game_settings.as_ref().unwrap();

        assert_eq!(panel.applied.language, language);

        assert_eq!(panel.draft.language, language);

        assert!(panel.visible);

        assert!(!panel.apply_requested);

        assert_eq!(client.screens.as_ref().unwrap().game().language(), language);

        assert_eq!(
            Preferences::load(&directory.settings())
                .unwrap()
                .unwrap()
                .language,
            language
        );

        assert!(client.runtime.net.is_connected());

        assert_eq!(client.runtime.net.phase(), crate::net::Phase::InGame);

        assert_eq!(client.runtime.net.account, "synthetic-account");

        assert_eq!(client.runtime.net.describe(), connection);

        assert_eq!(client.runtime.retained.account_lock, 37);

        let Scene::LoginStatic(scene) = &client.runtime.scene else {
            panic!("scene replaced")
        };

        assert_eq!(scene.time.now_ms, 1000);

        let mut byte = [0];

        assert_eq!(
            peer.read(&mut byte).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock
        );
    }
}

#[test]
fn unreadable_language_files_roll_back_settings_without_mutating_active_resources() {
    let directory = SettingsDirectory::new();

    let mut client = language_client(&directory);

    let previous = std::fs::read(directory.settings()).unwrap();

    std::fs::remove_file(directory.0.join("game/Lang/EN/UIString.txt")).unwrap();

    request_language(&mut client, formats::config::Language::English);

    client.runtime.game_settings.as_mut().unwrap().draft.music = 13;

    client.apply_game_settings();

    let panel = client.runtime.game_settings.as_ref().unwrap();

    assert_eq!(panel.applied, preferences());

    assert_eq!(panel.draft, preferences());

    assert!(panel.visible);

    assert!(!panel.apply_requested);

    assert_eq!(
        client.screens.as_ref().unwrap().game().language(),
        formats::config::Language::Portuguese
    );

    assert_eq!(std::fs::read(directory.settings()).unwrap(), previous);

    assert!(client.failure.is_none());
}

#[test]
fn settings_write_failure_after_language_preparation_keeps_the_current_language() {
    let directory = SettingsDirectory::new();

    let mut client = language_client(&directory);

    let previous = std::fs::read(directory.settings()).unwrap();

    client.runtime.game_settings.as_mut().unwrap().path = directory.0.join("missing/settings.json");

    request_language(&mut client, formats::config::Language::English);

    client.apply_game_settings();

    let panel = client.runtime.game_settings.as_ref().unwrap();

    assert_eq!(panel.applied, preferences());

    assert_eq!(panel.draft, preferences());

    assert!(panel.visible);

    assert_eq!(
        client.screens.as_ref().unwrap().game().language(),
        formats::config::Language::Portuguese
    );

    assert_eq!(std::fs::read(directory.settings()).unwrap(), previous);
}

#[test]
fn staged_language_files_support_lazy_english_fallback_after_source_files_change() {
    let directory = SettingsDirectory::new();

    let client = language_client(&directory);

    let resources = crate::language::Resources::load(
        client.screens.as_ref().unwrap().game(),
        formats::config::Language::Portuguese,
    )
    .unwrap();

    std::fs::remove_dir_all(directory.0.join("game/Lang")).unwrap();

    assert_eq!(resources.strings.translate("uu_1"), "PT label");

    assert_eq!(
        resources.strings.translate("ff_Caption.txt"),
        "English fallback<n>"
    );

    assert!(
        crate::language::Resources::load(&resources.game, formats::config::Language::English)
            .is_err()
    );
}

#[test]
fn incomplete_language_pack_cannot_discard_the_current_ui_tables() {
    let directory = SettingsDirectory::new();

    std::fs::create_dir_all(directory.0.join("Lang/PT")).unwrap();

    std::fs::create_dir_all(directory.0.join("Lang/EN")).unwrap();

    std::fs::write(directory.0.join("Lang/PT/UIString.txt"), b"1 Interface\n").unwrap();

    let mut game = assets::GameDir::open(&directory.0).unwrap();

    game.set_language(formats::config::Language::Portuguese);

    assert!(crate::language::Resources::load(&game, formats::config::Language::English).is_err());

    assert_eq!(game.language(), formats::config::Language::Portuguese);

    assert_eq!(crate::ui::strings(&game).translate("uu_1"), "Interface");
}

#[test]
fn invalid_language_tables_cannot_replace_active_resources_or_saved_settings() {
    for (name, bytes) in [
        ("UIString.txt", b"".as_slice()),
        ("UIString.txt", b"not a string table".as_slice()),
        ("strdef.bin", b"truncated".as_slice()),
    ] {
        let directory = SettingsDirectory::new();

        let mut client = language_client(&directory);

        let previous = std::fs::read(directory.settings()).unwrap();

        std::fs::write(directory.0.join("game/Lang/EN").join(name), bytes).unwrap();

        let mut game = assets::GameDir::open(directory.0.join("game")).unwrap();

        game.set_language(formats::config::Language::Portuguese);

        *client.screens.as_mut().unwrap().game_mut() = game;

        request_language(&mut client, formats::config::Language::English);

        assert!(
            client.commit_game_settings().is_err(),
            "accepted invalid {name}"
        );

        let panel = client.runtime.game_settings.as_ref().unwrap();

        assert_eq!(panel.applied, preferences());

        assert_eq!(panel.draft, preferences());

        let game = client.screens.as_ref().unwrap().game();

        assert_eq!(game.language(), formats::config::Language::Portuguese);

        assert_eq!(crate::ui::strings(game).translate("uu_1"), "PT label");

        assert_eq!(std::fs::read(directory.settings()).unwrap(), previous);
    }
}
