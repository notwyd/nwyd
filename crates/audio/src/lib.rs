use formats::audio::{MusicTable, SoundList};
use std::sync::{Arc, Mutex, MutexGuard};

pub mod bank;
pub mod device;
pub mod mixer;
pub mod music;
pub mod volume;
pub mod wave;

pub use bank::{Bank, Loader, Refusal};
pub use mixer::Mixer;
pub use music::Track;
pub use wave::Wave;

pub struct Engine {
    mixer: Arc<Mutex<Mixer>>,
    output: Option<device::Output>,
    music: MusicTable,
}

impl std::fmt::Debug for Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Engine")
            .field("output", &self.output)
            .field("silent", &self.output.is_none())
            .finish()
    }
}

impl Engine {
    pub fn open(
        sounds: SoundList,
        music: MusicTable,
        sfx_percent: i32,
        music_percent: i32,
    ) -> Engine {
        let rate = match device::default_config() {
            Ok((rate, _, _)) => rate,
            Err(e) => {
                tracing::info!(error = % e, "no audio device; running silent");

                DEFAULT_RATE
            }
        };

        let bank = Bank::new(sounds, volume::sfx(sfx_percent));

        let mixer = Arc::new(Mutex::new(Mixer::new(
            bank,
            volume::music(music_percent),
            rate,
        )));

        let output = match device::open(Arc::clone(&mixer)) {
            Ok(o) => Some(o),
            Err(e) => {
                tracing::info!(error = % e, "audio output unavailable; running silent");

                None
            }
        };

        Engine {
            mixer,
            output,
            music,
        }
    }

    pub fn silent(
        sounds: SoundList,
        music: MusicTable,
        sfx_percent: i32,
        music_percent: i32,
    ) -> Engine {
        let bank = Bank::new(sounds, volume::sfx(sfx_percent));

        Engine {
            mixer: Arc::new(Mutex::new(Mixer::new(
                bank,
                volume::music(music_percent),
                DEFAULT_RATE,
            ))),
            output: None,
            music,
        }
    }

    pub fn output(&self) -> Option<&device::Output> {
        self.output.as_ref()
    }

    pub fn music_table(&self) -> &MusicTable {
        &self.music
    }

    pub fn mixer(&self) -> MutexGuard<'_, Mixer> {
        self.mixer.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn play(&self, slot: i64, loader: &dyn Loader) {
        let _ = self.mixer().play(slot, loader);
    }

    pub fn play_looping(&self, slot: i64, loader: &dyn Loader) {
        let _ = self.mixer().play_looping(slot, loader);
    }

    pub fn stop(&self, slot: i64) {
        self.mixer().stop(slot);
    }

    pub fn is_playing(&self, slot: i64) -> bool {
        self.mixer().is_playing(slot)
    }

    pub fn release(&self, slot: i64) {
        self.mixer().bank.release(slot);
    }

    pub fn play_music(&self, slot: usize, loader: &dyn Loader) -> bool {
        if slot >= formats::audio::music::SLOTS {
            return false;
        }

        let path = match self.music.resolve(slot, |p| loader.read(p).is_some()) {
            Some(p) => p.to_string(),
            None => {
                tracing::debug!(slot, "music slot names no playable path");

                return false;
            }
        };

        let Some(bytes) = loader.read(&path) else {
            tracing::debug!(slot, % path, "music file is not in the install");

            return false;
        };

        match Track::open(bytes, &path) {
            Ok(track) => {
                self.mixer().play_music(slot, track);

                true
            }
            Err(e) => {
                tracing::warn!(slot, % path, error = % e, "music will not play");

                false
            }
        }
    }

    pub fn stop_music(&self) {
        self.mixer().stop_music();
    }

    pub fn set_active(&self, active: bool) {
        self.mixer().set_active(active);
    }
}

pub const DEFAULT_RATE: u32 = 22_050;

#[cfg(test)]
mod tests;
