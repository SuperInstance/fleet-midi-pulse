# fleet-midi-pulse

Heartbeat-driven timing layer for the [fleet-midi](https://github.com/SuperInstance) ecosystem — BPM, swing quantization, tempo ramps, fermata, and drift-corrected timing.

[![crates.io](https://img.shields.io/crates/v/fleet-midi-pulse.svg)](https://crates.io/crates/fleet-midi-pulse)
[![docs.rs](https://docs.rs/fleet-midi-pulse/badge.svg)](https://docs.rs/fleet-midi-pulse)

## Problem

MIDI timing is deceptively hard. A naive "sleep for N milliseconds per tick" approach drifts — OS scheduler jitter compounds, swing offsets clash with linear tick counts, and tempo changes need smooth interpolation without discontinuities. Existing solutions are either locked inside monolithic DAW frameworks or lack the precision guarantees that a fleet of coordinating agents needs.

## Insight

Separate the **pulse** (what time is it?) from the **agents** (what do we do at this time?). A single `Pulse` struct owns the clock, maintains a monotonically-increasing tick counter, and broadcasts `TickEvent` messages to any number of subscribers. Agents don't poll — they receive. The pulse handles:

- **Drift correction** — accumulates timing error and corrects it on subsequent ticks
- **Swing quantization** — non-linear phase mapping for shuffle/ternary feel
- **Tempo ramps** — interpolatable accelerando/ritardando with multiple curve types
- **Fermata** — pause/resume with duration tracking for expressive timing

## How It Works

### Tick Lifecycle

```
Pulse::start() → tick 0
  ├─ advance() → TickEvent { tick: 0, beat: 0, bar: 0, phase: 0.0 } → broadcast → tick becomes 1
  ├─ advance() → TickEvent { tick: 1, beat: 0, bar: 0, phase: 0.0417 } → broadcast → tick becomes 2
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

### Swing

Swing remaps the second half of each beat. A ratio of 0.5 = straight, 2/3 = ternary (triplet feel):

```
Straight:  |----|----|     equal halves
Swing:     |------|--|     first half stretched, second compressed
Ternary:   |--------||     ~2:1 ratio
```

### Tempo Ramps

Add control points to the tempo map. The pulse interpolates BPM at each tick:

```rust
pulse.tempo_map_mut().accelerando(0, 80.0, 4800, 160.0);
// Over 4800 ticks, BPM ramps from 80 → 160
```

Three curve types: `Linear`, `Exponential`, `Smooth` (hermite).

### Fermata

Pause the pulse indefinitely (like a fermata in sheet music), then resume. The clock restarts cleanly to avoid drift from the pause duration.

## Quick Start

```rust
use fleet_midi_pulse::{Pulse, SwingConfig};

let mut pulse = Pulse::new(120.0, 24); // 120 BPM, 24 ticks/beat

// Subscribe before starting
let rx = pulse.subscribe();

// Start the pulse
pulse.start().unwrap();

// Advance ticks (in a real app, sleep between advances)
loop {
    let event = pulse.advance().unwrap();
    println!("tick {} beat {} bar {} phase {:.3}",
             event.tick, event.beat, event.bar, event.phase);
    
    let sleep_duration = pulse.next_tick_duration();
    std::thread::sleep(sleep_duration);
}
```

### With Swing

```rust
pulse.swing_mut().set_config(SwingConfig::heavy_shuffle());
// Or ternary feel:
pulse.swing_mut().set_config(SwingConfig::ternary());
```

### With Tempo Ramp

```rust
use fleet_midi_pulse::tempo::{TempoCurve, TempoPoint};

pulse.tempo_map_mut().set_default_curve(TempoCurve::Smooth);
pulse.tempo_map_mut().accelerando(0, 100.0, 2400, 140.0);
pulse.tempo_map_mut().ritardando(2400, 140.0, 4800, 80.0);
```

### With Fermata

```rust
pulse.pause();  // freeze — advance() returns None
// ... hold for dramatic effect ...
pulse.resume(); // clock restarts cleanly, no drift
```

### Multiple Subscribers

```rust
let drums = pulse.subscribe();
let bass = pulse.subscribe();
let synths = pulse.subscribe();

// All three receive every TickEvent
pulse.advance(); // broadcasts to all three
```

## Module Map

| Module | Purpose |
|--------|---------|
| `pulse` | Core `Pulse` struct — BPM, tick counter, start/stop/pause, event generation |
| `event` | `TickEvent { tick, beat, bar, phase }`, `SwingConfig`, constants |
| `tempo` | `TempoMap` with `TempoPoint` control points, interpolation curves |
| `swing` | `SwingQuantizer` — phase remapping for shuffle/ternary feel |
| `fermata` | `Fermata` — pause/resume with duration tracking |
| `subscriber` | `SubscriberManager` + `PulseReceiver` — broadcast channel |
| `clock` | `Clock` — drift measurement, correction, timing stats |

## Design Decisions

**Tick-driven, not time-driven.** The pulse advances in discrete tick units. This makes it deterministic for testing and sequencing. The clock layer converts ticks to wall-clock time with drift correction.

**Broadcast, not request-response.** Subscribers receive events via `mpsc` channels. This decouples the pulse from agents and naturally handles slow consumers (they just fall behind).

**Drift correction, not drift prevention.** We can't prevent OS scheduler jitter. Instead, we measure actual elapsed time vs. expected, accumulate the error, and adjust the next sleep duration. This converges on accurate timing over time.

**Fermata resets the clock.** After a pause, we don't try to "catch up." The clock restarts from the resume point. This matches musical semantics — a fermata is a hold, not a pause that needs追赶.

**No unsafe.** The entire crate is safe Rust. Performance-critical paths use standard library primitives optimized by the compiler.

**Serde-friendly.** `TickEvent`, `SwingConfig`, `TempoPoint`, `TempoMap`, and `DriftStats` all derive `Serialize`/`Deserialize` for persistence and IPC.

## License

MIT
