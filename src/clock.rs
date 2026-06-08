//! Internal clock with drift correction.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Clock drift statistics.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct DriftStats {
    /// Total drift in microseconds (positive = clock ran fast).
    pub total_drift_us: i64,
    /// Number of corrections applied.
    pub corrections: u64,
    /// Average correction in microseconds.
    pub avg_correction_us: i64,
}

/// Internal clock with drift correction.
#[derive(Debug)]
pub struct Clock {
    /// The expected tick interval.
    interval: Duration,
    /// Last tick time.
    last_tick: Option<Instant>,
    /// Accumulated error in microseconds.
    drift_us: i64,
    /// Total corrections applied.
    corrections: u64,
    /// Sum of all correction magnitudes for averaging.
    correction_sum_us: u64,
    /// Maximum allowed drift before forced correction (microseconds).
    max_drift_us: u64,
}

impl Clock {
    /// Create a new clock with the given tick interval.
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            last_tick: None,
            drift_us: 0,
            corrections: 0,
            correction_sum_us: 0,
            max_drift_us: 10_000, // 10ms max drift before forced correction
        }
    }

    /// Start the clock (record the reference time).
    pub fn start(&mut self) {
        self.last_tick = Some(Instant::now());
    }

    /// Set the tick interval.
    pub fn set_interval(&mut self, interval: Duration) {
        self.interval = interval;
    }

    /// Get the tick interval.
    pub fn interval(&self) -> Duration {
        self.interval
    }

    /// Compute the corrected sleep duration for the next tick.
    ///
    /// Returns the duration to sleep, accounting for accumulated drift.
    /// A negative drift (clock ran slow) shortens the sleep;
    /// a positive drift (clock ran fast) lengthens it.
    pub fn next_tick_duration(&mut self) -> Duration {
        let mut target = self.interval;

        // Apply drift correction
        if self.drift_us != 0 {
            let correction = -self.drift_us; // opposite sign to correct
            let correction_dur = Duration::from_micros(correction.unsigned_abs());
            if correction > 0 {
                target = target.saturating_sub(correction_dur);
            } else {
                target = target.saturating_add(correction_dur);
            }
            self.corrections += 1;
            self.correction_sum_us += correction.unsigned_abs();
            self.drift_us = 0; // reset after correction
        }

        target
    }

    /// Mark that a tick has occurred. Updates drift tracking.
    pub fn tick(&mut self) {
        let now = Instant::now();
        if let Some(last) = self.last_tick {
            let elapsed = now.duration_since(last);
            let expected = self.interval;
            let diff = elapsed.as_micros() as i64 - expected.as_micros() as i64;
            self.drift_us += diff;

            // Force correction if drift exceeds max
            if self.drift_us.unsigned_abs() > self.max_drift_us {
                self.corrections += 1;
                self.correction_sum_us += self.drift_us.unsigned_abs();
                self.drift_us = 0;
            }
        }
        self.last_tick = Some(now);
    }

    /// Reset the clock state.
    pub fn reset(&mut self) {
        self.last_tick = None;
        self.drift_us = 0;
        self.corrections = 0;
        self.correction_sum_us = 0;
    }

    /// Get current drift in microseconds.
    pub fn current_drift_us(&self) -> i64 {
        self.drift_us
    }

    /// Get drift statistics.
    pub fn stats(&self) -> DriftStats {
        DriftStats {
            total_drift_us: self.drift_us,
            corrections: self.corrections,
            avg_correction_us: if self.corrections > 0 {
                self.correction_sum_us.checked_div(self.corrections).unwrap_or(0) as i64
            } else {
                0
            },
        }
    }

    /// Elapsed time since last tick (or start).
    pub fn elapsed(&self) -> Option<Duration> {
        self.last_tick.map(|t| t.elapsed())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_clock_no_drift() {
        let clock = Clock::new(Duration::from_millis(10));
        assert_eq!(clock.current_drift_us(), 0);
    }

    #[test]
    fn start_sets_last_tick() {
        let mut clock = Clock::new(Duration::from_millis(10));
        clock.start();
        assert!(clock.elapsed().is_some());
    }

    #[test]
    fn tick_updates_drift() {
        let mut clock = Clock::new(Duration::from_millis(10));
        clock.start();
        std::thread::sleep(Duration::from_millis(15));
        clock.tick();
        // Should have ~5ms drift
        assert!(clock.current_drift_us().unsigned_abs() > 3000);
    }

    #[test]
    fn next_tick_corrects_drift() {
        let mut clock = Clock::new(Duration::from_millis(10));
        clock.drift_us = 1000; // 1ms fast
        let _dur = clock.next_tick_duration();
        // Drift was positive, correction should have been applied
        // After correction, drift reset to 0
        assert_eq!(clock.current_drift_us(), 0);
    }

    #[test]
    fn drift_stats_initially_zero() {
        let clock = Clock::new(Duration::from_millis(10));
        let stats = clock.stats();
        assert_eq!(stats.corrections, 0);
        assert_eq!(stats.avg_correction_us, 0);
    }

    #[test]
    fn stats_after_corrections() {
        let mut clock = Clock::new(Duration::from_millis(10));
        clock.corrections = 5;
        clock.correction_sum_us = 1000;
        let stats = clock.stats();
        assert_eq!(stats.corrections, 5);
        assert_eq!(stats.avg_correction_us, 200);
    }

    #[test]
    fn reset_clears_state() {
        let mut clock = Clock::new(Duration::from_millis(10));
        clock.start();
        clock.tick();
        clock.reset();
        assert!(clock.elapsed().is_none());
        assert_eq!(clock.current_drift_us(), 0);
    }

    #[test]
    fn set_interval_updates() {
        let mut clock = Clock::new(Duration::from_millis(10));
        clock.set_interval(Duration::from_millis(20));
        assert_eq!(clock.interval(), Duration::from_millis(20));
    }

    #[test]
    fn max_drift_forces_correction() {
        let mut clock = Clock::new(Duration::from_millis(10));
        // Set a huge drift that will survive the tick() elapsed subtraction
        clock.drift_us = 100_000;
        clock.start();
        clock.tick();
        // The elapsed since start is tiny, so drift remains very large (> 10ms max)
        assert!(clock.stats().corrections > 0);
    }

    #[test]
    fn tick_without_start_is_ok() {
        let mut clock = Clock::new(Duration::from_millis(10));
        clock.tick(); // should not panic
        assert!(clock.last_tick.is_some());
    }
}
