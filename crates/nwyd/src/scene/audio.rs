use assets::GameDir;
use audio::{Engine, Loader, PlayResult};
use formats::audio::{music, sound_list};
use formats::config::config_bin::Config;
use std::cell::{Cell, RefCell};
use std::collections::{BTreeMap, VecDeque};

const MAX_PENDING_DAMAGE: usize = 4096;

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
struct DamageFeedback {
    latest: super::skill_anim::DamageSoundRequest,
    queued: VecDeque<(u64, super::skill_anim::DamageSoundRequest)>,
    last_play_ms: u32,
    played: Option<u32>,
}

#[derive(Debug, Default)]
struct DeferredDamage {
    receipts: BTreeMap<u64, super::skill_anim::DamageSoundRequest>,
    feedback: BTreeMap<u16, DamageFeedback>,
}

#[derive(Debug)]
pub struct SceneAudio {
    engine: Engine,
    assets: Assets,
    next_damage_receipt: Cell<u64>,
    pending_damage: RefCell<DeferredDamage>,
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

        let assets = Assets::new(game);

        let engine = if silent {
            Engine::silent(sounds, table, sfx, bgm)
        } else {
            Engine::open(sounds, table, sfx, bgm)
        };

        #[cfg(not(target_arch = "wasm32"))]
        let engine = if silent {
            engine
        } else {
            engine.with_background_loader(assets.clone())
        };

        if !silent && engine.output().is_none() {
            notes.push("no audio device; the scene runs silent".into());
        }

        SceneAudio {
            engine,
            assets,
            next_damage_receipt: Cell::new(0),
            pending_damage: RefCell::default(),
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
        self.engine.play_once(slot, &self.assets);
    }

    pub fn play_unconditional(&self, request: UnconditionalSound) {
        self.engine.play(request.slot(), &self.assets);
    }

    pub fn play_damage(&self, request: super::skill_anim::DamageSoundRequest) -> Option<bool> {
        if request.previous_kind == -1 {
            self.cancel_entity_damage(request.entity);
        }

        let receipt = self.next_damage_receipt.get().wrapping_add(1).max(1);

        self.next_damage_receipt.set(receipt);

        let mut pending = self.pending_damage.borrow_mut();

        let capacity = pending.receipts.len()
            + pending
                .feedback
                .values()
                .map(|feedback| feedback.queued.len())
                .sum::<usize>()
            < MAX_PENDING_DAMAGE;

        if let Some(feedback) = pending.feedback.get_mut(&request.entity) {
            feedback.latest = request;

            if self.engine.mixer().bank.volume() <= audio::volume::MIN {
                return None;
            }

            if capacity {
                feedback.queued.push_back((receipt, request));

                self.engine.preload(request.slot, &self.assets);
            } else {
                tracing::warn!("too many pending damage sound requests");
            }

            return None;
        }

        if !capacity {
            return Some(false);
        }

        match self.start_damage(request, receipt) {
            PlayResult::Ready(played) => Some(played),
            PlayResult::Pending => {
                pending.receipts.insert(receipt, request);

                pending.feedback.insert(
                    request.entity,
                    DamageFeedback {
                        latest: request,
                        queued: VecDeque::new(),
                        last_play_ms: request.last_play_ms,
                        played: None,
                    },
                );

                None
            }
        }
    }

    fn start_damage(
        &self,
        request: super::skill_anim::DamageSoundRequest,
        receipt: u64,
    ) -> PlayResult {
        if request.slot == super::skill_anim::DAMAGE_SOUND_SUPPRESSED {
            self.engine.preload(request.slot, &self.assets);

            return PlayResult::Ready(false);
        }

        self.engine.play_checked(
            request.slot,
            &self.assets,
            request.should_play(true),
            receipt,
        )
    }

    pub fn take_damage_results(&self) -> Vec<(super::skill_anim::DamageSoundRequest, bool)> {
        let results = self.engine.take_playback_results();

        let mut pending = self.pending_damage.borrow_mut();

        let mut completed = Vec::new();

        for (receipt, played) in results {
            let Some(request) = pending.receipts.remove(&receipt) else {
                continue;
            };

            let Some(mut feedback) = pending.feedback.remove(&request.entity) else {
                continue;
            };

            if played {
                feedback.last_play_ms = request.now_ms;

                feedback.played = Some(request.now_ms);
            }

            let mut waiting = false;

            while let Some((receipt, mut request)) = feedback.queued.pop_front() {
                if feedback.played != Some(request.now_ms) {
                    request.last_play_ms = feedback.last_play_ms;
                }

                match self.start_damage(request, receipt) {
                    PlayResult::Ready(true) => {
                        feedback.last_play_ms = request.now_ms;

                        feedback.played = Some(request.now_ms);
                    }
                    PlayResult::Ready(false) => {}
                    PlayResult::Pending => {
                        pending.receipts.insert(receipt, request);

                        waiting = true;

                        break;
                    }
                }
            }

            if waiting {
                pending.feedback.insert(request.entity, feedback);

                continue;
            }

            let mut latest = feedback.latest;

            if let Some(now_ms) = feedback.played {
                latest.now_ms = now_ms;
            }

            completed.push((latest, feedback.played.is_some()));
        }

        completed
    }

    pub fn cancel_pending_damage(&self) {
        let pending = std::mem::take(&mut *self.pending_damage.borrow_mut());

        for (receipt, _) in pending.receipts {
            self.engine.cancel_pending(receipt);
        }
    }

    pub fn cancel_pending_sounds(&self) {
        self.cancel_pending_damage();

        self.engine.cancel_pending_sounds();
    }

    fn cancel_entity_damage(&self, entity: u16) {
        let mut pending = self.pending_damage.borrow_mut();

        pending.feedback.remove(&entity);

        pending.receipts.retain(|&receipt, request| {
            if request.entity != entity {
                return true;
            }

            self.engine.cancel_pending(receipt);

            false
        });
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

#[cfg(test)]
mod tests;
