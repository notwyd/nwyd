use assets::GameDir;
use formats::config::Language;

const LANGUAGES: [Language; 8] = [
    Language::Portuguese,
    Language::English,
    Language::Korean,
    Language::TraditionalChinese,
    Language::SimplifiedChinese,
    Language::Spanish,
    Language::Turkish,
    Language::Polish,
];

pub fn default_language() -> Language {
    Language::Portuguese
}

pub fn available_languages(game: &GameDir) -> Vec<Language> {
    let mut present = [false; LANGUAGES.len()];

    for directory in game.directories() {
        let Some((parent, name)) = directory.split_once('/') else {
            continue;
        };

        if !parent.eq_ignore_ascii_case("Lang") || name.contains('/') {
            continue;
        }

        if let Some(index) = LANGUAGES
            .iter()
            .position(|language| language.dir().eq_ignore_ascii_case(name))
        {
            present[index] = true;
        }
    }

    LANGUAGES
        .into_iter()
        .zip(present)
        .filter_map(|(language, present)| present.then_some(language))
        .collect()
}

pub fn language_label(language: Language) -> &'static str {
    match language {
        Language::Korean => "한국어",
        Language::English => "English",
        Language::Portuguese => "Português",
        Language::TraditionalChinese => "繁體中文",
        Language::SimplifiedChinese => "简体中文",
        Language::Spanish => "Español",
        Language::Turkish => "Türkçe",
        Language::Polish => "Polski",
    }
}

pub fn resolve_language(preferred: Language, available: &[Language]) -> Language {
    [preferred, Language::Portuguese, Language::English]
        .into_iter()
        .find(|language| available.contains(language))
        .or_else(|| available.first().copied())
        .unwrap_or_else(default_language)
}

pub(crate) mod serde {

    use super::{LANGUAGES, Language};
    use ::serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S>(language: &Language, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(language.dir())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Language, D::Error>
    where
        D: Deserializer<'de>,
    {
        let code = String::deserialize(deserializer)?;

        LANGUAGES
            .into_iter()
            .find(|language| language.dir().eq_ignore_ascii_case(&code))
            .ok_or_else(|| {
                ::serde::de::Error::unknown_variant(
                    &code,
                    &["PT", "EN", "KR", "TC", "SC", "SP", "TK", "PL"],
                )
            })
    }
}

#[cfg(test)]
mod tests {

    use super::*;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            static NEXT: AtomicUsize = AtomicUsize::new(0);

            loop {
                let path = std::env::temp_dir().join(format!(
                    "nwyd-language-test-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                ));

                match std::fs::create_dir(&path) {
                    Ok(()) => return Self(path),
                    Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                    Err(error) => panic!("creating language test directory: {error}"),
                }
            }
        }

        fn game(&self) -> GameDir {
            GameDir::open(&self.0).unwrap()
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn empty_language_directories_are_discovered_case_insensitively_in_stable_order() {
        let directory = TestDirectory::new();

        for code in ["pl", "TK", "sP", "sc", "tC", "kr", "eN", "pT"] {
            std::fs::create_dir_all(directory.0.join("lAnG").join(code)).unwrap();
        }

        let game = directory.game();

        assert!(game.is_empty());

        assert_eq!(available_languages(&game), LANGUAGES);
    }

    #[test]
    fn discovery_ignores_files_unknown_codes_and_nested_language_directories() {
        let directory = TestDirectory::new();

        std::fs::create_dir_all(directory.0.join("Lang/unknown/PT")).unwrap();

        std::fs::create_dir_all(directory.0.join("Lang/PT")).unwrap();

        std::fs::create_dir_all(directory.0.join("other/EN")).unwrap();

        std::fs::write(directory.0.join("Lang/EN"), []).unwrap();

        assert_eq!(
            available_languages(&directory.game()),
            [Language::Portuguese]
        );
    }

    #[test]
    fn discovery_returns_no_languages_when_lang_is_absent_or_a_file() {
        let directory = TestDirectory::new();

        assert!(available_languages(&directory.game()).is_empty());

        std::fs::write(directory.0.join("Lang"), []).unwrap();

        assert!(available_languages(&directory.game()).is_empty());
    }

    #[test]
    fn discovery_uses_browser_file_paths_with_the_same_language_order() {
        let game = GameDir::from_files(
            "selected-game",
            [
                ("lAnG/eN/labels.txt".into(), vec![]),
                ("Lang/pT/labels.txt".into(), vec![]),
                ("Lang/unknown/KR/labels.txt".into(), vec![]),
                ("other/PL/labels.txt".into(), vec![]),
                ("Lang/SC".into(), vec![]),
            ],
        )
        .unwrap();

        assert_eq!(
            available_languages(&game),
            [Language::Portuguese, Language::English]
        );
    }

    #[test]
    fn language_fallback_prefers_saved_choice_then_portuguese_then_english() {
        use Language::{English, Polish, Portuguese, Spanish};

        assert_eq!(
            resolve_language(Polish, &[English, Polish, Portuguese]),
            Polish
        );

        assert_eq!(resolve_language(Polish, &[English, Portuguese]), Portuguese);

        assert_eq!(resolve_language(Polish, &[Spanish, English]), English);

        assert_eq!(resolve_language(Polish, &[Spanish]), Spanish);

        assert_eq!(resolve_language(Polish, &[]), Portuguese);

        assert_eq!(default_language(), Portuguese);
    }

    #[derive(Debug, PartialEq, Eq, ::serde::Serialize, ::serde::Deserialize)]
    struct SavedLanguage {
        #[serde(default = "default_language", with = "super::serde")]
        language: Language,
    }

    #[test]
    fn language_settings_emit_uppercase_codes_and_accept_lowercase_codes() {
        for language in LANGUAGES {
            let saved = SavedLanguage { language };

            assert_eq!(
                serde_json::to_value(&saved).unwrap(),
                serde_json::json!({ "language": language.dir() })
            );

            assert_eq!(
                serde_json::from_value::<SavedLanguage>(serde_json::json!({
                    "language": language.dir().to_ascii_lowercase()
                }))
                .unwrap(),
                saved
            );
        }

        assert_eq!(
            serde_json::from_str::<SavedLanguage>("{}")
                .unwrap()
                .language,
            Language::Portuguese
        );

        for value in [serde_json::json!("unknown"), serde_json::json!(3)] {
            assert!(
                serde_json::from_value::<SavedLanguage>(serde_json::json!({
                    "language": value
                }))
                .is_err()
            );
        }
    }
}
