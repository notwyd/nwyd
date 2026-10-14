use anyhow::{Context, Result, ensure};
use assets::GameDir;
use formats::config::Language;

pub(crate) struct Resources {
    pub game: GameDir,
    pub strings: ui::Strings,
}

impl Resources {
    pub fn load(game: &GameDir, language: Language) -> Result<Self> {
        ensure!(
            crate::game_settings::available_languages(game).contains(&language),
            "the selected language folder is no longer available"
        );

        let previous = game;

        let game = previous
            .snapshot_language(language)
            .context("loading the selected language")?;

        for name in ["UIString.txt", "strdef.bin"] {
            ensure!(
                previous.resolve_lang(name).is_none() || game.resolve_lang(name).is_some(),
                "the selected language and English fallback are missing {name}"
            );
        }

        if game.resolve_lang("strdef.bin").is_some() {
            ensure!(
                game.read_lang("strdef.bin")?.len() >= formats::text::strdef::LEN,
                "the selected language's strdef.bin is truncated"
            );
        }

        let strings = crate::ui::strings(&game);

        ensure!(
            strings
                .ui
                .as_ref()
                .is_none_or(|table| table.populated() > 0),
            "the selected language's UIString.txt contains no usable strings"
        );

        Ok(Self { game, strings })
    }
}
