pub const MAX: i32 = 0;

pub const MIN: i32 = -10000;

pub const SFX_STEP: i32 = 25;

pub const SFX_FLOOR: i32 = -2500;

pub const MUSIC_STEP: i32 = 30;

pub fn sfx(percent: i32) -> i32 {
    ((percent - 100) * SFX_STEP).clamp(SFX_FLOOR, MAX)
}

pub fn music(percent: i32) -> i32 {
    ((percent - 100) * MUSIC_STEP).clamp(MIN, MAX)
}

pub fn amplitude(hundredths_db: i32) -> f32 {
    if hundredths_db <= MIN {
        return 0.0;
    }

    10f32.powf(hundredths_db as f32 / 2000.0)
}
