//! Fermata — pause/resume with duration tracking.

use serde::{Deserialize, Serialize};
use std::time::{Duration, Instant};

/// Fermata state: pause/resume with duration tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Fermata {
    /// Whether the fermata is currently active (paused).
    active: bool,
    /// Total accumulated pause duration across all activations.
    total_pause_duration_us: u64,
    /// Duration of the current (or most recent) pause in microseconds.
    current_pause_duration_us: u64,
    /// Tick at which the fermata was activated.
    #[serde(skip)]
    activated_at: Option<FermataInstant>,
}

/// Serializable instant wrapper for fermata timing.
#[derive(Debug, Clone, Copy)]
struct FermataInstant {
    inner: Instant,
}

impl FermataInstant {
    fn now() -> Self {
        Self { inner: Instant::now() }
    }

    fn elapsed(&self) -> Duration {
        self.inner.elapsed()
    }
}

impl Default for Fermata {
    fn default() -> Self {
        Self::new()
    }
}

impl Fermata {
    /// Create a new inactive fermata.
    pub fn new() -> Self {
        Self {
            active: false,
            total_pause_duration_us: 0,
            current_pause_duration_us: 0,
            activated_at: None,
        }
    }

    /// Activate (pause). Returns false if already active.
    pub fn activate(&mut self) -> bool {
        if self.active {
            return false;
        }
        self.active = true;
        self.activated_at = Some(FermataInstant::now());
        true
    }

    /// Deactivate (resume). Returns the duration of this pause, or None if wasn't active.
    pub fn deactivate(&mut self) -> Option<Duration> {
        if !self.active {
            return None;
        }
        let duration = self.activated_at.map(|i| i.elapsed()).unwrap_or(Duration::ZERO);
        self.current_pause_duration_us = duration.as_micros() as u64;
        self.total_pause_duration_us += self.current_pause_duration_us;
        self.active = false;
        self.activated_at = None;
        Some(duration)
    }

    /// Whether the fermata is currently active.
    pub fn is_active(&self) -> bool {
        self.active
    }

    /// Total accumulated pause duration across all activations.
    pub fn total_pause_duration(&self) -> Duration {
        Duration::from_micros(self.total_pause_duration_us)
    }

    /// Duration of the most recent pause.
    pub fn last_pause_duration(&self) -> Duration {
        Duration::from_micros(self.current_pause_duration_us)
    }

    /// Get current pause duration if active, or last pause duration.
    pub fn current_duration(&self) -> Duration {
        if let Some(ref instant) = self.activated_at {
            instant.elapsed()
        } else {
            self.last_pause_duration()
        }
    }

    /// Reset all fermata state.
    pub fn reset(&mut self) {
        self.active = false;
        self.total_pause_duration_us = 0;
        self.current_pause_duration_us = 0;
        self.activated_at = None;
    }

    /// Number of times the fermata has been activated (approximate, via duration tracking).
    /// Returns true if there has been any pause.
    pub fn has_paused(&self) -> bool {
        self.total_pause_duration_us > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_is_inactive() {
        let f = Fermata::new();
        assert!(!f.is_active());
    }

    #[test]
    fn activate_sets_active() {
        let mut f = Fermata::new();
        assert!(f.activate());
        assert!(f.is_active());
    }

    #[test]
    fn double_activate_fails() {
        let mut f = Fermata::new();
        f.activate();
        assert!(!f.activate());
    }

    #[test]
    fn deactivate_returns_duration() {
        let mut f = Fermata::new();
        f.activate();
        let dur = f.deactivate();
        assert!(dur.is_some());
        assert!(!f.is_active());
    }

    #[test]
    fn deactivate_inactive_returns_none() {
        let mut f = Fermata::new();
        assert!(f.deactivate().is_none());
    }

    #[test]
    fn total_pause_accumulates() {
        let mut f = Fermata::new();
        f.activate();
        std::thread::sleep(Duration::from_micros(100));
        let _ = f.deactivate();
        f.activate();
        std::thread::sleep(Duration::from_micros(100));
        let _ = f.deactivate();
        assert!(f.total_pause_duration() > Duration::ZERO);
        assert!(f.has_paused());
    }

    #[test]
    fn reset_clears_all() {
        let mut f = Fermata::new();
        f.activate();
        let _ = f.deactivate();
        f.reset();
        assert!(!f.is_active());
        assert!(!f.has_paused());
        assert_eq!(f.total_pause_duration(), Duration::ZERO);
    }

    #[test]
    fn current_duration_while_active() {
        let mut f = Fermata::new();
        f.activate();
        // Should be very small but non-zero
        std::thread::sleep(Duration::from_millis(1));
        let dur = f.current_duration();
        assert!(dur >= Duration::from_millis(1));
    }

    #[test]
    fn last_pause_after_deactivate() {
        let mut f = Fermata::new();
        f.activate();
        std::thread::sleep(Duration::from_millis(2));
        f.deactivate();
        let last = f.last_pause_duration();
        assert!(last >= Duration::from_millis(2));
    }

    #[test]
    fn default_is_new() {
        let f = Fermata::default();
        assert!(!f.is_active());
        assert_eq!(f.total_pause_duration(), Duration::ZERO);
    }
}
