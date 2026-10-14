#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    pub raw_ms: u32,
    pub wall_secs: u32,
}

impl Sample {
    pub fn now(raw_ms: u32) -> Option<Self> {
        let wall = web_time::SystemTime::now()
            .duration_since(web_time::UNIX_EPOCH)
            .ok()?;

        Some(Self {
            raw_ms,
            wall_secs: wall.as_secs() as u32,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Event {
    Strike { seconds: u32 },
    Report { seconds: u32 },
}

#[derive(Debug, Default)]
pub struct Detector {
    baseline: Option<Sample>,
    strikes: u8,
}

impl Detector {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn observe(&mut self, active: bool, sample: Sample) -> Option<Event> {
        if !active {
            self.reset();

            return None;
        }

        let Some(base) = self.baseline.filter(|base| base.raw_ms != 0) else {
            self.baseline = Some(sample);

            self.strikes = 0;

            return None;
        };

        if self.strikes == 2 {
            if sample.raw_ms.wrapping_sub(base.raw_ms) > 300_000 {
                self.reset();
            }

            return None;
        }

        let before = base.wall_secs.wrapping_sub(base.raw_ms / 1000);

        let after = sample.wall_secs.wrapping_sub(sample.raw_ms / 1000);

        let seconds = before.abs_diff(after);

        if seconds <= 15 {
            return None;
        }

        self.baseline = Some(sample);

        self.strikes += 1;

        Some(if self.strikes == 2 {
            Event::Report { seconds }
        } else {
            Event::Strike { seconds }
        })
    }
}

#[derive(Debug, Default)]
pub struct Monitor {
    detector: Detector,
}

impl Monitor {
    pub fn reset(&mut self) {
        *self = Self::default();
    }

    pub fn observe(&mut self, active: bool, sample: Option<Sample>) -> Option<Event> {
        let Some(sample) = sample.filter(|_| active) else {
            self.reset();

            return None;
        };

        self.detector.observe(true, sample)
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    fn sample(raw_ms: u32, wall_secs: u32) -> Sample {
        Sample { raw_ms, wall_secs }
    }

    #[test]
    fn ordinary_progress_and_tolerated_drift_do_not_accumulate_strikes() {
        let mut detector = Detector::default();

        for current in [
            sample(1_000, 100),
            sample(2_000, 101),
            sample(3_000, 117),
            sample(4_000, 88),
            sample(5_000, 104),
        ] {
            assert_eq!(detector.observe(true, current), None);
        }

        assert_eq!(
            detector.observe(true, sample(6_000, 121)),
            Some(Event::Strike { seconds: 16 })
        );
    }

    #[test]
    fn independent_drift_events_are_rebased_and_reported_on_second_strike() {
        let mut detector = Detector::default();

        assert_eq!(detector.observe(true, sample(1_000, 100)), None);

        assert_eq!(
            detector.observe(true, sample(2_000, 117)),
            Some(Event::Strike { seconds: 16 })
        );

        assert_eq!(detector.observe(true, sample(3_000, 118)), None);

        assert_eq!(
            detector.observe(true, sample(4_000, 103)),
            Some(Event::Report { seconds: 16 })
        );
    }

    #[test]
    fn inactive_observation_resets_the_strike_count() {
        let mut detector = Detector::default();

        detector.observe(true, sample(1_000, 100));

        assert_eq!(
            detector.observe(true, sample(2_000, 117)),
            Some(Event::Strike { seconds: 16 })
        );

        assert_eq!(detector.observe(false, sample(3_000, 200)), None);

        assert_eq!(detector.observe(true, sample(4_000, 300)), None);

        assert_eq!(
            detector.observe(true, sample(5_000, 317)),
            Some(Event::Strike { seconds: 16 })
        );
    }

    #[test]
    fn reported_drift_is_suppressed_until_cooldown_expires_even_across_tick_wrap() {
        for start in [1_000, u32::MAX - 10_000] {
            let first = sample(start, start / 1_000 + 100);

            let second = sample(start + 1_000, (start + 1_000) / 1_000 + 116);

            let third = sample(start + 2_000, (start + 2_000) / 1_000 + 132);

            let mut detector = Detector::default();

            assert_eq!(detector.observe(true, first), None);

            assert_eq!(
                detector.observe(true, second),
                Some(Event::Strike { seconds: 16 })
            );

            assert_eq!(
                detector.observe(true, third),
                Some(Event::Report { seconds: 16 })
            );

            let boundary = third.raw_ms.wrapping_add(300_000);

            assert_eq!(detector.observe(true, sample(boundary, 500)), None);

            assert_eq!(
                detector.observe(true, sample(boundary.wrapping_add(1), 500)),
                None
            );

            let rebase = boundary.wrapping_add(1_000);

            assert_eq!(detector.observe(true, sample(rebase, 500)), None);

            assert_eq!(
                detector.observe(true, sample(rebase.wrapping_add(1_000), 517)),
                Some(Event::Strike { seconds: 16 })
            );
        }
    }

    #[test]
    fn monitor_missing_sample_resets_detection() {
        let mut monitor = Monitor::default();

        assert_eq!(monitor.observe(true, Some(sample(1_000, 100))), None);

        assert_eq!(
            monitor.observe(true, Some(sample(2_000, 117))),
            Some(Event::Strike { seconds: 16 })
        );

        assert_eq!(monitor.observe(true, None), None);

        assert_eq!(monitor.observe(true, Some(sample(3_000, 200))), None);

        assert_eq!(
            monitor.observe(true, Some(sample(4_000, 217))),
            Some(Event::Strike { seconds: 16 })
        );
    }
}
