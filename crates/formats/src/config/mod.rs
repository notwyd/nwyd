pub mod config_bin;
pub mod ini;
pub mod serverlist;
pub mod sn;

pub use config_bin::{Config, CursorMode, Language, PRESETS, Preset};

pub fn language_path(
    language: Language,
    name: &str,
    exists: impl Fn(&str) -> bool,
) -> std::path::PathBuf {
    let preferred = format!("Lang/{}/{}", language.dir(), name);

    if exists(&preferred) {
        return preferred.into();
    }

    format!("Lang/{}/{}", Language::English.dir(), name).into()
}
