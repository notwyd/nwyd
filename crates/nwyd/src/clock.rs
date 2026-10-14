#[cfg(not(windows))]
use web_time::Instant;

#[cfg(windows)]
#[link(name = "winmm")]
unsafe extern "system" {

    fn timeGetTime() -> u32;

}

mod time;

pub use time::{ClockSample, GameTime, RawTime, WireTime};

#[derive(Debug)]
enum TimeBase {
    #[cfg(windows)]
    Native,
    #[cfg(not(windows))]
    Real(Instant),
    Frozen(u32),
}

#[derive(Debug)]
pub struct Clock {
    epoch: TimeBase,
    server_ms: u32,
    local_ms_at_sync: u32,
    offset_ms: i32,
}

impl Default for Clock {
    fn default() -> Clock {
        Clock::new()
    }
}

impl Clock {
    pub fn sample(&self) -> ClockSample {
        let raw = self.tick_ms();

        let wire = game_time(self.server_ms, self.local_ms_at_sync, raw, 0);

        ClockSample {
            raw: RawTime::from_millis(raw),
            wire: WireTime::from_millis(wire),
            game: GameTime::from_millis(wire.wrapping_add(self.offset_ms as u32)),
        }
    }

    pub fn raw_now(&self) -> RawTime {
        RawTime::from_millis(self.tick_ms())
    }

    pub fn game_now(&self) -> GameTime {
        self.sample().game
    }

    pub fn wire_now(&self) -> WireTime {
        self.sample().wire
    }

    pub fn synchronize(&mut self, timestamp: WireTime) {
        self.set_time(timestamp.as_millis());
    }

    pub fn is_frozen(&self) -> bool {
        matches!(self.epoch, TimeBase::Frozen(_))
    }

    pub fn new() -> Clock {
        #[cfg(windows)]
        let epoch = TimeBase::Native;

        #[cfg(not(windows))]
        let epoch = TimeBase::Real(Instant::now());

        let mut clock = Clock {
            epoch,
            server_ms: 0,
            local_ms_at_sync: 0,
            offset_ms: 0,
        };

        clock.set_time(0);

        clock
    }

    pub fn frozen(tick_ms: u32) -> Clock {
        Clock {
            epoch: TimeBase::Frozen(tick_ms),
            server_ms: 0,
            local_ms_at_sync: 0,
            offset_ms: 0,
        }
    }

    pub fn tick_ms(&self) -> u32 {
        match self.epoch {
            #[cfg(windows)]
            TimeBase::Native => unsafe { timeGetTime() },
            #[cfg(not(windows))]
            TimeBase::Real(epoch) => epoch.elapsed().as_millis() as u32,
            TimeBase::Frozen(tick) => tick,
        }
    }

    pub fn set_time(&mut self, server_ms: u32) {
        self.server_ms = server_ms;

        self.local_ms_at_sync = self.tick_ms();
    }

    pub fn game_time(&self) -> u32 {
        self.game_now().as_millis()
    }

    pub fn wire_tick(&self) -> u32 {
        self.wire_now().as_millis()
    }

    pub fn offset_ms(&self) -> i32 {
        self.offset_ms
    }

    pub fn set_offset_ms(&mut self, offset_ms: i32) {
        self.offset_ms = offset_ms;
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TimeSync {
    last_compare_ms: GameTime,
    desync: Option<Desync>,
    armed_at_ms: Option<GameTime>,
    armed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Desync {
    Behind = 2,
    Ahead = 3,
}

impl TimeSync {
    pub const INTERVAL_MS: u32 = 10_000;

    pub const TOLERANCE_MS: u32 = 500;

    pub const STEP_MS: u32 = 0x55;

    pub const ARM_MS: u32 = 3_000;

    pub fn loaded_zone(&mut self, now_ms: GameTime) {
        self.armed_at_ms = Some(now_ms);

        self.armed = false;
    }

    pub fn left_world(&mut self) {
        self.armed_at_ms = None;

        self.armed = false;
    }

    pub fn advance_field(&mut self, now: GameTime) {
        self.armed |= self
            .armed_at_ms
            .is_some_and(|at| at.as_millis().wrapping_add(Self::ARM_MS) < now.as_millis());
    }

    pub fn compare(&mut self, clock: &mut Clock, server_ms: WireTime) -> Option<Desync> {
        let now = clock.game_now();

        if self
            .last_compare_ms
            .as_millis()
            .wrapping_add(Self::INTERVAL_MS)
            >= now.as_millis()
        {
            return None;
        }

        self.last_compare_ms = now;

        if !self.armed {
            return None;
        }

        let desync = if now.as_millis().wrapping_add(Self::TOLERANCE_MS) < server_ms.as_millis() {
            clock.set_time(now.as_millis().wrapping_add(Self::STEP_MS));

            Desync::Behind
        } else if server_ms.as_millis() < now.as_millis().wrapping_sub(Self::TOLERANCE_MS) {
            clock.set_time(now.as_millis().wrapping_sub(Self::STEP_MS));

            Desync::Ahead
        } else {
            return None;
        };

        self.desync = Some(desync);

        Some(desync)
    }

    pub fn take_report(&mut self) -> Option<Desync> {
        self.desync.take()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Calendar {
    pub day_of_month: u32,
    pub server_day_of_month: u32,
    pub hour: u32,
    pub minute: u32,
    pub second: u32,
    pub epoch_day: i64,
    pub unix_seconds: i64,
}

impl Calendar {
    pub const PINNED_EPOCH_DAY: i64 = 20_000;

    pub const GOLDEN: Calendar = Calendar::pinned(2, 0);

    pub const fn pinned(day_of_month: u32, second: u32) -> Calendar {
        Calendar::pinned_time(day_of_month, 0, 0, second)
    }

    pub const fn pinned_time(day_of_month: u32, hour: u32, minute: u32, second: u32) -> Calendar {
        Calendar {
            day_of_month,
            server_day_of_month: day_of_month,
            hour,
            minute,
            second,
            epoch_day: Calendar::PINNED_EPOCH_DAY,
            unix_seconds: (Calendar::PINNED_EPOCH_DAY + 3) * 86_400
                + hour as i64 * 3_600
                + minute as i64 * 60
                + second as i64,
        }
    }

    pub fn local() -> Calendar {
        let secs = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Self::from_unix_seconds(secs as i64)
    }

    fn from_unix_seconds(secs: i64) -> Calendar {
        let (day_of_month, hour, minute, second) = civil_local(secs);

        Calendar {
            day_of_month,
            server_day_of_month: civil_utc(secs - 3 * 3_600).0,
            hour,
            minute,
            second,
            epoch_day: secs.div_euclid(86_400) - 3,
            unix_seconds: secs,
        }
    }

    pub fn bg_variant(&self) -> u32 {
        self.second % 3
    }

    pub fn day_quarter(&self) -> u32 {
        self.day_of_month & 3
    }
}

#[cfg(unix)]
fn civil_local(secs: i64) -> (u32, u32, u32, u32) {
    let t = secs as libc::time_t;

    let mut tm: libc::tm = unsafe { std::mem::zeroed() };

    let converted = unsafe { !libc::localtime_r(&t, &mut tm).is_null() };

    if converted {
        (
            tm.tm_mday as u32,
            tm.tm_hour as u32,
            tm.tm_min as u32,
            tm.tm_sec as u32,
        )
    } else {
        civil_utc(secs)
    }
}

#[cfg(all(not(unix), not(target_arch = "wasm32")))]
fn civil_local(secs: i64) -> (u32, u32, u32, u32) {
    civil_utc(secs)
}

fn civil_utc(secs: i64) -> (u32, u32, u32, u32) {
    let days = secs.div_euclid(86_400);

    let day_secs = secs.rem_euclid(86_400);

    let z = days + 719_468;

    let era = z.div_euclid(146_097);

    let doe = z.rem_euclid(146_097);

    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;

    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);

    let mp = (5 * doy + 2) / 153;

    let day = doy - (153 * mp + 2) / 5 + 1;

    let _ = era;

    (
        day as u32,
        (day_secs / 3_600) as u32,
        ((day_secs % 3_600) / 60) as u32,
        (day_secs % 60) as u32,
    )
}

pub fn game_time(server_ms: u32, local_ms_at_sync: u32, now_ms: u32, offset_ms: i32) -> u32 {
    let delta = now_ms.wrapping_sub(local_ms_at_sync);

    server_ms.wrapping_add(delta).wrapping_add(offset_ms as u32)
}

#[cfg(test)]
mod tests {

    use super::*;

    fn armed_sync() -> TimeSync {
        let mut sync = TimeSync::default();

        sync.loaded_zone(GameTime::from_millis(0));

        sync.advance_field(GameTime::from_millis(TimeSync::ARM_MS + 1));

        sync
    }

    #[test]
    fn synchronized_samples_keep_raw_and_wire_time_separate_from_game_offset() {
        let mut clock = Clock::frozen(1_234);

        clock.synchronize(WireTime::from_millis(9_000));

        clock.set_offset_ms(-250);

        assert_eq!(
            clock.sample(),
            ClockSample {
                raw: RawTime::from_millis(1_234),
                wire: WireTime::from_millis(9_000),
                game: GameTime::from_millis(8_750),
            }
        );

        clock.epoch = TimeBase::Frozen(2_234);

        assert_eq!(clock.wire_tick(), 10_000);

        assert_eq!(clock.game_time(), 9_750);

        clock.set_offset_ms(500);

        assert_eq!(clock.wire_tick(), 10_000);

        assert_eq!(clock.game_time(), 10_500);
    }

    #[test]
    fn clock_arithmetic_wraps_at_tick_and_server_boundaries() {
        assert_eq!(game_time(100, u32::MAX - 9, 10, -150), u32::MAX - 29);

        assert_eq!(game_time(u32::MAX - 4, 20, 30, 0), 5);

        assert_eq!(
            RawTime::from_millis(5).elapsed_since(RawTime::from_millis(u32::MAX - 4)),
            10
        );

        assert_eq!(
            GameTime::from_millis(5).elapsed_since(GameTime::from_millis(u32::MAX - 4)),
            10
        );

        assert_eq!(
            WireTime::from_millis(5).elapsed_since(WireTime::from_millis(u32::MAX - 4)),
            10
        );
    }

    #[test]
    fn time_sync_waits_for_zone_delay_and_comparison_interval() {
        let mut sync = TimeSync::default();

        sync.loaded_zone(GameTime::from_millis(0));

        sync.advance_field(GameTime::from_millis(TimeSync::ARM_MS));

        let mut clock = Clock::frozen(10_001);

        assert_eq!(
            sync.compare(&mut clock, WireTime::from_millis(11_000)),
            None
        );

        assert_eq!(clock.wire_tick(), 10_001);

        sync.advance_field(GameTime::from_millis(TimeSync::ARM_MS + 1));

        clock.set_time(20_001);

        assert_eq!(
            sync.compare(&mut clock, WireTime::from_millis(21_000)),
            None
        );

        assert_eq!(clock.wire_tick(), 20_001);

        clock.set_time(20_002);

        assert_eq!(
            sync.compare(&mut clock, WireTime::from_millis(21_000)),
            Some(Desync::Behind)
        );
    }

    #[test]
    fn time_sync_accepts_exact_tolerance_in_both_directions() {
        for server_ms in [9_501, 10_501] {
            let mut sync = armed_sync();

            let mut clock = Clock::frozen(10_001);

            assert_eq!(
                sync.compare(&mut clock, WireTime::from_millis(server_ms)),
                None
            );

            assert_eq!(clock.wire_tick(), 10_001);

            assert_eq!(sync.take_report(), None);
        }
    }

    #[test]
    fn time_sync_corrects_gradually_and_reports_once() {
        for (server_ms, direction, corrected_ms) in [
            (10_502, Desync::Behind, 10_086),
            (9_500, Desync::Ahead, 9_916),
        ] {
            let mut sync = armed_sync();

            let mut clock = Clock::frozen(10_001);

            assert_eq!(
                sync.compare(&mut clock, WireTime::from_millis(server_ms)),
                Some(direction)
            );

            assert_eq!(clock.wire_tick(), corrected_ms);

            assert_eq!(
                sync.compare(&mut clock, WireTime::from_millis(server_ms)),
                None
            );

            assert_eq!(clock.wire_tick(), corrected_ms);

            assert_eq!(sync.take_report(), Some(direction));

            assert_eq!(sync.take_report(), None);
        }
    }

    #[test]
    fn leaving_world_disarms_time_correction() {
        let mut sync = armed_sync();

        sync.left_world();

        sync.advance_field(GameTime::from_millis(10_001));

        let mut clock = Clock::frozen(10_001);

        assert_eq!(
            sync.compare(&mut clock, WireTime::from_millis(20_000)),
            None
        );

        assert_eq!(clock.wire_tick(), 10_001);

        assert_eq!(sync.take_report(), None);
    }
}

#[cfg(target_arch = "wasm32")]
fn civil_local(secs: i64) -> (u32, u32, u32, u32) {
    let date = js_sys::Date::new(&wasm_bindgen::JsValue::from_f64(secs as f64 * 1000.0));

    (
        date.get_date(),
        date.get_hours(),
        date.get_minutes(),
        date.get_seconds(),
    )
}
