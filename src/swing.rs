//! Swing quantization for shuffle and ternary feel.

use crate::event::SwingConfig;

/// Swing quantizer applies groove timing offsets to ticks.
#[derive(Debug, Clone, PartialEq)]
pub struct SwingQuantizer {
    config: SwingConfig,
}

impl SwingQuantizer {
    /// Create a new quantizer with the given swing config.
    pub fn new(config: SwingConfig) -> Self {
        Self { config }
    }

    /// Create a straight (no swing) quantizer.
    pub fn straight() -> Self {
        Self::new(SwingConfig::straight())
    }

    /// Return the current swing config.
    pub fn config(&self) -> &SwingConfig {
        &self.config
    }

    /// Update the swing config.
    pub fn set_config(&mut self, config: SwingConfig) {
        self.config = config;
    }

    /// Given a phase within a beat (0.0–1.0) and the total ticks per beat,
    /// return the swung phase offset.
    ///
    /// Swing shifts the second half of the beat later (ratio > 0.5)
    /// or earlier (ratio < 0.5).
    pub fn quantize_phase(&self, phase: f64) -> f64 {
        if self.config.ratio == 0.5 || phase < 0.5 {
            return phase;
        }
        // Remap the second half [0.5, 1.0) → [ratio, 1.0)
        let half_phase = (phase - 0.5) / 0.5; // normalized 0..1 in second half
        self.config.ratio + half_phase * (1.0 - self.config.ratio)
    }

    /// Quantize a tick position within a beat to the nearest swung tick.
    ///
    /// `tick_in_beat` is 0..ticks_per_beat. Returns the swung tick position.
    pub fn quantize_tick(&self, tick_in_beat: u32, ticks_per_beat: u32) -> u32 {
        if ticks_per_beat == 0 || self.config.ratio == 0.5 {
            return tick_in_beat;
        }

        let phase = tick_in_beat as f64 / ticks_per_beat as f64;
        let swung = self.quantize_phase(phase);
        (swung * ticks_per_beat as f64).round() as u32
    }

    /// Compute the effective tick duration for the given half of the beat.
    ///
    /// Returns (first_half_ticks, second_half_ticks) summing to ticks_per_beat.
    pub fn half_durations(&self, ticks_per_beat: u32) -> (u32, u32) {
        let first = (self.config.ratio * ticks_per_beat as f64).round() as u32;
        let second = ticks_per_beat.saturating_sub(first);
        (first, second)
    }

    /// Check if a tick lands on a swung "on-beat" position (start or midpoint).
    pub fn is_on_grid(&self, tick_in_beat: u32, ticks_per_beat: u32) -> bool {
        if tick_in_beat == 0 {
            return true;
        }
        let (first, _) = self.half_durations(ticks_per_beat);
        tick_in_beat == first
    }
}

impl Default for SwingQuantizer {
    fn default() -> Self {
        Self::straight()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn straight_no_change() {
        let q = SwingQuantizer::straight();
        assert_eq!(q.quantize_phase(0.0), 0.0);
        assert_eq!(q.quantize_phase(0.25), 0.25);
        assert_eq!(q.quantize_phase(0.5), 0.5);
        assert_eq!(q.quantize_phase(0.75), 0.75);
    }

    #[test]
    fn swing_shifts_second_half() {
        let q = SwingQuantizer::new(SwingConfig { ratio: 0.67 });
        let swung = q.quantize_phase(0.75);
        // 0.67 + 0.5 * (1.0 - 0.67) = 0.67 + 0.165 = 0.835
        assert!((swung - 0.835).abs() < 0.001);
    }

    #[test]
    fn first_half_unchanged() {
        let q = SwingQuantizer::new(SwingConfig { ratio: 0.67 });
        assert_eq!(q.quantize_phase(0.25), 0.25);
        assert_eq!(q.quantize_phase(0.0), 0.0);
    }

    #[test]
    fn ternary_feel() {
        let q = SwingQuantizer::new(SwingConfig::ternary());
        // ratio = 2/3, phase 0.75 → 0.6667 + 0.5 * 0.3333 = 0.8333
        let swung = q.quantize_phase(0.75);
        assert!((swung - (2.0 / 3.0 + 0.5 / 3.0)).abs() < 0.001);
    }

    #[test]
    fn half_durations_straight() {
        let q = SwingQuantizer::straight();
        let (first, second) = q.half_durations(24);
        assert_eq!(first, 12);
        assert_eq!(second, 12);
    }

    #[test]
    fn half_durations_swing() {
        let q = SwingQuantizer::new(SwingConfig { ratio: 0.67 });
        let (first, second) = q.half_durations(24);
        // 0.67 * 24 ≈ 16
        assert_eq!(first, 16);
        assert_eq!(second, 8);
    }

    #[test]
    fn quantize_tick_straight() {
        let q = SwingQuantizer::straight();
        assert_eq!(q.quantize_tick(6, 24), 6);
        assert_eq!(q.quantize_tick(18, 24), 18);
    }

    #[test]
    fn is_on_grid_start() {
        let q = SwingQuantizer::new(SwingConfig { ratio: 0.67 });
        assert!(q.is_on_grid(0, 24));
    }

    #[test]
    fn is_on_grid_midpoint() {
        let q = SwingQuantizer::new(SwingConfig { ratio: 0.67 });
        assert!(q.is_on_grid(16, 24)); // first half = 16
    }

    #[test]
    fn is_not_on_grid() {
        let q = SwingQuantizer::new(SwingConfig { ratio: 0.67 });
        assert!(!q.is_on_grid(8, 24));
    }

    #[test]
    fn default_is_straight() {
        let q = SwingQuantizer::default();
        assert_eq!(q.config().ratio, 0.5);
    }

    #[test]
    fn set_config_updates() {
        let mut q = SwingQuantizer::straight();
        q.set_config(SwingConfig::heavy_shuffle());
        assert_eq!(q.config().ratio, 0.67);
    }
}
