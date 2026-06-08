//! # fleet-midi-pulse
//!
//! Heartbeat-driven timing layer for the fleet-midi ecosystem.
//!
//! Provides BPM-aware tick generation with swing quantization, tempo ramps,
//! fermata (pause/resume), and drift-corrected clock timing.

pub mod clock;
pub mod event;
pub mod fermata;
pub mod pulse;
pub mod subscriber;
pub mod swing;
pub mod tempo;

pub use clock::Clock;
pub use event::{SwingConfig, TickEvent};
pub use fermata::Fermata;
pub use pulse::Pulse;
pub use subscriber::PulseReceiver;
pub use swing::SwingQuantizer;
pub use tempo::{TempoMap, TempoPoint};

/// Errors for the pulse crate.
#[derive(Debug, thiserror::Error)]
pub enum PulseError {
    #[error("BPM {bpm} out of range [{min}, {max}]")]
    BpmOutOfRange { bpm: f64, min: f64, max: f64 },

    #[error("ticks per beat must be positive, got {value}")]
    InvalidTicksPerBeat { value: u32 },

    #[error("swing ratio {ratio} out of range (0.0, 1.0)")]
    InvalidSwingRatio { ratio: f64 },

    #[error("pulse is already {state}")]
    InvalidState { state: String },

    #[error("tempo ramp interpolation failed at tick {tick}")]
    RampInterpolationFailed { tick: u64 },

    #[error("channel closed")]
    ChannelClosed,
}
