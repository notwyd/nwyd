use super::*;
use std::sync::atomic::{AtomicU64, Ordering};

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);

        loop {
            let path = std::env::temp_dir().join(format!(
                "nwyd-user-paths-test-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));

            match fs::create_dir(&path) {
                Ok(()) => return Self(path),
                Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
                Err(error) => panic!("creating test directory: {error}"),
            }
        }
    }

    fn game(&self, name: &str) -> PathBuf {
        let path = self.0.join(name);

        fs::create_dir_all(&path).unwrap();

        path
    }

    fn config(&self) -> PathBuf {
        self.0.join("user-config")
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]

fn settings_create_an_application_directory_outside_game_files() {
    let root = TestDirectory::new();

    let game_root = root.game("game");

    let game = GameDir::open(&game_root).unwrap();

    let path = settings_at(&root.config()).unwrap();

    assert_eq!(path, root.config().join("nwyd/nwyd-settings.json"));

    assert!(path.parent().unwrap().is_dir());

    assert!(!path.starts_with(game.root()));

    fs::write(&path, b"synthetic preferences").unwrap();

    assert_eq!(fs::read_dir(&game_root).unwrap().count(), 0);
}

#[test]

fn game_profiles_are_isolated_and_canonical_paths_share_a_profile() {
    let root = TestDirectory::new();

    let first_root = root.game("first/game");

    let second_root = root.game("second/game");

    let first = GameDir::open(&first_root).unwrap();

    let alias = GameDir::open(first_root.join(".")).unwrap();

    let second = GameDir::open(&second_root).unwrap();

    let first_path = quick_slots_at(&first, &root.config()).unwrap();

    let second_path = quick_slots_at(&second, &root.config()).unwrap();

    assert_ne!(first_path, second_path);

    assert_eq!(first_path, quick_slots_at(&alias, &root.config()).unwrap());

    assert!(first_path.starts_with(root.config().join("nwyd/profiles")));

    assert!(second_path.starts_with(root.config().join("nwyd/profiles")));

    fs::write(&first_path, b"first profile").unwrap();

    fs::write(&second_path, b"second profile").unwrap();

    assert_eq!(
        read_quick_slots_at(&first, &root.config()).unwrap(),
        b"first profile"
    );

    assert_eq!(
        read_quick_slots_at(&second, &root.config()).unwrap(),
        b"second profile"
    );

    assert_eq!(fs::read_dir(&first_root).unwrap().count(), 0);

    assert_eq!(fs::read_dir(&second_root).unwrap().count(), 0);
}

#[test]

fn quick_slots_fall_back_to_game_defaults_until_a_user_copy_exists() {
    let root = TestDirectory::new();

    let game_root = root.game("game");

    let legacy = game_root.join("nquickslot_01.BIN");

    fs::write(&legacy, b"synthetic game defaults").unwrap();

    let game = GameDir::open(&game_root).unwrap();

    assert_eq!(
        read_quick_slots_at(&game, &root.config()).unwrap(),
        b"synthetic game defaults"
    );

    let path = quick_slots_at(&game, &root.config()).unwrap();

    assert!(!path.exists());

    fs::write(&path, b"synthetic user bindings").unwrap();

    assert_eq!(
        read_quick_slots_at(&game, &root.config()).unwrap(),
        b"synthetic user bindings"
    );

    assert_eq!(fs::read(&legacy).unwrap(), b"synthetic game defaults");

    assert_eq!(fs::read_dir(&game_root).unwrap().count(), 1);
}

#[test]

fn quick_slots_do_not_hide_errors_in_an_existing_user_copy() {
    let root = TestDirectory::new();

    let game_root = root.game("game");

    fs::write(game_root.join("NQuickSLot_01.bin"), b"defaults").unwrap();

    let game = GameDir::open(&game_root).unwrap();

    let path = quick_slots_at(&game, &root.config()).unwrap();

    fs::create_dir(&path).unwrap();

    assert!(read_quick_slots_at(&game, &root.config()).is_err());
}

#[test]

fn guild_mark_defaults_are_seeded_once_and_game_files_remain_unchanged() {
    let root = TestDirectory::new();

    let game_root = root.game("game");

    let legacy = game_root.join("WGMRK.INI");

    fs::write(&legacy, b"[GuildMark]\r\noff=1\r\n").unwrap();

    let game = GameDir::open(&game_root).unwrap();

    let path = guild_mark_at(&game, &root.config()).unwrap();

    assert!(!path.starts_with(&game_root));

    assert_eq!(fs::read(&path).unwrap(), b"[GuildMark]\r\noff=1\r\n");

    fs::write(&path, b"[GuildMark]\r\noff=0\r\n").unwrap();

    assert_eq!(path, guild_mark_at(&game, &root.config()).unwrap());

    assert_eq!(fs::read(&path).unwrap(), b"[GuildMark]\r\noff=0\r\n");

    assert_eq!(fs::read(&legacy).unwrap(), b"[GuildMark]\r\noff=1\r\n");

    assert_eq!(fs::read_dir(&game_root).unwrap().count(), 1);
}

#[test]

fn absent_guild_mark_defaults_use_only_the_user_profile() {
    let root = TestDirectory::new();

    let game_root = root.game("game");

    let game = GameDir::open(&game_root).unwrap();

    let path = guild_mark_at(&game, &root.config()).unwrap();

    assert!(path.parent().unwrap().is_dir());

    assert!(!path.exists());

    fs::write(&path, b"[GuildMark]\r\noff=0\r\n").unwrap();

    assert_eq!(fs::read_dir(&game_root).unwrap().count(), 0);
}

#[test]

fn legacy_preferences_are_resolved_without_writing_or_removing_them() {
    let root = TestDirectory::new();

    let game_root = root.game("game");

    let legacy = game_root.join("NWYD-SETTINGS.JSON");

    fs::write(&legacy, b"synthetic legacy preferences").unwrap();

    let game = GameDir::open(&game_root).unwrap();

    assert_eq!(legacy_settings(&game), legacy);

    assert_eq!(fs::read(&legacy).unwrap(), b"synthetic legacy preferences");

    let empty_root = root.game("empty");

    let empty = GameDir::open(&empty_root).unwrap();

    assert_eq!(
        legacy_settings(&empty),
        empty_root.join("nwyd-settings.json")
    );

    assert_eq!(fs::read_dir(&empty_root).unwrap().count(), 0);
}

#[test]

fn virtual_game_profiles_import_defaults_without_requiring_a_host_directory() {
    let root = TestDirectory::new();

    let game = GameDir::from_files(
        "selected-game",
        [
            ("NQuickSLot_01.bin".into(), b"default bindings".to_vec()),
            ("WGMRK.INI".into(), b"[GuildMark]\r\noff=1\r\n".to_vec()),
        ],
    )
    .unwrap();

    assert_eq!(
        read_quick_slots_at(&game, &root.config()).unwrap(),
        b"default bindings"
    );

    let slots = quick_slots_at(&game, &root.config()).unwrap();

    write(&slots, b"user bindings").unwrap();

    assert_eq!(
        read_quick_slots_at(&game, &root.config()).unwrap(),
        b"user bindings"
    );

    assert_eq!(
        game.read(b"NQuickSLot_01.bin").unwrap(),
        b"default bindings"
    );

    let mark = guild_mark_at(&game, &root.config()).unwrap();

    assert_eq!(read(&mark).unwrap(), b"[GuildMark]\r\noff=1\r\n");

    write(&mark, b"user mark").unwrap();

    assert_eq!(
        read(&guild_mark_at(&game, &root.config()).unwrap()).unwrap(),
        b"user mark"
    );
}

#[cfg(unix)]
#[test]

fn preferences_and_bindings_work_with_a_read_only_game_directory() {
    use std::os::unix::fs::PermissionsExt;

    struct RestorePermissions(PathBuf, fs::Permissions);

    impl Drop for RestorePermissions {
        fn drop(&mut self) {
            let _ = fs::set_permissions(&self.0, self.1.clone());
        }
    }

    let root = TestDirectory::new();

    let game_root = root.game("read-only-game");

    let preferences =
        crate::game_settings::Preferences::from_config(&formats::config::Config::default());

    let legacy = game_root.join(crate::game_settings::FILE);

    preferences.save(&legacy).unwrap();

    fs::write(
        game_root.join("NQuickSLot_01.bin"),
        b"synthetic default bindings",
    )
    .unwrap();

    let permissions = fs::metadata(&game_root).unwrap().permissions();

    let _restore = RestorePermissions(game_root.clone(), permissions);

    fs::set_permissions(&game_root, fs::Permissions::from_mode(0o555)).unwrap();

    let game = GameDir::open(&game_root).unwrap();

    let path = settings_at(&root.config()).unwrap();

    let mut loaded = crate::game_settings::Preferences::load_or_import(&path, &legacy)
        .unwrap()
        .unwrap();

    assert_eq!(loaded, preferences);

    loaded.music = 19;

    loaded.language = formats::config::Language::English;

    loaded.save(&path).unwrap();

    assert_eq!(
        crate::game_settings::Preferences::load(&path).unwrap(),
        Some(loaded)
    );

    assert_eq!(
        crate::game_settings::Preferences::load(&legacy).unwrap(),
        Some(preferences)
    );

    assert_eq!(
        read_quick_slots_at(&game, &root.config()).unwrap(),
        b"synthetic default bindings"
    );

    fs::write(
        quick_slots_at(&game, &root.config()).unwrap(),
        b"synthetic user bindings",
    )
    .unwrap();

    fs::write(
        guild_mark_at(&game, &root.config()).unwrap(),
        b"synthetic guild preference",
    )
    .unwrap();

    assert_eq!(
        read_quick_slots_at(&game, &root.config()).unwrap(),
        b"synthetic user bindings"
    );

    assert_eq!(fs::read_dir(&game_root).unwrap().count(), 2);
}
