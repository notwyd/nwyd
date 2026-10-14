use assets::GameDir;
use audio::{Engine, Loader};
use formats::audio::{music, sound_list};
use formats::config::config_bin::Config;

pub const MUSIC_LOGIN: usize = 0;

pub const AMBIENCE: i64 = 0x66;

pub const AMBIENCE_STOPPED: i64 = 0x65;

pub const SOUND_LIST: &[u8] = b"sound\\soundlist.txt";

pub const MUSIC_TABLE: &[u8] = b"Music.txt";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]

pub struct UnconditionalSound {
    slot: i64,
}

impl UnconditionalSound {
    pub const fn new(slot: i64) -> Self {
        Self { slot }
    }

    pub const fn slot(self) -> i64 {
        self.slot
    }
}

#[derive(Debug, Clone)]

pub struct Assets(GameDir);

impl Assets {
    pub fn new(game: &GameDir) -> Assets {
        Assets(game.clone())
    }
}

impl Loader for Assets {
    fn read(&self, path: &str) -> Option<Vec<u8>> {
        self.0.read(path.as_bytes()).ok()
    }
}

#[derive(Debug)]

pub struct SceneAudio {
    engine: Engine,
    assets: Assets,
    pub notes: Vec<String>,
}

impl SceneAudio {
    pub fn open(game: &GameDir, config: &Config, silent: bool) -> SceneAudio {
        let mut notes = Vec::new();

        let sounds = match game.read(SOUND_LIST) {
            Ok(bytes) => match sound_list::parse(&bytes) {
                Ok(list) => list,
                Err(e) => {
                    notes.push(format!(
                        "soundlist.txt will not parse ({e}); running silent"
                    ));

                    sound_list::SoundList::default()
                }
            },
            Err(_) => {
                notes.push("soundlist.txt is not in this install; sound effects are off".into());

                sound_list::SoundList::default()
            }
        };

        let table = match game.read(MUSIC_TABLE) {
            Ok(bytes) => music::parse(&bytes).unwrap_or_else(|e| {
                notes.push(format!(
                    "Music.txt will not parse ({e}); using the built-in table"
                ));

                music::MusicTable::default()
            }),
            Err(_) => music::MusicTable::default(),
        };

        let sfx = config.sfx_volume() as i32;

        let bgm = config.music_volume() as i32;

        let engine = if silent {
            Engine::silent(sounds, table, sfx, bgm)
        } else {
            Engine::open(sounds, table, sfx, bgm)
        };

        if !silent && engine.output().is_none() {
            notes.push("no audio device; the scene runs silent".into());
        }

        SceneAudio {
            engine,
            assets: Assets::new(game),
            notes,
        }
    }

    pub fn engine(&self) -> &Engine {
        &self.engine
    }

    pub fn assets(&self) -> &Assets {
        &self.assets
    }

    pub fn audible(&self) -> bool {
        self.engine.output().is_some()
    }

    pub fn enter(&mut self) {
        self.engine.stop_music();

        if !self.engine.play_music(MUSIC_LOGIN, &self.assets) {
            self.notes
                .push("music slot 0 will not play; the screen is quiet".into());
        }

        self.engine.play_looping(AMBIENCE, &self.assets);

        self.engine.stop(AMBIENCE_STOPPED);
    }

    pub fn switch_music(&mut self, slot: usize) -> bool {
        if slot >= music::SLOTS {
            return false;
        }

        self.engine.stop_music();

        self.engine.play_music(slot, &self.assets)
    }

    pub fn play_once(&self, slot: i64) {
        if !self.engine.is_playing(slot) {
            self.engine.play(slot, &self.assets);
        }
    }

    pub fn play_unconditional(&self, request: UnconditionalSound) {
        self.engine.play(request.slot(), &self.assets);
    }

    pub fn play_damage(&self, request: super::skill_anim::DamageSoundRequest) -> bool {
        let mut mixer = self.engine.mixer();

        if mixer.bank.sound(request.slot, &self.assets).is_err()
            || request.slot == super::skill_anim::DAMAGE_SOUND_SUPPRESSED
        {
            return false;
        }

        if !request.should_play(mixer.is_playing(request.slot)) {
            return false;
        }

        mixer.play(request.slot, &self.assets).is_ok()
    }

    pub fn leave(&mut self) {
        self.engine.stop(AMBIENCE);
    }

    pub fn leave_field(&mut self) {
        self.engine.stop(107);

        self.engine.stop(6);
    }

    pub fn set_active(&self, active: bool) {
        self.engine.set_active(active);
    }

    pub fn set_volumes(&self, sound: u8, music: u8) {
        let mut mixer = self.engine.mixer();

        mixer.bank.set_volume(if sound == 0 {
            audio::volume::MIN
        } else {
            audio::volume::sfx(i32::from(sound.min(100)))
        });

        mixer.set_music_volume(if music == 0 {
            audio::volume::MIN
        } else {
            audio::volume::music(i32::from(music.min(100)))
        });
    }
}
