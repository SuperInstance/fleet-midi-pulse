//! Tick events and swing configuration.

use serde::{Deserialize, Serialize};

/// A single tick event emitted by the pulse.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TickEvent {
    /// Global tick counter (monotonically increasing).
    pub tick: u64,
    /// Current beat within the bar (0-indexed).
    pub beat: u32,
    /// Current bar number (0-indexed).
    pub bar: u64,
    /// Phase within the current beat [0.0, 1.0).
    pub phase: f64,
}

impl TickEvent {
    /// Create a new tick event.
    pub fn new(tick: u64, beat: u32, bar: u64, phase: f64) -> Self {
        Self {
            tick,
            beat,
            bar,
            phase: phase.clamp(0.0, 1.0),
        }
    }

    /// Create a tick event at the start (beat 0, bar 0, phase 0).
    pub fn zero() -> Self {
        Self::new(0, 0, 0, 0.0)
    }
}

/// Swing configuration for groove quantization.
///
/// A ratio of 0.5 = straight, >0.5 = swung (shuffle feel).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct SwingConfig {
    /// Swing ratio in range (0.0, 1.0). 0.5 = no swing.
    pub ratio: f64,
}

impl Default for SwingConfig {
    fn default() -> Self {
        Self { ratio: 0.5 }
    }
}

impl SwingConfig {
    /// Create a new swing config with the given ratio.
    pub fn new(ratio: f64) -> Self {
        Self { ratio }
    }

    /// Straight timing (no swing).
    pub fn straight() -> Self {
        Self { ratio: 0.5 }
    }

    /// Light shuffle feel.
    pub fn light_shuffle() -> Self {
        Self { ratio: 0.58 }
    }

    /// Heavy shuffle feel.
    pub fn heavy_shuffle() -> Self {
        Self { ratio: 0.67 }
    }

    /// Ternary feel (triplet-based).
    pub fn ternary() -> Self {
        Self { ratio: 2.0 / 3.0 }
    }

    /// Validate the swing ratio.
    pub fn validate(&self) -> Result<(), crate::PulseError> {
        if self.ratio <= 0.0 || self.ratio >= 1.0 {
            return Err(crate::PulseError::InvalidSwingRatio { ratio: self.ratio });
        }
        Ok(())
    }
}

/// Beats per bar (time signature numerator).
pub const DEFAULT_BEATS_PER_BAR: u32 = 4;

/// Default ticks per beat.
pub const DEFAULT_TICKS_PER_BEAT: u32 = 24;

/// Minimum BPM.
pub const MIN_BPM: f64 = 1.0;

/// Maximum BPM.
pub const MAX_BPM: f64 = 600.0;
