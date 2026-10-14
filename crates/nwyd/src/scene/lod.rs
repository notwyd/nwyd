#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameRate {
    rate: f32,
    frames: u32,
    epoch_s: f32,
    sampling: bool,
}

pub const RESAMPLE_AFTER_S: f32 = 2.0;

pub const CAPTURE_RATE: f32 = 0.0;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Inputs {
    pub rate: f32,
    pub smoothing: formats::mesh::pose::Smoothing,
}

impl Default for Inputs {
    fn default() -> Self {
        Self {
            rate: 0.0,
            smoothing: formats::mesh::pose::Smoothing::On,
        }
    }
}

impl Inputs {
    pub fn live(rate: f32, quality: i16) -> Self {
        Self {
            rate: if rate.is_finite() && rate >= 0.0 {
                rate
            } else {
                0.0
            },
            smoothing: if quality != 0 && rate >= 10.0 && rate.is_finite() {
                formats::mesh::pose::Smoothing::On
            } else {
                formats::mesh::pose::Smoothing::Off
            },
        }
    }
}

pub fn effects_enabled(quality: i16) -> bool {
    quality.min(2) == 2
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EffectOptions {
    pub detail: super::aura::Detail,
    pub secondary: bool,
    pub grass: bool,
}

impl Default for EffectOptions {
    fn default() -> Self {
        Self {
            detail: Default::default(),
            secondary: true,
            grass: true,
        }
    }
}

impl EffectOptions {
    pub fn toggle(&mut self, toggle_grass: bool) {
        self.detail.effects = !self.detail.effects;

        self.secondary = !self.detail.effects;

        self.detail.glow = self.detail.effects;

        if toggle_grass {
            self.grass = self.detail.effects;
        }
    }
}

impl Default for FrameRate {
    fn default() -> FrameRate {
        FrameRate {
            rate: 0.0,
            frames: 0,
            epoch_s: 0.0,
            sampling: true,
        }
    }
}

impl FrameRate {
    pub fn starting_at(now_ms: u32) -> Self {
        Self {
            epoch_s: now_ms as f32 * 0.001,
            ..Self::new()
        }
    }

    pub fn new() -> FrameRate {
        FrameRate::default()
    }

    pub fn frozen() -> FrameRate {
        FrameRate {
            rate: CAPTURE_RATE,
            frames: 0,
            epoch_s: 0.0,
            sampling: false,
        }
    }

    pub fn rate(&self) -> f32 {
        self.rate
    }

    pub fn frames_since_sample(&self) -> u32 {
        self.frames
    }

    pub fn is_sampling(&self) -> bool {
        self.sampling
    }

    pub fn sample(&mut self, now_ms: u32) -> bool {
        if !self.sampling {
            return false;
        }

        let now_s = now_ms as f32 * 0.001;

        self.frames = self.frames.wrapping_add(1);

        let elapsed = now_s - self.epoch_s;

        if RESAMPLE_AFTER_S < elapsed {
            self.rate = self.frames as f32 / elapsed;

            self.frames = 0;

            self.epoch_s = now_s;

            return true;
        }

        false
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ladder {
    pub rungs: &'static [(f32, i32)],
    pub default: i32,
}

impl Ladder {
    pub fn at(&self, rate: f32) -> i32 {
        let Some((last, rest)) = self.rungs.split_last() else {
            return self.default;
        };

        for &(threshold, payload) in rest {
            if threshold <= rate {
                continue;
            }

            return payload;
        }

        match rate < last.0 {
            true => last.1,
            false => self.default,
        }
    }

    pub fn minimum(&self) -> i32 {
        match self.rungs.first() {
            Some(&(_, payload)) => payload,
            None => self.default,
        }
    }
}

pub const DETAIL: i32 = 0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DetailGate {
    Always,
    Full,
    NotReduced,
}

impl DetailGate {
    pub fn admits(self, detail: i32) -> bool {
        match self {
            DetailGate::Always => true,
            DetailGate::Full => detail == 0,
            DetailGate::NotReduced => detail != 1,
        }
    }
}
