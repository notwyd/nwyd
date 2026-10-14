use crate::volume;
use crate::wave::{self, Wave};
use formats::audio::sound_list::{Slot, SoundList};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Lcg(u32);

impl Lcg {
    fn next_mod(&mut self, n: u32) -> u32 {
        self.0 = self.0.wrapping_mul(0x343FD).wrapping_add(0x269EC3);

        ((self.0 >> 16) & 0x7FFF) % n.max(1)
    }
}

pub trait Loader: Send {
    fn read(&self, path: &str) -> Option<Vec<u8>>;
}

pub(crate) const FRAME: u64 = 1 << 32;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
struct Voice {
    position: Option<u64>,
    looping: bool,
}

#[derive(Debug, Clone)]
pub struct Sound {
    wave: Wave,
    voices: Vec<Voice>,
    rng: Lcg,
}

impl Sound {
    pub fn play(&mut self, looping: bool) -> usize {
        let free = self.voices.iter().position(|v| v.position.is_none());

        let index = match free {
            Some(i) => i,
            None => self.rng.next_mod(self.voices.len() as u32) as usize,
        };

        self.voices[index] = Voice {
            position: Some(0),
            looping,
        };

        index
    }

    pub(crate) fn sample(&self, position: u64) -> [f32; 2] {
        let i = (position / FRAME) as usize;

        let t = (position % FRAME) as f32 / FRAME as f32;

        let a = self.wave.frame(i);

        let b = self.wave.frame(i + 1);

        [
            a[0] as f32 + (b[0] as f32 - a[0] as f32) * t,
            a[1] as f32 + (b[1] as f32 - a[1] as f32) * t,
        ]
    }

    pub(crate) fn rate(&self) -> u32 {
        self.wave.rate
    }

    pub fn stop(&mut self) {
        for v in &mut self.voices {
            *v = Voice::default();
        }
    }

    pub fn is_playing(&self) -> bool {
        self.voices.iter().any(|v| v.position.is_some())
    }

    pub fn wave(&self) -> &Wave {
        &self.wave
    }

    pub fn active(&self) -> impl Iterator<Item = (usize, bool)> + '_ {
        self.voices
            .iter()
            .filter_map(|v| v.position.map(|p| ((p / FRAME) as usize, v.looping)))
    }

    pub(crate) fn active_fixed(&self) -> impl Iterator<Item = (usize, u64, bool)> + '_ {
        self.voices
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.position.map(|p| (i, p, v.looping)))
    }

    pub(crate) fn advance_fixed(&mut self, voice: usize, delta: u64) {
        let total = self.wave.frames() as u64 * FRAME;

        let Some(v) = self.voices.get_mut(voice) else {
            return;
        };

        let Some(pos) = v.position else {
            return;
        };

        let next = pos.saturating_add(delta);

        v.position = if total == 0 {
            None
        } else if next < total {
            Some(next)
        } else if v.looping {
            Some(next % total)
        } else {
            None
        };
    }

    pub fn voices(&self) -> usize {
        self.voices.len()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    OutOfRange,
    Muted,
    Unlisted,
    NoFile(String),
    Undecodable(String),
}

#[derive(Debug)]
pub struct Bank {
    list: SoundList,
    volume: i32,
    sounds: BTreeMap<u16, Sound>,
    refused: BTreeMap<u16, Refusal>,
}

impl Bank {
    pub fn new(list: SoundList, volume: i32) -> Bank {
        Bank {
            list,
            volume: volume.clamp(volume::MIN, volume::MAX),
            sounds: BTreeMap::new(),
            refused: BTreeMap::new(),
        }
    }

    pub fn set_volume(&mut self, hundredths_db: i32) {
        self.volume = hundredths_db.clamp(volume::MIN, volume::MAX);
    }

    pub fn volume(&self) -> i32 {
        self.volume
    }

    pub fn list(&self) -> &SoundList {
        &self.list
    }

    pub fn sound(&mut self, index: i64, loader: &dyn Loader) -> Result<&mut Sound, Refusal> {
        if self.volume <= volume::MIN {
            return Err(Refusal::Muted);
        }

        let slot = Slot::new(index).ok_or(Refusal::OutOfRange)?;

        let key = slot.get() as u16;

        if let Some(r) = self.refused.get(&key) {
            return Err(r.clone());
        }

        if !self.sounds.contains_key(&key) {
            let sound = self.load(slot, loader).inspect_err(|r| {
                tracing::debug!(slot = key, refusal = ? r, "Load Sound Error");

                self.refused.insert(key, r.clone());
            })?;

            self.sounds.insert(key, sound);
        }

        Ok(self.sounds.get_mut(&key).expect("just inserted"))
    }

    fn load(&self, slot: Slot, loader: &dyn Loader) -> Result<Sound, Refusal> {
        let entry = self.list.get(slot).ok_or(Refusal::Unlisted)?;

        let path = entry.path();

        let bytes = loader
            .read(&path)
            .ok_or_else(|| Refusal::NoFile(path.clone()))?;

        let wave = wave::parse(&bytes).map_err(|e| {
            tracing::warn!(% path, error = % e, "sound will not decode");

            Refusal::Undecodable(path.clone())
        })?;

        let voices = entry.voices.max(1) as usize;

        Ok(Sound {
            wave,
            voices: vec![Voice::default(); voices],
            rng: Lcg(slot.get() as u32),
        })
    }

    pub fn loaded(&self, index: i64) -> Option<&Sound> {
        Slot::new(index).and_then(|s| self.sounds.get(&(s.get() as u16)))
    }

    pub fn loaded_mut(&mut self, index: i64) -> Option<&mut Sound> {
        Slot::new(index).and_then(|s| self.sounds.get_mut(&(s.get() as u16)))
    }

    pub fn resident(&self) -> impl Iterator<Item = i64> + '_ {
        self.sounds.keys().map(|&k| k as i64)
    }

    pub(crate) fn sounds_mut(&mut self) -> impl Iterator<Item = (&u16, &mut Sound)> {
        self.sounds.iter_mut()
    }

    pub fn release(&mut self, index: i64) {
        if let Some(slot) = Slot::new(index) {
            self.sounds.remove(&(slot.get() as u16));

            self.refused.remove(&(slot.get() as u16));
        }
    }
}
