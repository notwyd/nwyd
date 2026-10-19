use crate::bank::{Bank, FRAME, Loader, Refusal, Sound};
use crate::music::Track;
use crate::volume;
#[cfg(not(target_arch = "wasm32"))]
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};

pub const CHANNELS: usize = 2;

#[derive(Debug)]
pub struct Mixer {
    pub bank: Bank,
    device_rate: u32,
    music: Option<Track>,
    music_slot: Option<usize>,
    music_volume: i32,
    active: bool,
    music_prev: [f32; 2],
    music_next: [f32; 2],
    music_frac: u64,
    #[cfg(not(target_arch = "wasm32"))]
    music_generation: Arc<AtomicU64>,
}

#[cfg(not(target_arch = "wasm32"))]
pub(crate) struct MusicBuffer {
    track: Track,
    frames: usize,
    generation: u64,
    current_generation: Arc<AtomicU64>,
}

#[cfg(not(target_arch = "wasm32"))]
impl MusicBuffer {
    pub(crate) fn buffer(&mut self, mut should_stop: impl FnMut() -> bool) -> bool {
        self.track.buffer_frames(self.frames, || {
            should_stop() || self.current_generation.load(Ordering::Acquire) != self.generation
        })
    }
}

impl Mixer {
    pub fn new(bank: Bank, music_volume: i32, device_rate: u32) -> Mixer {
        Mixer {
            bank,
            device_rate: device_rate.max(1),
            music: None,
            music_slot: None,
            music_volume: music_volume.clamp(volume::MIN, volume::MAX),
            active: true,
            music_prev: [0.0; 2],
            music_next: [0.0; 2],
            music_frac: 0,
            #[cfg(not(target_arch = "wasm32"))]
            music_generation: Arc::new(AtomicU64::new(0)),
        }
    }

    pub fn device_rate(&self) -> u32 {
        self.device_rate
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn set_device_rate(&mut self, rate: u32) {
        self.device_rate = rate.max(1);
    }

    pub fn play_music(&mut self, slot: usize, track: Track) {
        #[cfg(not(target_arch = "wasm32"))]
        self.music_generation.fetch_add(1, Ordering::Release);

        self.music = Some(track);

        self.music_slot = Some(slot);

        self.music_prev = [0.0; 2];

        self.music_next = [0.0; 2];

        self.music_frac = 0;
    }

    pub fn stop_music(&mut self) {
        #[cfg(not(target_arch = "wasm32"))]
        self.music_generation.fetch_add(1, Ordering::Release);

        self.music = None;

        self.music_slot = None;
    }

    pub fn music_slot(&self) -> Option<usize> {
        self.music_slot
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn take_music_buffer(&mut self, frames: usize) -> Option<MusicBuffer> {
        if !self.active {
            return None;
        }
        let track = self.music.take()?;
        let step = (track.rate() as u64 * FRAME) / self.device_rate as u64;
        let frames = ((self.music_frac as u128 + frames as u128 * step as u128) / FRAME as u128)
            .try_into()
            .unwrap_or(usize::MAX);
        Some(MusicBuffer {
            track,
            frames,
            generation: self.music_generation.load(Ordering::Acquire),
            current_generation: Arc::clone(&self.music_generation),
        })
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn restore_music_buffer(&mut self, buffered: MusicBuffer) -> Option<MusicBuffer> {
        if self.music_generation.load(Ordering::Acquire) != buffered.generation
            || self.music.is_some()
        {
            return Some(buffered);
        }
        self.music = Some(buffered.track);
        None
    }

    pub fn set_music_volume(&mut self, hundredths_db: i32) {
        self.music_volume = hundredths_db.clamp(volume::MIN, volume::MAX);
    }

    pub fn music_volume(&self) -> i32 {
        self.music_volume
    }

    pub fn set_active(&mut self, active: bool) {
        self.active = active;
    }

    pub fn is_active(&self) -> bool {
        self.active
    }

    pub fn play(&mut self, index: i64, loader: &dyn Loader) -> Result<usize, Refusal> {
        Ok(self.bank.sound(index, loader)?.play(false))
    }

    pub fn play_looping(&mut self, index: i64, loader: &dyn Loader) -> Result<(), Refusal> {
        let sound = self.bank.sound(index, loader)?;

        if !sound.is_playing() {
            sound.play(true);
        }

        Ok(())
    }

    pub fn stop(&mut self, index: i64) {
        if let Some(s) = self.bank.loaded_mut(index) {
            s.stop();
        }
    }

    pub fn is_playing(&self, index: i64) -> bool {
        self.bank.loaded(index).is_some_and(Sound::is_playing)
    }

    pub fn fill(&mut self, out: &mut [f32]) {
        out.fill(0.0);

        if !self.active {
            return;
        }

        let frames = out.len() / CHANNELS;

        self.mix_effects(out);

        self.mix_music(out, frames);

        for s in out.iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }
    }

    fn mix_effects(&mut self, out: &mut [f32]) {
        let gain = volume::amplitude(self.bank.volume()) / i16::MAX as f32;

        if gain == 0.0 {
            return;
        }

        let device_rate = self.device_rate;

        for (_, sound) in self.bank.sounds_mut() {
            if !sound.is_playing() {
                continue;
            }

            let step = (sound.rate() as u64 * FRAME) / device_rate as u64;

            sound.mix(out, step, gain);
        }
    }

    fn mix_music(&mut self, out: &mut [f32], frames: usize) {
        let gain = volume::amplitude(self.music_volume);

        let Some(track) = self.music.as_mut() else {
            return;
        };

        let step = (track.rate() as u64 * FRAME) / self.device_rate as u64;

        for frame in 0..frames {
            let t = self.music_frac as f32 / FRAME as f32;

            let l = self.music_prev[0] + (self.music_next[0] - self.music_prev[0]) * t;

            let r = self.music_prev[1] + (self.music_next[1] - self.music_prev[1]) * t;

            out[frame * CHANNELS] += l * gain;

            out[frame * CHANNELS + 1] += r * gain;

            self.music_frac += step;

            while self.music_frac >= FRAME {
                self.music_frac -= FRAME;

                self.music_prev = self.music_next;

                match track.next_frame() {
                    Some(f) => self.music_next = f,
                    None => {
                        self.music_next = [0.0; 2];

                        self.music = None;

                        self.music_slot = None;

                        return;
                    }
                }
            }
        }
    }
}
