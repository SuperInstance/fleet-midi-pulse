//! Pulse — the core timing struct with BPM, ticks, and lifecycle.

use crate::clock::Clock;
use crate::event::{TickEvent, DEFAULT_BEATS_PER_BAR, DEFAULT_TICKS_PER_BEAT, MAX_BPM, MIN_BPM};
use crate::fermata::Fermata;
use crate::subscriber::{PulseReceiver, SubscriberManager};
use crate::swing::SwingQuantizer;
use crate::tempo::TempoMap;
use crate::PulseError;
use std::time::Duration;

/// Running state of the pulse.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PulseState {
    Stopped,
    Running,
    Paused,
}

/// The core Pulse struct that drives timing for the fleet-midi ecosystem.
pub struct Pulse {
    /// Current BPM.
    bpm: f64,
    /// Ticks per beat (resolution).
    ticks_per_beat: u32,
    /// Beats per bar (time signature).
    beats_per_bar: u32,
    /// Global tick counter.
    tick: u64,
    /// Current state.
    state: PulseState,
    /// Tempo map for tempo ramps.
    tempo_map: TempoMap,
    /// Swing quantizer.
    swing: SwingQuantizer,
    /// Fermata (pause/resume).
    fermata: Fermata,
    /// Subscriber manager.
    subscribers: SubscriberManager,
    /// Internal clock.
    clock: Clock,
}

impl Default for Pulse {
    fn default() -> Self {
        Self::new(120.0, DEFAULT_TICKS_PER_BEAT)
    }
}

impl Pulse {
    /// Create a new pulse at the given BPM and ticks-per-beat.
    pub fn new(bpm: f64, ticks_per_beat: u32) -> Self {
        let interval = TempoMap::tick_duration(bpm, ticks_per_beat);
        Self {
            bpm,
            ticks_per_beat,
            beats_per_bar: DEFAULT_BEATS_PER_BAR,
            tick: 0,
            state: PulseState::Stopped,
            tempo_map: TempoMap::constant(bpm),
            swing: SwingQuantizer::straight(),
            fermata: Fermata::new(),
            subscribers: SubscriberManager::new(),
            clock: Clock::new(interval),
        }
    }

    /// Create a pulse with 120 BPM and default resolution.
    pub fn standard() -> Self {
        Self::default()
    }

    /// Validate BPM is in range.
    pub fn validate_bpm(bpm: f64) -> Result<f64, PulseError> {
        if !(MIN_BPM..=MAX_BPM).contains(&bpm) {
            return Err(PulseError::BpmOutOfRange {
                bpm,
                min: MIN_BPM,
                max: MAX_BPM,
            });
        }
        Ok(bpm)
    }

    /// Validate ticks_per_beat is positive.
    pub fn validate_ticks_per_beat(ticks: u32) -> Result<u32, PulseError> {
        if ticks == 0 {
            return Err(PulseError::InvalidTicksPerBeat { value: 0 });
        }
        Ok(ticks)
    }

    // ── Lifecycle ──

    /// Start the pulse. Returns an error if already running.
    pub fn start(&mut self) -> Result<(), PulseError> {
        if self.state == PulseState::Running {
            return Err(PulseError::InvalidState {
                state: "running".into(),
            });
        }
        self.state = PulseState::Running;
        self.tick = 0;
        self.clock = Clock::new(TempoMap::tick_duration(self.bpm, self.ticks_per_beat));
        self.clock.start();
        Ok(())
    }

    /// Stop the pulse. Returns an error if already stopped.
    pub fn stop(&mut self) -> Result<(), PulseError> {
        if self.state == PulseState::Stopped {
            return Err(PulseError::InvalidState {
                state: "stopped".into(),
            });
        }
        self.state = PulseState::Stopped;
        self.clock.reset();
        Ok(())
    }

    /// Pause the pulse via fermata.
    pub fn pause(&mut self) -> bool {
        if self.state != PulseState::Running {
            return false;
        }
        self.state = PulseState::Paused;
        self.fermata.activate()
    }

    /// Resume from fermata pause.
    pub fn resume(&mut self) -> bool {
        if self.state != PulseState::Paused {
            return false;
        }
        self.fermata.deactivate();
        self.state = PulseState::Running;
        self.clock.start(); // restart clock to avoid drift from pause
        true
    }

    // ── Tick Advancement ──

    /// Advance by one tick and broadcast the event.
    /// Returns the tick event, or None if not running.
    pub fn advance(&mut self) -> Option<TickEvent> {
        if self.state != PulseState::Running {
            return None;
        }

        let event = self.current_event();
        self.subscribers.broadcast(event);
        self.clock.tick();
        self.tick += 1;

        // Update BPM from tempo map if ramping
        let bpm = self.tempo_map.bpm_at(self.tick);
        if (bpm - self.bpm).abs() > f64::EPSILON {
            self.bpm = bpm;
            self.clock.set_interval(TempoMap::tick_duration(bpm, self.ticks_per_beat));
        }

        Some(event)
    }

    /// Advance N ticks, returning all events.
    pub fn advance_n(&mut self, n: u32) -> Vec<TickEvent> {
        let mut events = Vec::with_capacity(n as usize);
        for _ in 0..n {
            if let Some(e) = self.advance() {
                events.push(e);
            }
        }
        events
    }

    /// Get the current tick event without advancing.
    pub fn current_event(&self) -> TickEvent {
        let tick = self.tick;
        let ticks_per_beat = self.ticks_per_beat as u64;
        let ticks_per_bar = ticks_per_beat * self.beats_per_bar as u64;

        let bar = tick / ticks_per_bar;
        let beat = ((tick % ticks_per_bar) / ticks_per_beat) as u32;
        let phase = if ticks_per_beat > 0 {
            (tick % ticks_per_beat) as f64 / ticks_per_beat as f64
        } else {
            0.0
        };

        TickEvent::new(tick, beat, bar, phase)
    }

    // ── Configuration ──

    /// Set the BPM. Returns error if out of range.
    pub fn set_bpm(&mut self, bpm: f64) -> Result<(), PulseError> {
        let bpm = Self::validate_bpm(bpm)?;
        self.bpm = bpm;
        self.tempo_map = TempoMap::constant(bpm);
        self.clock.set_interval(TempoMap::tick_duration(bpm, self.ticks_per_beat));
        Ok(())
    }

    /// Get the current BPM.
    pub fn bpm(&self) -> f64 {
        self.bpm
    }

    /// Set ticks per beat.
    pub fn set_ticks_per_beat(&mut self, ticks: u32) -> Result<(), PulseError> {
        let ticks = Self::validate_ticks_per_beat(ticks)?;
        self.ticks_per_beat = ticks;
        Ok(())
    }

    /// Get ticks per beat.
    pub fn ticks_per_beat(&self) -> u32 {
        self.ticks_per_beat
    }

    /// Set beats per bar (time signature).
    pub fn set_beats_per_bar(&mut self, beats: u32) {
        self.beats_per_bar = beats.max(1);
    }

    /// Get beats per bar.
    pub fn beats_per_bar(&self) -> u32 {
        self.beats_per_bar
    }

    /// Get the current tick counter.
    pub fn tick(&self) -> u64 {
        self.tick
    }

    /// Get the current state.
    pub fn state(&self) -> PulseState {
        self.state
    }

    /// Access the tempo map for configuration.
    pub fn tempo_map_mut(&mut self) -> &mut TempoMap {
        &mut self.tempo_map
    }

    /// Access the tempo map.
    pub fn tempo_map(&self) -> &TempoMap {
        &self.tempo_map
    }

    /// Access the swing quantizer.
    pub fn swing(&self) -> &SwingQuantizer {
        &self.swing
    }

    /// Access the swing quantizer mutably.
    pub fn swing_mut(&mut self) -> &mut SwingQuantizer {
        &mut self.swing
    }

    /// Access the fermata.
    pub fn fermata(&self) -> &Fermata {
        &self.fermata
    }

    /// Subscribe to tick events.
    pub fn subscribe(&mut self) -> PulseReceiver {
        self.subscribers.subscribe()
    }

    /// Get the number of subscribers.
    pub fn subscriber_count(&self) -> usize {
        self.subscribers.subscriber_count()
    }

    /// Get the next tick duration (with drift correction).
    pub fn next_tick_duration(&mut self) -> Duration {
        self.clock.next_tick_duration()
    }

    /// Get drift stats from the clock.
    pub fn clock_stats(&self) -> crate::clock::DriftStats {
        self.clock.stats()
    }

    /// Get the swing-adjusted tick interval for the current beat position.
    pub fn swung_interval(&self, base_interval: Duration) -> Duration {
        let tick_in_beat = (self.tick % self.ticks_per_beat as u64) as u32;
        let (first, second) = self.swing.half_durations(self.ticks_per_beat);

        if tick_in_beat < first {
            // First half: normal
            base_interval
        } else if tick_in_beat == first {
            // On the boundary: use second half duration
            let ratio = second as f64 / self.ticks_per_beat as f64;
            Duration::from_micros((base_interval.as_micros() as f64 * ratio * (self.ticks_per_beat as f64 / first as f64)) as u64)
        } else {
            base_interval
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_pulse() {
        let p = Pulse::new(120.0, 24);
        assert_eq!(p.bpm(), 120.0);
        assert_eq!(p.ticks_per_beat(), 24);
        assert_eq!(p.tick(), 0);
        assert_eq!(p.state(), PulseState::Stopped);
    }

    #[test]
    fn default_pulse() {
        let p = Pulse::default();
        assert_eq!(p.bpm(), 120.0);
        assert_eq!(p.ticks_per_beat(), DEFAULT_TICKS_PER_BEAT);
    }

    #[test]
    fn start_sets_running() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        assert_eq!(p.state(), PulseState::Running);
    }

    #[test]
    fn double_start_errors() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        assert!(p.start().is_err());
    }

    #[test]
    fn stop_sets_stopped() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        p.stop().unwrap();
        assert_eq!(p.state(), PulseState::Stopped);
    }

    #[test]
    fn double_stop_errors() {
        let mut p = Pulse::new(120.0, 24);
        assert!(p.stop().is_err());
    }

    #[test]
    fn advance_not_running_returns_none() {
        let mut p = Pulse::new(120.0, 24);
        assert!(p.advance().is_none());
    }

    #[test]
    fn advance_increments_tick() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        let e = p.advance().unwrap();
        assert_eq!(e.tick, 0);
        assert_eq!(p.tick(), 1);
    }

    #[test]
    fn advance_n() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        let events = p.advance_n(5);
        assert_eq!(events.len(), 5);
        assert_eq!(p.tick(), 5);
    }

    #[test]
    fn tick_event_beat_calculation() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        // Tick 0 = beat 0, bar 0
        let e = p.advance().unwrap();
        assert_eq!(e.beat, 0);
        assert_eq!(e.bar, 0);
        assert_eq!(e.phase, 0.0);

        // Advance to tick 24 = beat 1
        p.advance_n(23);
        let e = p.advance().unwrap();
        assert_eq!(e.tick, 24);
        assert_eq!(e.beat, 1);
        assert_eq!(e.bar, 0);
    }

    #[test]
    fn tick_event_bar_calculation() {
        let mut p = Pulse::new(120.0, 24);
        p.set_beats_per_bar(4);
        p.start().unwrap();
        // 24 ticks/beat * 4 beats/bar = 96 ticks/bar
        // Advance 95 to get to tick 95, then advance to tick 96
        // After 96 advances, tick counter is 96
        // The last advance() returned event for tick 95 (before increment)
        // We want tick 96 which is beat 1 of bar 1
        // Actually tick counter advances AFTER event creation
        // So after advance_n(96), tick=96 and the last event was at tick 95
        // We need advance_n(97) to see tick 96
        p.advance_n(96);
        // tick is now 96. current_event() shows tick 96
        let e = p.current_event();
        assert_eq!(e.tick, 96);
        assert_eq!(e.beat, 0); // tick 96 % 96 = 0, so beat 0
        assert_eq!(e.bar, 1);
    }

    #[test]
    fn set_bpm() {
        let mut p = Pulse::new(120.0, 24);
        p.set_bpm(140.0).unwrap();
        assert_eq!(p.bpm(), 140.0);
    }

    #[test]
    fn set_bpm_out_of_range() {
        let mut p = Pulse::new(120.0, 24);
        assert!(p.set_bpm(0.5).is_err());
        assert!(p.set_bpm(700.0).is_err());
    }

    #[test]
    fn set_ticks_per_beat() {
        let mut p = Pulse::new(120.0, 24);
        p.set_ticks_per_beat(48).unwrap();
        assert_eq!(p.ticks_per_beat(), 48);
    }

    #[test]
    fn set_ticks_per_beat_zero_errors() {
        let mut p = Pulse::new(120.0, 24);
        assert!(p.set_ticks_per_beat(0).is_err());
    }

    #[test]
    fn pause_and_resume() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        assert!(p.pause());
        assert_eq!(p.state(), PulseState::Paused);
        assert!(p.resume());
        assert_eq!(p.state(), PulseState::Running);
    }

    #[test]
    fn pause_not_running_fails() {
        let mut p = Pulse::new(120.0, 24);
        assert!(!p.pause());
    }

    #[test]
    fn resume_not_paused_fails() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        assert!(!p.resume());
    }

    #[test]
    fn advance_while_paused_returns_none() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        p.pause();
        assert!(p.advance().is_none());
    }

    #[test]
    fn subscribe_receives_events() {
        let mut p = Pulse::new(120.0, 24);
        let rx = p.subscribe();
        p.start().unwrap();
        p.advance();
        let event = rx.try_recv().unwrap();
        assert_eq!(event.tick, 0);
    }

    #[test]
    fn multiple_subscribers() {
        let mut p = Pulse::new(120.0, 24);
        let rx1 = p.subscribe();
        let rx2 = p.subscribe();
        p.start().unwrap();
        p.advance();
        assert_eq!(rx1.try_recv().unwrap().tick, 0);
        assert_eq!(rx2.try_recv().unwrap().tick, 0);
    }

    #[test]
    fn tempo_ramp_updates_bpm() {
        let mut p = Pulse::new(100.0, 24);
        p.start().unwrap();
        p.tempo_map_mut().accelerando(0, 100.0, 100, 200.0);
        p.advance_n(50);
        // At tick 50, BPM should be 150 (linear)
        assert!((p.bpm() - 150.0).abs() < 0.1);
    }

    #[test]
    fn validate_bpm_boundary() {
        assert!(Pulse::validate_bpm(1.0).is_ok());
        assert!(Pulse::validate_bpm(600.0).is_ok());
        assert!(Pulse::validate_bpm(0.99).is_err());
        assert!(Pulse::validate_bpm(600.1).is_err());
    }

    #[test]
    fn current_event_no_advance() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        let e = p.current_event();
        assert_eq!(e.tick, 0);
        assert_eq!(p.tick(), 0); // not advanced
    }

    #[test]
    fn stop_resets_state() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        p.advance_n(10);
        p.stop().unwrap();
        assert_eq!(p.state(), PulseState::Stopped);
    }

    #[test]
    fn restart_resets_tick() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        p.advance_n(10);
        p.stop().unwrap();
        p.start().unwrap();
        assert_eq!(p.tick(), 0);
    }

    #[test]
    fn fermata_tracking() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        p.pause();
        assert!(p.fermata().is_active());
        p.resume();
        assert!(!p.fermata().is_active());
    }

    #[test]
    fn set_beats_per_bar() {
        let mut p = Pulse::new(120.0, 24);
        p.set_beats_per_bar(3);
        assert_eq!(p.beats_per_bar(), 3);
    }

    #[test]
    fn set_beats_per_bar_minimum() {
        let mut p = Pulse::new(120.0, 24);
        p.set_beats_per_bar(0);
        assert_eq!(p.beats_per_bar(), 1);
    }

    #[test]
    fn phase_calculation() {
        let mut p = Pulse::new(120.0, 24);
        p.start().unwrap();
        // tick 12 of 24 = phase 0.5
        p.advance_n(12);
        let e = p.current_event();
        assert!((e.phase - 0.5).abs() < 0.001);
    }
}
