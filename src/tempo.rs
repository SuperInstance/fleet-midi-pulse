//! Tempo map with ramps (accelerando/ritardando) and tempo curves.

use serde::{Deserialize, Serialize};
use std::time::Duration;

/// A point in the tempo map.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TempoPoint {
    /// Tick at which this tempo takes effect.
    pub tick: u64,
    /// BPM at this point.
    pub bpm: f64,
}

impl TempoPoint {
    pub fn new(tick: u64, bpm: f64) -> Self {
        Self { tick, bpm }
    }
}

/// Interpolation curve for tempo ramps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum TempoCurve {
    /// Linear interpolation between points.
    #[default]
    Linear,
    /// Exponential ease-in.
    Exponential,
    /// Smooth ease-in-out.
    Smooth,
}

/// Tempo map supporting ramps and curves.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TempoMap {
    /// Sorted tempo points.
    points: Vec<TempoPoint>,
    /// Default curve for ramps.
    default_curve: TempoCurve,
}

impl Default for TempoMap {
    fn default() -> Self {
        Self::new()
    }
}

impl TempoMap {
    /// Create an empty tempo map.
    pub fn new() -> Self {
        Self {
            points: Vec::new(),
            default_curve: TempoCurve::Linear,
        }
    }

    /// Create a tempo map with a single constant BPM.
    pub fn constant(bpm: f64) -> Self {
        let mut map = Self::new();
        map.points.push(TempoPoint::new(0, bpm));
        map
    }

    /// Set the default interpolation curve.
    pub fn set_default_curve(&mut self, curve: TempoCurve) {
        self.default_curve = curve;
    }

    /// Add a tempo point. Points are kept sorted by tick.
    pub fn add_point(&mut self, point: TempoPoint) {
        self.points.push(point);
        self.points.sort_by_key(|p| p.tick);
    }

    /// Add an accelerando from `start_tick` to `end_tick`.
    pub fn accelerando(&mut self, start_tick: u64, start_bpm: f64, end_tick: u64, end_bpm: f64) {
        self.add_point(TempoPoint::new(start_tick, start_bpm));
        self.add_point(TempoPoint::new(end_tick, end_bpm));
    }

    /// Add a ritardando (same mechanism, just decreasing BPM).
    pub fn ritardando(&mut self, start_tick: u64, start_bpm: f64, end_tick: u64, end_bpm: f64) {
        self.add_point(TempoPoint::new(start_tick, start_bpm));
        self.add_point(TempoPoint::new(end_tick, end_bpm));
    }

    /// Get the BPM at a given tick, interpolating ramps.
    pub fn bpm_at(&self, tick: u64) -> f64 {
        if self.points.is_empty() {
            return 120.0; // sensible default
        }

        // Before first point: use first BPM
        if tick <= self.points[0].tick {
            return self.points[0].bpm;
        }

        // After last point: use last BPM
        if tick >= self.points[self.points.len() - 1].tick {
            return self.points[self.points.len() - 1].bpm;
        }

        // Find bracketing points
        let idx = self
            .points
            .iter()
            .position(|p| p.tick > tick)
            .unwrap_or(self.points.len());

        let prev = &self.points[idx - 1];
        let next = &self.points[idx];

        // If same tick, prefer later point
        if prev.tick == next.tick {
            return next.bpm;
        }

        let t = (tick - prev.tick) as f64 / (next.tick - prev.tick) as f64;
        self.interpolate(prev.bpm, next.bpm, t)
    }

    /// Interpolate between two BPMs using the default curve.
    fn interpolate(&self, a: f64, b: f64, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self.default_curve {
            TempoCurve::Linear => a + (b - a) * t,
            TempoCurve::Exponential => {
                if a == 0.0 {
                    return b * t;
                }
                a * (b / a).powf(t)
            }
            TempoCurve::Smooth => {
                // Hermite smoothstep
                let s = t * t * (3.0 - 2.0 * t);
                a + (b - a) * s
            }
        }
    }

    /// Compute the duration of a single tick at the given BPM and ticks-per-beat.
    pub fn tick_duration(bpm: f64, ticks_per_beat: u32) -> Duration {
        if bpm <= 0.0 || ticks_per_beat == 0 {
            return Duration::from_millis(10); // fallback
        }
        let beats_per_sec = bpm / 60.0;
        let ticks_per_sec = beats_per_sec * ticks_per_beat as f64;
        let micros = (1_000_000.0 / ticks_per_sec) as u64;
        Duration::from_micros(micros)
    }

    /// Get all tempo points.
    pub fn points(&self) -> &[TempoPoint] {
        &self.points
    }

    /// Clear all points.
    pub fn clear(&mut self) {
        self.points.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_tempo() {
        let map = TempoMap::constant(120.0);
        assert_eq!(map.bpm_at(0), 120.0);
        assert_eq!(map.bpm_at(1000), 120.0);
    }

    #[test]
    fn empty_map_default() {
        let map = TempoMap::new();
        assert_eq!(map.bpm_at(0), 120.0);
    }

    #[test]
    fn linear_ramp() {
        let mut map = TempoMap::new();
        map.set_default_curve(TempoCurve::Linear);
        map.add_point(TempoPoint::new(0, 100.0));
        map.add_point(TempoPoint::new(100, 200.0));
        // Midpoint should be 150
        assert!((map.bpm_at(50) - 150.0).abs() < 0.001);
    }

    #[test]
    fn accelerando() {
        let mut map = TempoMap::new();
        map.accelerando(0, 80.0, 1000, 160.0);
        assert!((map.bpm_at(500) - 120.0).abs() < 0.001);
    }

    #[test]
    fn ritardando() {
        let mut map = TempoMap::new();
        map.ritardando(0, 160.0, 1000, 80.0);
        assert!((map.bpm_at(500) - 120.0).abs() < 0.001);
    }

    #[test]
    fn exponential_curve() {
        let mut map = TempoMap::new();
        map.set_default_curve(TempoCurve::Exponential);
        map.add_point(TempoPoint::new(0, 100.0));
        map.add_point(TempoPoint::new(100, 200.0));
        let mid = map.bpm_at(50);
        // Exponential should be less than linear midpoint (150) for increasing ramp
        // Actually 100 * (200/100)^0.5 = 100 * sqrt(2) ≈ 141.4
        assert!((mid - 100.0 * 2.0_f64.powf(0.5)).abs() < 0.01);
    }

    #[test]
    fn smooth_curve() {
        let mut map = TempoMap::new();
        map.set_default_curve(TempoCurve::Smooth);
        map.add_point(TempoPoint::new(0, 100.0));
        map.add_point(TempoPoint::new(100, 200.0));
        let mid = map.bpm_at(50);
        // Smoothstep at 0.5 = 0.5, so same as linear at midpoint
        assert!((mid - 150.0).abs() < 0.001);
    }

    #[test]
    fn before_first_point() {
        let mut map = TempoMap::new();
        map.add_point(TempoPoint::new(100, 140.0));
        assert_eq!(map.bpm_at(0), 140.0);
        assert_eq!(map.bpm_at(50), 140.0);
    }

    #[test]
    fn after_last_point() {
        let mut map = TempoMap::new();
        map.add_point(TempoPoint::new(0, 120.0));
        assert_eq!(map.bpm_at(99999), 120.0);
    }

    #[test]
    fn multi_point_ramp() {
        let mut map = TempoMap::new();
        map.add_point(TempoPoint::new(0, 100.0));
        map.add_point(TempoPoint::new(100, 200.0));
        map.add_point(TempoPoint::new(200, 150.0));
        assert!((map.bpm_at(50) - 150.0).abs() < 0.001);
        assert!((map.bpm_at(150) - 175.0).abs() < 0.001);
    }

    #[test]
    fn tick_duration_120_bpm() {
        let dur = TempoMap::tick_duration(120.0, 24);
        // 120 BPM = 2 beats/sec, 48 ticks/sec = ~20833 µs
        assert_eq!(dur.as_micros(), 20833);
    }

    #[test]
    fn tick_duration_zero_bpm() {
        let dur = TempoMap::tick_duration(0.0, 24);
        assert_eq!(dur, Duration::from_millis(10)); // fallback
    }

    #[test]
    fn points_sorted() {
        let mut map = TempoMap::new();
        map.add_point(TempoPoint::new(200, 130.0));
        map.add_point(TempoPoint::new(0, 120.0));
        map.add_point(TempoPoint::new(100, 125.0));
        assert_eq!(map.points()[0].tick, 0);
        assert_eq!(map.points()[1].tick, 100);
        assert_eq!(map.points()[2].tick, 200);
    }

    #[test]
    fn clear_points() {
        let mut map = TempoMap::constant(120.0);
        map.clear();
        assert!(map.points().is_empty());
    }

    #[test]
    fn default_is_linear() {
        let map = TempoMap::default();
        assert_eq!(map.default_curve, TempoCurve::Linear);
    }
}
