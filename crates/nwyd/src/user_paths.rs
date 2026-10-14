use anyhow::{Context, Result};
use assets::GameDir;
#[cfg(not(target_arch = "wasm32"))]
use std::{fs, io::Write};
use std::{
    io::ErrorKind,
    path::{Path, PathBuf},
};

#[cfg(not(target_arch = "wasm32"))]

fn config_base() -> Result<PathBuf> {
    dirs::config_dir().context("locating the user configuration directory")
}

#[cfg(target_arch = "wasm32")]

fn config_base() -> Result<PathBuf> {
    // These paths are browser-storage keys, never host filesystem paths.
    Ok(PathBuf::from("browser"))
}

fn app_directory(base: &Path) -> Result<PathBuf> {
    let path = base.join("nwyd");

    #[cfg(not(target_arch = "wasm32"))]
    fs::create_dir_all(&path).with_context(|| format!("creating {}", path.display()))?;

    Ok(path)
}

fn profile_directory(game: &GameDir, base: &Path) -> Result<PathBuf> {
    let root = if game.is_memory_backed() {
        game.root().to_owned()
    } else {
        game.root()
            .canonicalize()
            .context("resolving the selected game directory")?
    };

    let hash = root
        .as_os_str()
        .as_encoded_bytes()
        .iter()
        .fold(0xcbf29ce484222325_u64, |hash, byte| {
            (hash ^ u64::from(*byte)).wrapping_mul(0x100000001b3)
        });

    let path = app_directory(base)?
        .join("profiles")
        .join(format!("{hash:016x}"));

    #[cfg(not(target_arch = "wasm32"))]
    fs::create_dir_all(&path).with_context(|| format!("creating {}", path.display()))?;

    Ok(path)
}

fn settings_at(base: &Path) -> Result<PathBuf> {
    Ok(app_directory(base)?.join(crate::game_settings::FILE))
}

pub fn settings() -> Result<PathBuf> {
    settings_at(&config_base()?)
}

pub fn legacy_settings(game: &GameDir) -> PathBuf {
    game.resolve(crate::game_settings::FILE.as_bytes())
        .unwrap_or_else(|| game.root().join(crate::game_settings::FILE))
}

fn guild_mark_at(game: &GameDir, base: &Path) -> Result<PathBuf> {
    let path = profile_directory(game, base)?.join(formats::config::ini::guild_mark::FILE);

    #[cfg(not(target_arch = "wasm32"))]
    if path.try_exists()? {
        return Ok(path);
    }

    #[cfg(target_arch = "wasm32")]
    match read(&path) {
        Ok(_) => return Ok(path),
        Err(error) if error.kind() == ErrorKind::NotFound => {}
        Err(error) => return Err(error).context("reading user guild-mark preferences"),
    }

    let bytes = match game.read(formats::config::ini::guild_mark::FILE.as_bytes()) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(path),
        Err(error) => return Err(error).context("reading existing guild-mark preferences"),
    };

    #[cfg(not(target_arch = "wasm32"))]
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
    {
        Ok(mut file) => {
            if let Err(error) = file.write_all(&bytes) {
                drop(file);

                let _ = fs::remove_file(&path);

                return Err(error).context("seeding user guild-mark preferences");
            }
        }
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error).context("creating user guild-mark preferences"),
    }

    #[cfg(target_arch = "wasm32")]
    write(&path, &bytes).context("seeding user guild-mark preferences")?;

    Ok(path)
}

pub fn guild_mark(game: &GameDir) -> Result<PathBuf> {
    guild_mark_at(game, &config_base()?)
}

fn quick_slots_at(game: &GameDir, base: &Path) -> Result<PathBuf> {
    Ok(profile_directory(game, base)?.join("NQuickSLot_01.bin"))
}

pub fn quick_slots(game: &GameDir) -> Result<PathBuf> {
    quick_slots_at(game, &config_base()?)
}

fn read_quick_slots_at(game: &GameDir, base: &Path) -> Result<Vec<u8>> {
    let path = quick_slots_at(game, base)?;

    match read(&path) {
        Ok(bytes) => Ok(bytes),
        Err(error) if error.kind() == ErrorKind::NotFound => {
            game.read(crate::scene::quick_bar::FILE).map_err(Into::into)
        }
        Err(error) => Err(error).with_context(|| format!("reading {}", path.display())),
    }
}

pub fn read_quick_slots(game: &GameDir) -> Result<Vec<u8>> {
    read_quick_slots_at(game, &config_base()?)
}

pub fn write_quick_slots(game: &GameDir, bytes: &[u8]) -> Result<()> {
    let path = quick_slots(game)?;

    write(&path, bytes).with_context(|| format!("writing {}", path.display()))
}

#[cfg(not(target_arch = "wasm32"))]

pub(crate) fn read(path: &Path) -> std::io::Result<Vec<u8>> {
    fs::read(path)
}

#[cfg(not(target_arch = "wasm32"))]

pub(crate) fn write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    fs::write(path, bytes)
}

pub(crate) fn read_to_string(path: &Path) -> std::io::Result<String> {
    String::from_utf8(read(path)?)
        .map_err(|error| std::io::Error::new(ErrorKind::InvalidData, error))
}

#[cfg(target_arch = "wasm32")]
mod browser;

#[cfg(any(target_arch = "wasm32", test))]
mod browser_store;

#[cfg(target_arch = "wasm32")]
pub(crate) use browser::{read, write};

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests;
