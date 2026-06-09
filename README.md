# fleet-midi-pulse

Heartbeat-driven timing layer for the [fleet-midi](https://github.com/SuperInstance) ecosystem — BPM, swing quantization, tempo ramps, fermata, and drift-corrected clock timing.

[![Crates.io](https://img.shields.io/crates/v/fleet-midi-pulse.svg)](https://crates.io/crates/fleet-midi-pulse)
[![docs.rs](https://docs.rs/fleet-midi-pulse/badge.svg)](https://docs.rs/fleet-midi-pulse)

---

## Architecture

```
┌──────────────────────────────────────────────────────────────┐
│                          lib.rs                              │
│  Re-exports: Pulse, Clock, TickEvent, SwingConfig,           │
│              TempoMap, TempoPoint, TempoCurve,               │
│              SwingQuantizer, Fermata, PulseReceiver          │
└──────────────────────────────────────────────────────────────┘
     │              │              │              │
     ▼              ▼              ▼              ▼
┌──────────┐  ┌──────────┐  ┌──────────┐  ┌──────────┐
│  pulse   │  │  clock   │  │  event   │  │  tempo   │
│ (Core)   │  │(Drift)   │  │(Broadcast│  │(Ramps)   │
│BPM/Ticks │  │Correction│  │TickEvent)│  │Curves   │
└──────────┘  └──────────┘  └──────────┘  └──────────┘
     │                                    │
     ▼                                    ▼
┌──────────┐                        ┌──────────┐
│ subscriber│                        │  swing   │
│(mpsc)   │                        │(Quantize)│
└──────────┘                        └──────────┘
                                         │
                                         ▼
                                    ┌──────────┐
                                    │  fermata │
                                    │(Pause)   │
                                    └──────────┘
```

The crate separates **timing logic** from **musical semantics**. The `Pulse` struct owns the clock, maintains a monotonically-increasing tick counter, and broadcasts `TickEvent` messages to any number of subscribers. Agents don't poll — they receive. This decouples timing from action and makes the system deterministic for testing.

### Key Design Decisions

- **Tick-driven, not time-driven** — The pulse advances in discrete tick units. The clock layer converts ticks to wall-clock time with drift correction.
- **Broadcast, not request-response** — Subscribers receive events via `mpsc` channels. Slow consumers naturally fall behind without blocking the pulse.
- **Drift correction, not drift prevention** — OS scheduler jitter is measured, accumulated, and corrected on subsequent ticks. This converges on accurate timing over time.
- **Fermata resets the clock** — After a pause, the clock restarts from the resume point. No catch-up. This matches musical semantics — a fermata is a hold, not a delay.
- **No unsafe** — The entire crate is safe Rust. Performance-critical paths use standard library primitives.
- **Serde-friendly** — `TickEvent`, `SwingConfig`, `TempoPoint`, `TempoMap`, `DriftStats` all derive `Serialize`/`Deserialize` for persistence and IPC.

---

## Quick Start

```rust
use fleet_midi_pulse::{Pulse, SwingConfig};
use fleet_midi_pulse::tempo::{TempoCurve, TempoPoint};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // 1. Create pulse: 120 BPM, 24 ticks per beat
    let mut pulse = Pulse::new(120.0, 24);

    // 2. Subscribe before starting
    let rx = pulse.subscribe();

    // 3. Configure swing (optional)
    pulse.swing_mut().set_config(SwingConfig::heavy_shuffle());

    // 4. Configure tempo ramp (optional)
    pulse.tempo_map_mut().set_default_curve(TempoCurve::Smooth);
    pulse.tempo_map_mut().accelerando(0, 100.0, 2400, 140.0);
    pulse.tempo_map_mut().ritardando(2400, 140.0, 4800, 80.0);

    // 5. Start the pulse
    pulse.start()?;

    // 6. Advance ticks (in a real app, sleep between advances)
    loop {
        let event = pulse.advance().unwrap();
        println!("tick {} beat {} bar {} phase {:.3}",
                 event.tick, event.beat, event.bar, event.phase);

        let sleep_duration = pulse.next_tick_duration();
        std::thread::sleep(sleep_duration);
    }
}
```

### With Multiple Subscribers

```rust
let drums = pulse.subscribe();
let bass = pulse.subscribe();
let synths = pulse.subscribe();

// All three receive every TickEvent
pulse.advance(); // broadcasts to all three
```

### With Fermata

```rust
pulse.pause();  // freeze — advance() returns None
// ... hold for dramatic effect ...
pulse.resume(); // clock restarts cleanly, no drift
```

---

## API Reference

### Core Types

| Type | Fields | Description |
|------|--------|-------------|
| `Pulse` | `bpm, ticks_per_beat, beats_per_bar, tick, state, tempo_map, swing, fermata, subscribers, clock` | Core timing driver |
| `TickEvent` | `tick, beat, bar, phase` | Broadcast event per tick |
| `PulseState` | `Stopped / Running / Paused` | Lifecycle state |
| `Clock` | `interval, last_tick, drift_us, corrections` | Drift-corrected timer |
| `DriftStats` | `total_drift_us, corrections, avg_correction_us` | Timing accuracy metrics |
| `TempoMap` | `points, default_curve` | Piecewise tempo curve |
| `TempoPoint` | `tick, bpm` | Control point on tempo curve |
| `TempoCurve` | `Linear / Exponential / Smooth` | Interpolation curve |
| `SwingQuantizer` | `config` | Phase remapping for shuffle |
| `SwingConfig` | `ratio` | Swing ratio in `(0.0, 1.0)` |
| `Fermata` | `active, total_pause_duration_us, current_pause_duration_us` | Pause/resume tracker |
| `PulseReceiver` | `rx: Receiver<TickEvent>` | Subscriber channel |
| `PulseError` | `BpmOutOfRange / InvalidTicksPerBeat / InvalidSwingRatio / InvalidState / RampInterpolationFailed / ChannelClosed` | Typed errors |

### Pulse Lifecycle

| Function | Signature | Description |
|----------|-----------|-------------|
| `Pulse::new` | `fn new(bpm: f64, ticks_per_beat: u32) -> Self` | Create pulse |
| `Pulse::standard` | `fn standard() -> Self` | 120 BPM, default resolution |
| `Pulse::start` | `fn start(&mut self) -> Result<(), PulseError>` | Begin ticking |
| `Pulse::stop` | `fn stop(&mut self) -> Result<(), PulseError>` | Halt and reset |
| `Pulse::pause` | `fn pause(&mut self) -> bool` | Fermata pause |
| `Pulse::resume` | `fn resume(&mut self) -> bool` | Resume from fermata |
| `Pulse::advance` | `fn advance(&mut self) -> Option<TickEvent>` | Tick + broadcast |
| `Pulse::advance_n` | `fn advance_n(&mut self, n: u32) -> Vec<TickEvent>` | Bulk tick |
| `Pulse::current_event` | `fn current_event(&self) -> TickEvent` | Event without advancing |

### Configuration

| Function | Description |
|----------|-------------|
| `Pulse::set_bpm(bpm)` | Set constant BPM (validates range) |
| `Pulse::set_ticks_per_beat(ticks)` | Set tick resolution |
| `Pulse::set_beats_per_bar(beats)` | Set time signature |
| `Pulse::tempo_map_mut()` | Access tempo map for ramps |
| `Pulse::swing_mut()` | Access swing quantizer |
| `Pulse::subscribe()` | Create a new subscriber channel |
| `Pulse::subscriber_count()` | Number of active subscribers |
| `Pulse::next_tick_duration()` | Corrected sleep duration |
| `Pulse::clock_stats()` | Drift statistics |

### Tempo Map

| Function | Description |
|----------|-------------|
| `TempoMap::constant(bpm)` | Flat tempo |
| `TempoMap::add_point(point)` | Insert control point |
| `TempoMap::accelerando(start, start_bpm, end, end_bpm)` | Speed up |
| `TempoMap::ritardando(start, start_bpm, end, end_bpm)` | Slow down |
| `TempoMap::bpm_at(tick)` | Interpolated BPM at tick |
| `TempoMap::tick_duration(bpm, tpb)` | Convert BPM to `Duration` |

### Swing

| Function | Description |
|----------|-------------|
| `SwingConfig::straight()` | Ratio 0.5 — no swing |
| `SwingConfig::light_shuffle()` | Ratio 0.58 |
| `SwingConfig::heavy_shuffle()` | Ratio 0.67 |
| `SwingConfig::ternary()` | Ratio 2/3 — triplet feel |
| `SwingQuantizer::quantize_phase(phase)` | Remap phase for swing |
| `SwingQuantizer::half_durations(tpb)` | `(first_half_ticks, second_half_ticks)` |
| `SwingQuantizer::is_on_grid(tick, tpb)` | Check if tick lands on swung grid |

### Clock

| Function | Description |
|----------|-------------|
| `Clock::new(interval)` | Create clock |
| `Clock::start()` | Record reference time |
| `Clock::tick()` | Measure elapsed, update drift |
| `Clock::next_tick_duration()` | Sleep duration with correction |
| `Clock::reset()` | Clear all state |
| `Clock::stats()` | Drift statistics |

---

## Tick Lifecycle

```
Pulse::start() → tick 0
  ├─ advance() → TickEvent { tick: 0, beat: 0, bar: 0, phase: 0.0 }
  │              → broadcast → tick becomes 1
  ├─ advance() → TickEvent { tick: 1, beat: 0, bar: 0, phase: 0.0417 }
  │              → broadcast → tick becomes 2
  ├─ ... (24 ticks per beat)
  ├─ advance() → TickEvent { tick: 24, beat: 1, bar: 0, phase: 0.0 }
  └─ ...
```

Each tick:
1. Reads current BPM from tempo map (supports ramps)
2. Computes `TickEvent` with beat/bar/phase
3. Broadcasts to all subscribers
4. Clock measures actual elapsed time and accumulates drift
5. Next tick duration is adjusted to compensate

### Swing Timing

```
Straight:  |----|----|     equal halves (ratio 0.5)
Swing:     |------|--|     first half stretched, second compressed (ratio 0.67)
Ternary:   |--------||     ~2:1 ratio (ratio 0.667)
```

### Tempo Ramp Interpolation

Three curve types:
- **Linear** — `a + (b - a) * t`
- **Exponential** — `a * (b / a)^t`
- **Smooth** — Hermite smoothstep `t²(3 - 2t)`

---

## Integration Notes

### With Audio Thread Schedulers

Run `Pulse` on a dedicated timing thread and send `TickEvent`s to the audio callback:

```rust
use std::sync::mpsc;

let mut pulse = Pulse::new(120.0, 24);
let rx = pulse.subscribe();
pulse.start()?;

// Timing thread
std::thread::spawn(move || {
    loop {
        if let Some(event) = pulse.advance() {
            std::thread::sleep(pulse.next_tick_duration());
        }
    }
});

// Audio callback (or sequencer thread)
while let Ok(event) = rx.recv() {
    sequencer.tick(event); // schedule MIDI notes
}
```

### With DAWs

Export `TempoMap` as a tempo track:

```rust
let mut tempo_map = TempoMap::constant(120.0);
tempo_map.accelerando(0, 120.0, 960, 140.0);
tempo_map.ritardando(960, 140.0, 1920, 100.0);

let export: Vec<(u64, f64)> = tempo_map.points().iter()
    .map(|p| (p.tick, p.bpm))
    .collect();
// Write as MIDI tempo track or DAW project file
```

### With Live Performances

Use fermata for expressive pauses:

```rust
// Player hits sustain pedal
pulse.pause();

// Player releases sustain pedal
pulse.resume(); // clock restarts cleanly
```

The `Fermata` tracks total pause duration, useful for set-list timing analytics.

### With Grid-Based Sequencers

Use `SwingQuantizer` to align notes to a swung grid:

```rust
let quantizer = SwingQuantizer::new(SwingConfig::ternary());
let grid_tick = quantizer.quantize_tick(raw_tick, 24);
let on_grid = quantizer.is_on_grid(grid_tick, 24);
```

### With Distributed Agents

`TickEvent` derives `Serialize`/`Deserialize`. Broadcast over WebSocket or MQTT:

```rust
let event = pulse.current_event();
let bytes = bincode::serialize(&event)?;
mqtt.publish("fleet/midi/tick", bytes)?;
```

Subscribers on remote nodes deserialize and synchronize.

### With Logging / Telemetry

Export `DriftStats` to verify timing accuracy:

```rust
let stats = pulse.clock_stats();
println!("Corrections: {}, Avg correction: {} µs",
         stats.corrections, stats.avg_correction_us);
// Send to Prometheus / Datadog
```

---

## Module Map

| Module | Purpose | Key Types |
|--------|---------|-----------|
| `pulse` | Core `Pulse` struct — BPM, tick counter, lifecycle, event generation | `Pulse`, `PulseState` |
| `event` | `TickEvent { tick, beat, bar, phase }`, `SwingConfig`, constants | `TickEvent`, `SwingConfig` |
| `tempo` | `TempoMap` with `TempoPoint` control points, interpolation curves | `TempoMap`, `TempoPoint`, `TempoCurve` |
| `swing` | `SwingQuantizer` — phase remapping for shuffle/ternary feel | `SwingQuantizer` |
| `fermata` | `Fermata` — pause/resume with duration tracking | `Fermata` |
| `subscriber` | `SubscriberManager` + `PulseReceiver` — broadcast channel | `SubscriberManager`, `PulseReceiver` |
| `clock` | `Clock` — drift measurement, correction, timing stats | `Clock`, `DriftStats` |

---

## Testing

```bash
cargo test   # 60+ unit tests across all modules
cargo test pulse::tests::advance_increments_tick
cargo test pulse::tests::tempo_ramp_updates_bpm
cargo test pulse::tests::pause_and_resume
cargo test tempo::tests::linear_ramp
cargo test tempo::tests::exponential_curve
cargo test swing::tests::half_durations_swing
cargo test clock::tests::next_tick_corrects_drift
cargo test subscriber::tests::multiple_subscribers
cargo test fermata::tests::total_pause_accumulates
```

---

## License

MIT
