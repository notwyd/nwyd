use crate::error::{Error, Kind, Result};

pub const SLOTS: usize = 16;

pub const LEN: usize = SLOTS * 2;

pub mod slot {

    pub const UNUSED_0: usize = 0;

    pub const RESOLUTION: usize = 1;

    pub const EFFECT_QUALITY: usize = 2;

    pub const SFX_VOLUME: usize = 3;

    pub const MUSIC_VOLUME: usize = 4;

    pub const UNUSED_5: usize = 5;

    pub const BRIGHTNESS: usize = 6;

    pub const CURSOR_MODE: usize = 7;

    pub const DEMO_SCENE: usize = 8;

    pub const WINDOW_MODE: usize = 9;

    pub const DEAD_10: usize = 10;

    pub const INVERT_MOUSE: usize = 11;

    pub const TEXTURE_QUALITY: usize = 12;

    pub const CONTROL_SCHEME: usize = 13;

    pub const UNUSED_14: usize = 14;

    pub const LANGUAGE: usize = 15;
}

pub const DEFAULTS: [i16; SLOTS] = [7000, 7, 2, 100, 100, -1, 57, 2, 1, 1, 1, 0, 0, 0, 1, 0];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Preset {
    pub width: u32,
    pub height: u32,
    pub bpp: u32,
}

pub const PRESETS: [Preset; 10] = [
    Preset {
        width: 800,
        height: 600,
        bpp: 32,
    },
    Preset {
        width: 1024,
        height: 768,
        bpp: 32,
    },
    Preset {
        width: 1280,
        height: 720,
        bpp: 32,
    },
    Preset {
        width: 1280,
        height: 800,
        bpp: 32,
    },
    Preset {
        width: 1280,
        height: 960,
        bpp: 32,
    },
    Preset {
        width: 1280,
        height: 1024,
        bpp: 32,
    },
    Preset {
        width: 1440,
        height: 900,
        bpp: 32,
    },
    Preset {
        width: 1600,
        height: 900,
        bpp: 32,
    },
    Preset {
        width: 1600,
        height: 1024,
        bpp: 32,
    },
    Preset {
        width: 1680,
        height: 1050,
        bpp: 32,
    },
];

pub const FALLBACK: Preset = PRESETS[0];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Language {
    Korean,
    #[default]
    English,
    Portuguese,
    TraditionalChinese,
    SimplifiedChinese,
    Spanish,
    Turkish,
    Polish,
}

impl Language {
    pub fn from_code(code: i16) -> Language {
        match code {
            1 => Language::Korean,
            3 => Language::Portuguese,
            4 => Language::TraditionalChinese,
            5 => Language::SimplifiedChinese,
            6 => Language::Spanish,
            7 => Language::Turkish,
            8 => Language::Polish,
            _ => Language::English,
        }
    }

    pub fn dir(&self) -> &'static str {
        match self {
            Language::Korean => "KR",
            Language::English => "EN",
            Language::Portuguese => "PT",
            Language::TraditionalChinese => "TC",
            Language::SimplifiedChinese => "SC",
            Language::Spanish => "SP",
            Language::Turkish => "TK",
            Language::Polish => "PL",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorMode {
    Engine,
    System,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Config {
    pub raw: [i16; SLOTS],
}

impl Default for Config {
    fn default() -> Self {
        Config { raw: DEFAULTS }
    }
}

pub fn parse(bytes: &[u8]) -> Result<Config> {
    if bytes.len() < LEN {
        return Err(Error::Truncated {
            kind: Kind::Config,
            offset: 0,
            needed: LEN,
            len: bytes.len(),
        });
    }

    let mut raw = [0i16; SLOTS];

    for (i, w) in raw.iter_mut().enumerate() {
        *w = i16::from_le_bytes([bytes[i * 2], bytes[i * 2 + 1]]);
    }

    Ok(Config { raw })
}

impl Config {
    pub fn to_bytes(&self) -> [u8; LEN] {
        let mut out = [0u8; LEN];

        for (i, w) in self.raw.iter().enumerate() {
            out[i * 2..i * 2 + 2].copy_from_slice(&w.to_le_bytes());
        }

        out
    }

    pub fn preset(&self) -> Preset {
        let index = self.raw[slot::RESOLUTION];

        if (1..=PRESETS.len() as i16).contains(&index) {
            PRESETS[index as usize - 1]
        } else {
            FALLBACK
        }
    }

    pub fn fullscreen(&self) -> Option<bool> {
        let v = self.raw[slot::WINDOW_MODE];

        (v >= 0).then_some(v == 0)
    }

    pub fn brightness(&self) -> i16 {
        self.raw[slot::BRIGHTNESS].clamp(0, 100)
    }

    pub fn sfx_volume(&self) -> i16 {
        self.raw[slot::SFX_VOLUME]
    }

    pub fn music_volume(&self) -> i16 {
        self.raw[slot::MUSIC_VOLUME]
    }

    pub fn low_effects(&self) -> bool {
        let v = self.raw[slot::EFFECT_QUALITY];

        if v < 3 { v != 2 } else { false }
    }

    pub fn low_texture(&self) -> bool {
        self.raw[slot::TEXTURE_QUALITY] <= 0
    }

    pub fn invert_mouse(&self) -> bool {
        self.raw[slot::INVERT_MOUSE] > 0
    }

    pub fn control_scheme(&self) -> i16 {
        self.raw[slot::CONTROL_SCHEME]
    }

    pub fn play_demo_scene(&self) -> bool {
        self.raw[slot::DEMO_SCENE] == 1
    }

    pub fn cursor_mode(&self) -> CursorMode {
        if self.raw[slot::CURSOR_MODE] == 1 {
            CursorMode::Engine
        } else {
            CursorMode::System
        }
    }

    pub fn language(&self) -> Language {
        Language::from_code(self.raw[slot::LANGUAGE])
    }

    pub fn font_size(&self) -> u32 {
        match self.preset().width {
            640 => 10,
            800 => 12,
            1024 => 14,
            1280 => 20,
            _ => 24,
        }
    }

    pub fn ui_scale(&self) -> (f32, f32) {
        let p = self.preset();

        (p.width as f32 / 800.0, p.height as f32 / 600.0)
    }
}
