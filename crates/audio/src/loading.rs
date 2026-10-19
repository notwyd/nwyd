use crate::bank::{Refusal, Sound};
use crate::{Bank, Loader, Mixer, PlayResult, Track};
use formats::audio::{
    MusicTable, music,
    sound_list::{Entry, Slot},
};
use std::{
    collections::{BTreeMap, VecDeque},
    sync::{Arc, Condvar, Mutex},
};

const MAX_PENDING_PLAYS: usize = 4_096;

#[derive(Clone, Copy)]
pub(crate) enum Action {
    Prepare,
    Play { overlap: bool },
    Loop,
}

impl Action {
    fn applies(self, playing: bool) -> bool {
        match self {
            Self::Prepare => false,
            Self::Play { overlap } => overlap || !playing,
            Self::Loop => !playing,
        }
    }

    pub(crate) fn apply(self, sound: &mut Sound) -> bool {
        if !self.applies(sound.is_playing()) {
            return false;
        }
        sound.play(matches!(self, Self::Loop));
        true
    }
}

struct Request {
    action: Action,
    receipt: Option<u64>,
}

struct Pending {
    generation: u64,
    requests: Vec<Request>,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Key {
    Sound(Slot),
    Music,
}

struct Job {
    generation: u64,
    kind: JobKind,
}

enum JobKind {
    Sound(Slot, Entry),
    Music(usize, MusicTable),
}

impl Job {
    fn key(&self) -> Key {
        match &self.kind {
            JobKind::Sound(slot, _) => Key::Sound(*slot),
            JobKind::Music(..) => Key::Music,
        }
    }
}

struct Loaded {
    generation: u64,
    value: LoadedValue,
}

enum LoadedValue {
    Sound(Slot, Result<Sound, Refusal>),
    Music(usize, Result<Track, crate::music::Error>),
}

impl Loaded {
    fn key(&self) -> Key {
        match &self.value {
            LoadedValue::Sound(slot, _) => Key::Sound(*slot),
            LoadedValue::Music(..) => Key::Music,
        }
    }
}

#[derive(Default)]
struct Queue {
    jobs: VecDeque<Job>,
    ready: VecDeque<Loaded>,
    stopped: bool,
}

struct Shared {
    queue: Mutex<Queue>,
    wake: Condvar,
}

struct Worker {
    shared: Arc<Shared>,
}

impl Worker {
    fn new(loader: impl Loader + 'static) -> std::io::Result<Self> {
        let shared = Arc::new(Shared {
            queue: Mutex::new(Queue::default()),
            wake: Condvar::new(),
        });
        let work = shared.clone();
        std::thread::Builder::new()
            .name("nwyd-audio-load".into())
            .spawn(move || {
                loop {
                    let job = {
                        let mut queue = work.queue.lock().unwrap_or_else(|e| e.into_inner());
                        while !queue.stopped && queue.jobs.is_empty() {
                            queue = work.wake.wait(queue).unwrap_or_else(|e| e.into_inner());
                        }
                        if queue.stopped {
                            return;
                        }
                        queue.jobs.pop_front().unwrap()
                    };
                    let value = match job.kind {
                        JobKind::Sound(slot, entry) => {
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    Bank::load_entry(slot, &entry, &loader)
                                }))
                                .unwrap_or_else(|_| Err(Refusal::Undecodable(entry.path())));
                            LoadedValue::Sound(slot, result)
                        }
                        JobKind::Music(slot, table) => {
                            let result =
                                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                    open_music(slot, &table, &loader)
                                }))
                                .unwrap_or_else(|_| {
                                    Err(crate::music::Error::Undecodable(
                                        "music loader panicked".into(),
                                    ))
                                });
                            LoadedValue::Music(slot, result)
                        }
                    };
                    let loaded = Loaded {
                        generation: job.generation,
                        value,
                    };
                    let mut queue = work.queue.lock().unwrap_or_else(|e| e.into_inner());
                    if queue.stopped {
                        return;
                    }
                    queue.ready.retain(|old| old.key() != loaded.key());
                    queue.ready.push_back(loaded);
                }
            })?;
        Ok(Self { shared })
    }

    fn submit(&self, job: Job) {
        let mut queue = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.jobs.retain(|old| old.key() != job.key());
        queue.ready.retain(|old| old.key() != job.key());
        queue.jobs.push_back(job);
        self.shared.wake.notify_one();
    }

    fn cancel(&self, key: Key) {
        let mut queue = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.jobs.retain(|job| job.key() != key);
        queue.ready.retain(|loaded| loaded.key() != key);
    }

    fn take_ready(&self) -> VecDeque<Loaded> {
        let mut queue = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        std::mem::take(&mut queue.ready)
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        let mut queue = self.shared.queue.lock().unwrap_or_else(|e| e.into_inner());
        queue.stopped = true;
        queue.jobs.clear();
        queue.ready.clear();
        self.shared.wake.notify_one();
    }
}

pub(crate) struct Background {
    worker: Worker,
    generation: u64,
    sounds: BTreeMap<Slot, Pending>,
    music: Option<u64>,
    results: Vec<(u64, bool)>,
}

impl Background {
    pub(crate) fn new(loader: impl Loader + 'static) -> std::io::Result<Self> {
        Ok(Self {
            worker: Worker::new(loader)?,
            generation: 0,
            sounds: BTreeMap::new(),
            music: None,
            results: Vec::new(),
        })
    }

    fn next_generation(&mut self) -> u64 {
        self.generation = self.generation.wrapping_add(1);
        self.generation
    }

    pub(crate) fn request(
        &mut self,
        mixer: &mut Mixer,
        index: i64,
        action: Action,
        receipt: Option<u64>,
    ) -> PlayResult {
        let Ok(slot) = mixer.bank.prepare(index) else {
            return PlayResult::Ready(false);
        };
        if let Some(sound) = mixer.bank.loaded_mut(index) {
            return PlayResult::Ready(action.apply(sound));
        }
        if self.results.len()
            + self
                .sounds
                .values()
                .map(|pending| pending.requests.len())
                .sum::<usize>()
            >= MAX_PENDING_PLAYS
        {
            tracing::warn!(index, "too many pending sound requests");
            return PlayResult::Ready(false);
        }
        if !self.sounds.contains_key(&slot) {
            let generation = self.next_generation();
            let entry = mixer.bank.list().get(slot).unwrap().clone();
            self.sounds.insert(
                slot,
                Pending {
                    generation,
                    requests: Vec::new(),
                },
            );
            self.worker.submit(Job {
                generation,
                kind: JobKind::Sound(slot, entry),
            });
        }
        let pending = self.sounds.get_mut(&slot).unwrap();
        let playing = pending
            .requests
            .iter()
            .any(|request| !matches!(request.action, Action::Prepare));
        if !action.applies(playing) {
            return PlayResult::Ready(false);
        }
        pending.requests.push(Request { action, receipt });
        PlayResult::Pending
    }

    pub(crate) fn stop(&mut self, index: i64) {
        if let Some(pending) = Slot::new(index).and_then(|slot| self.sounds.get_mut(&slot)) {
            for request in pending.requests.drain(..) {
                if let Some(receipt) = request.receipt {
                    self.results.push((receipt, false));
                }
            }
        }
    }

    pub(crate) fn stop_all(&mut self) {
        for pending in self.sounds.values_mut() {
            for request in pending.requests.drain(..) {
                if let Some(receipt) = request.receipt {
                    self.results.push((receipt, false));
                }
            }
        }
    }

    pub(crate) fn release(&mut self, index: i64) {
        self.stop(index);
        if let Some(slot) = Slot::new(index) {
            self.sounds.remove(&slot);
            self.worker.cancel(Key::Sound(slot));
        }
    }

    pub(crate) fn cancel(&mut self, receipt: u64) {
        for pending in self.sounds.values_mut() {
            pending
                .requests
                .retain(|request| request.receipt != Some(receipt));
        }
        self.results.retain(|(id, _)| *id != receipt);
    }

    pub(crate) fn play_music(&mut self, slot: usize, table: &MusicTable) -> bool {
        if slot >= music::SLOTS {
            return false;
        }
        let generation = self.next_generation();
        self.music = Some(generation);
        self.worker.submit(Job {
            generation,
            kind: JobKind::Music(slot, table.clone()),
        });
        true
    }

    pub(crate) fn stop_music(&mut self) {
        self.music = None;
        self.worker.cancel(Key::Music);
    }

    pub(crate) fn poll(&mut self, mixer: &mut Mixer) -> Vec<(u64, bool)> {
        for loaded in self.worker.take_ready() {
            match loaded.value {
                LoadedValue::Sound(slot, result) => {
                    if !self
                        .sounds
                        .get(&slot)
                        .is_some_and(|pending| pending.generation == loaded.generation)
                    {
                        continue;
                    }
                    let pending = self.sounds.remove(&slot).unwrap();
                    mixer.bank.complete(slot, result);
                    for request in pending.requests {
                        let played = mixer.bank.volume() > crate::volume::MIN
                            && mixer
                                .bank
                                .loaded_mut(slot.get() as i64)
                                .is_some_and(|sound| request.action.apply(sound));
                        if let Some(receipt) = request.receipt {
                            self.results.push((receipt, played));
                        }
                    }
                }
                LoadedValue::Music(slot, result) if self.music == Some(loaded.generation) => {
                    self.music = None;
                    match result {
                        Ok(track) => mixer.play_music(slot, track),
                        Err(error) => tracing::warn!(slot, %error, "music will not play"),
                    }
                }
                LoadedValue::Music(..) => {}
            }
        }
        std::mem::take(&mut self.results)
    }
}

pub(crate) fn open_music(
    slot: usize,
    table: &MusicTable,
    loader: &dyn Loader,
) -> Result<Track, crate::music::Error> {
    if slot >= music::SLOTS {
        return Err(crate::music::Error::Unlisted(slot));
    }
    let (path, bytes) = match table
        .get(slot)
        .and_then(|path| loader.read(path).map(|bytes| (path, bytes)))
    {
        Some(found) => found,
        None => {
            let path = music::BUILTIN[slot];
            let bytes = loader
                .read(path)
                .ok_or_else(|| crate::music::Error::NoFile(path.into()))?;
            (path, bytes)
        }
    };
    Track::open(bytes, path)
}

#[cfg(test)]
mod tests;
