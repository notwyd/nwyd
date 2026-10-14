use crate::bank::{Bank, FRAME, Loader, Refusal, Sound};
use crate::music::Track;
use crate::volume;

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
        }
    }

    pub fn device_rate(&self) -> u32 {
        self.device_rate
    }

    pub fn play_music(&mut self, slot: usize, track: Track) {
        self.music = Some(track);

        self.music_slot = Some(slot);

        self.music_prev = [0.0; 2];

        self.music_next = [0.0; 2];

        self.music_frac = 0;
    }

    pub fn stop_music(&mut self) {
        self.music = None;

        self.music_slot = None;
    }

    pub fn music_slot(&self) -> Option<usize> {
        self.music_slot
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

        self.mix_effects(out, frames);

        self.mix_music(out, frames);

        for s in out.iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }
    }

    fn mix_effects(&mut self, out: &mut [f32], frames: usize) {
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

            let voices: Vec<(usize, u64)> =
                sound.active_fixed().map(|(i, pos, _)| (i, pos)).collect();

            for (voice, start) in voices {
                let mut pos = start;

                for frame in 0..frames {
                    let [l, r] = sound.sample(pos);

                    out[frame * CHANNELS] += l * gain;

                    out[frame * CHANNELS + 1] += r * gain;

                    sound.advance_fixed(voice, step);

                    match sound.active_fixed().find(|&(i, _, _)| i == voice) {
                        Some((_, next, _)) => pos = next,
                        None => break,
                    }
                }
            }
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
