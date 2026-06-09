//! # Fleet MIDI Pulse Tutorial
//!
//! A progressive walkthrough of the heartbeat-driven timing layer.
//! Provides BPM-aware tick generation with swing quantization, tempo ramps,
//! fermata (pause/resume), and drift-corrected clock timing.
//!
//! ## Lessons
//!
//! 1. Tempo & tick math — BPM, ticks per beat, tick duration
//! 2. The Pulse lifecycle — start, advance, pause, resume, stop
//! 3. Tick events — beat, bar, and phase tracking
//! 4. Subscribers — broadcast channels for tick events
//! 5. Swing quantization — shuffle and ternary feel
//! 6. Tempo maps — accelerando, ritardando, and curves
//! 7. Fermata — pause/resume with duration tracking
//! 8. Drift correction — internal clock stability

use fleet_midi_pulse::clock::{Clock, DriftStats};
use fleet_midi_pulse::event::{TickEvent, SwingConfig, DEFAULT_TICKS_PER_BEAT};
use fleet_midi_pulse::fermata::Fermata;
use fleet_midi_pulse::pulse::Pulse;
use fleet_midi_pulse::swing::SwingQuantizer;
use fleet_midi_pulse::tempo::{TempoMap, TempoPoint, TempoCurve};
use fleet_midi_pulse::subscriber::SubscriberManager;

use std::time::Duration;

fn main() {
    println!("════════════════════════════════════════════════════════");
    println!("  FLEET MIDI PULSE TUTORIAL");
    println!("  Heartbeat-Driven Timing Layer");
    println!("════════════════════════════════════════════════════════\n");

    lesson_1_tempo_math();
    lesson_2_pulse_lifecycle();
    lesson_3_tick_events();
    lesson_4_subscribers();
    lesson_5_swing();
    lesson_6_tempo_maps();
    lesson_7_fermata();
    lesson_8_drift_correction();

    println!("\n✅ Tutorial complete! The pulse beats on.");
}

// ─── Lesson 1: Tempo & Tick Math ──────────────────────────────────────

fn lesson_1_tempo_math() {
    println!("━━━ Lesson 1: Tempo & Tick Math ━━━\n");
    println!("At the core: BPM (beats per minute), ticks per beat, and tick duration.\n");

    // Tick duration at different BPMs
    for bpm in [60.0, 90.0, 120.0, 140.0, 180.0] {
        let dur = TempoMap::tick_duration(bpm, 24);
        println!("  {:.0} BPM, 24 tpb: tick = {} µs ({:.2} ms)",
            bpm, dur.as_micros(), dur.as_secs_f64() * 1000.0);
    }

    // MIDI resolution: 24 ticks per beat (standard)
    println!("\n  Standard MIDI resolution: {} ticks/beat", DEFAULT_TICKS_PER_BEAT);
    let dur_120 = TempoMap::tick_duration(120.0, 24);
    println!("  At 120 BPM: tick = {} µs ≈ {:.2} ms",
        dur_120.as_micros(), dur_120.as_secs_f64() * 1000.0);

    // Higher resolution
    let dur_120_480 = TempoMap::tick_duration(120.0, 480);
    println!("  At 120 BPM, 480 tpb: tick = {} µs", dur_120_480.as_micros());

    // Edge cases
    let dur_zero = TempoMap::tick_duration(0.0, 24);
    println!("\n  Edge case (0 BPM): fallback = {} ms", dur_zero.as_millis());

    println!();
}

// ─── Lesson 2: Pulse Lifecycle ──────────────────────────────────────

fn lesson_2_pulse_lifecycle() {
    println!("━━━ Lesson 2: Pulse Lifecycle ━━━\n");
    println!("The Pulse drives timing through a lifecycle: start → advance → pause → resume → stop.\n");

    let mut pulse = Pulse::new(120.0, 24);
    println!("  Created pulse: {} BPM, {} ticks/beat", pulse.bpm(), pulse.ticks_per_beat());
    println!("  State: {:?}", pulse.state());

    // Start
    pulse.start().unwrap();
    println!("\n  Started! State: {:?}", pulse.state());

    // Advance a few ticks
    for _i in 0..5 {
        let event = pulse.advance().unwrap();
        println!("  Tick {}: beat={}, bar={}, phase={:.2}",
            event.tick, event.beat, event.bar, event.phase);
    }

    // Pause
    assert!(pulse.pause());
    println!("\n  Paused! State: {:?}", pulse.state());
    assert!(pulse.advance().is_none());
    println!("  Advance while paused: None ✓");

    // Resume
    assert!(pulse.resume());
    println!("  Resumed! State: {:?}", pulse.state());
    let event = pulse.advance().unwrap();
    println!("  Tick {} after resume", event.tick);

    // Stop
    pulse.stop().unwrap();
    println!("\n  Stopped! State: {:?}", pulse.state());

    // BPM validation
    println!("\n  BPM validation:");
    assert!(Pulse::validate_bpm(120.0).is_ok());
    assert!(Pulse::validate_bpm(0.5).is_err());
    assert!(Pulse::validate_bpm(600.0).is_ok());
    assert!(Pulse::validate_bpm(600.1).is_err());
    println!("    120.0 ✓, 0.5 ✗, 600.0 ✓, 600.1 ✗");

    // Changing BPM
    let mut pulse2 = Pulse::standard();
    pulse2.set_bpm(140.0).unwrap();
    println!("\n  Changed BPM to {}", pulse2.bpm());

    // Time signature
    pulse2.set_beats_per_bar(3);
    println!("  Set 3/4 time: {} beats/bar", pulse2.beats_per_bar());
    pulse2.set_beats_per_bar(0); // minimum is 1
    println!("  Tried 0 beats/bar: got {} (minimum)", pulse2.beats_per_bar());

    println!();
}

// ─── Lesson 3: Tick Events ──────────────────────────────────────

fn lesson_3_tick_events() {
    println!("━━━ Lesson 3: Tick Events ━━━\n");
    println!("Every tick produces a TickEvent with beat, bar, and phase info.\n");

    // Manual tick event
    let event = TickEvent::new(42, 1, 0, 0.5);
    println!("  Manual event: tick={}, beat={}, bar={}, phase={}",
        event.tick, event.beat, event.bar, event.phase);

    let zero = TickEvent::zero();
    println!("  Zero event: tick={}, beat={}, bar={}, phase={}",
        zero.tick, zero.beat, zero.bar, zero.phase);

    // Advance through a full bar at 120 BPM, 4/4 time, 24 tpb
    let mut pulse = Pulse::new(120.0, 24);
    pulse.set_beats_per_bar(4);
    pulse.start().unwrap();

    println!("\n  Walking through one bar (96 ticks = 4 beats × 24 tpb):");
    let mut last_beat = 0u32;
    let mut last_bar = 0u64;

    for _ in 0..96 {
        let e = pulse.advance().unwrap();
        if e.beat != last_beat || e.bar != last_bar {
            println!("    tick {}: beat {}, bar {}, phase {:.2}",
                e.tick, e.beat, e.bar, e.phase);
            last_beat = e.beat;
            last_bar = e.bar;
        }
    }

    // Phase progression within a beat
    println!("\n  Phase progression within beat 0:");
    let mut pulse2 = Pulse::new(120.0, 12); // 12 tpb for clarity
    pulse2.start().unwrap();
    for _ in 0..12 {
        let e = pulse2.advance().unwrap();
        print!("  {:.2}", e.phase);
    }
    println!();

    // advance_n
    let mut pulse3 = Pulse::new(120.0, 24);
    pulse3.start().unwrap();
    let events = pulse3.advance_n(10);
    println!("\n  advance_n(10): {} events, ticks {}-{}",
        events.len(), events[0].tick, events[9].tick);

    // current_event without advancing
    let mut pulse4 = Pulse::new(120.0, 24);
    pulse4.start().unwrap();
    let peek = pulse4.current_event();
    println!("  current_event (no advance): tick={}", peek.tick);
    println!("  tick counter still: {}", pulse4.tick());

    println!();
}

// ─── Lesson 4: Subscribers ──────────────────────────────────────

fn lesson_4_subscribers() {
    println!("━━━ Lesson 4: Subscribers ━━━\n");
    println!("Subscribe to tick events via broadcast channels.\n");

    let mut pulse = Pulse::new(120.0, 24);

    // Subscribe before starting
    let rx1 = pulse.subscribe();
    let rx2 = pulse.subscribe();
    println!("  Subscribed 2 receivers ({})", pulse.subscriber_count());

    pulse.start().unwrap();

    // Advance and both receive
    pulse.advance();
    let e1 = rx1.try_recv().unwrap();
    let e2 = rx2.try_recv().unwrap();
    println!("  Both received tick {}", e1.tick);
    assert_eq!(e1, e2);

    // Advance more
    pulse.advance_n(3);
    println!("  After advance_n(3): 4 events in each queue");
    for i in 0..3 {
        let e = rx1.try_recv().unwrap();
        println!("    Event {}: tick={}", i + 1, e.tick);
    }

    // Third subscriber joins late
    let rx3 = pulse.subscribe();
    println!("\n  Third subscriber joined (total: {})", pulse.subscriber_count());
    pulse.advance();
    // rx1 and rx2 have pending, rx3 gets the new one
    let _ = rx1.try_recv();
    let _ = rx2.try_recv();
    let e3 = rx3.try_recv().unwrap();
    println!("  Late subscriber received tick {}", e3.tick);

    // Direct subscriber manager usage
    println!("\n  Direct SubscriberManager:");
    let mut mgr = SubscriberManager::new();
    let s1 = mgr.subscribe();
    let s2 = mgr.subscribe();
    println!("    {} subscribers", mgr.subscriber_count());

    let event = TickEvent::new(99, 2, 1, 0.75);
    let sent = mgr.broadcast(event);
    println!("    Broadcast to {} receivers", sent);
    assert_eq!(s1.try_recv().unwrap(), event);
    assert_eq!(s2.try_recv().unwrap(), event);

    // Disconnected subscriber cleanup
    drop(s1);
    let sent2 = mgr.broadcast(TickEvent::zero());
    println!("    After disconnect: broadcast to {}", sent2);

    mgr.clear();
    println!("    Cleared: {} subscribers", mgr.subscriber_count());

    println!();
}

// ─── Lesson 5: Swing Quantization ──────────────────────────────────────

fn lesson_5_swing() {
    println!("━━━ Lesson 5: Swing Quantization ━━━\n");
    println!("Swing shifts the second half of each beat, creating groove.\n");

    // Straight timing (no swing)
    let straight = SwingQuantizer::straight();
    println!("  Straight (ratio=0.5):");
    for phase in [0.0, 0.25, 0.5, 0.75] {
        println!("    phase {:.2} → {:.2}", phase, straight.quantize_phase(phase));
    }

    // Light shuffle
    let light = SwingQuantizer::new(SwingConfig::light_shuffle());
    println!("\n  Light shuffle (ratio=0.58):");
    for phase in [0.0, 0.25, 0.5, 0.75] {
        let swung = light.quantize_phase(phase);
        let shift = swung - phase;
        println!("    phase {:.2} → {:.2} (shift: {:+.3})", phase, swung, shift);
    }

    // Heavy shuffle
    let heavy = SwingQuantizer::new(SwingConfig::heavy_shuffle());
    println!("\n  Heavy shuffle (ratio=0.67):");
    for phase in [0.0, 0.25, 0.5, 0.75] {
        let swung = heavy.quantize_phase(phase);
        println!("    phase {:.2} → {:.2}", phase, swung);
    }

    // Ternary (triplet feel)
    let ternary = SwingQuantizer::new(SwingConfig::ternary());
    println!("\n  Ternary (ratio=2/3):");
    for phase in [0.0, 0.25, 0.5, 0.75] {
        println!("    phase {:.2} → {:.2}", phase, ternary.quantize_phase(phase));
    }

    // Half durations
    println!("\n  Half durations (24 tpb):");
    for (name, q) in [
        ("Straight", SwingQuantizer::straight()),
        ("Light", SwingQuantizer::new(SwingConfig::light_shuffle())),
        ("Heavy", SwingQuantizer::new(SwingConfig::heavy_shuffle())),
        ("Ternary", SwingQuantizer::new(SwingConfig::ternary())),
    ] {
        let (first, second) = q.half_durations(24);
        println!("    {}: first half = {} ticks, second = {} ticks", name, first, second);
    }

    // Grid detection
    println!("\n  On-grid positions (heavy shuffle, 24 tpb):");
    for tick in 0..24 {
        if heavy.is_on_grid(tick, 24) {
            print!(" tick={}", tick);
        }
    }
    println!(" (beat start + swung midpoint)");

    // Tick quantization
    let quantized = heavy.quantize_tick(12, 24);
    println!("\n  quantize_tick(12, 24) = {} (with heavy swing)", quantized);

    // Validation
    let valid = SwingConfig::new(0.5);
    assert!(valid.validate().is_ok());
    let invalid = SwingConfig::new(0.0);
    assert!(invalid.validate().is_err());
    println!("  Swing validation: 0.5 ✓, 0.0 ✗");

    println!();
}

// ─── Lesson 6: Tempo Maps ──────────────────────────────────────

fn lesson_6_tempo_maps() {
    println!("━━━ Lesson 6: Tempo Maps — Ramps and Curves ━━━\n");
    println!("Tempo maps support accelerando, ritardando, and interpolation curves.\n");

    // Constant tempo
    let constant = TempoMap::constant(120.0);
    println!("  Constant 120 BPM:");
    println!("    At tick 0: {}", constant.bpm_at(0));
    println!("    At tick 1000: {}", constant.bpm_at(1000));

    // Linear accelerando
    let mut linear = TempoMap::new();
    linear.set_default_curve(TempoCurve::Linear);
    linear.accelerando(0, 100.0, 1000, 200.0);
    println!("\n  Linear accelerando 100→200 BPM over 1000 ticks:");
    for tick in [0, 250, 500, 750, 1000, 1500] {
        println!("    tick {}: {:.1} BPM", tick, linear.bpm_at(tick));
    }

    // Ritardando
    let mut rit = TempoMap::new();
    rit.ritardando(0, 160.0, 800, 80.0);
    println!("\n  Ritardando 160→80 BPM:");
    for tick in [0, 200, 400, 600, 800] {
        println!("    tick {}: {:.1} BPM", tick, rit.bpm_at(tick));
    }

    // Exponential curve
    let mut exp = TempoMap::new();
    exp.set_default_curve(TempoCurve::Exponential);
    exp.add_point(TempoPoint::new(0, 100.0));
    exp.add_point(TempoPoint::new(100, 200.0));
    println!("\n  Exponential curve:");
    for tick in [0, 25, 50, 75, 100] {
        println!("    tick {}: {:.1} BPM", tick, exp.bpm_at(tick));
    }

    // Smooth (Hermite) curve
    let mut smooth = TempoMap::new();
    smooth.set_default_curve(TempoCurve::Smooth);
    smooth.add_point(TempoPoint::new(0, 100.0));
    smooth.add_point(TempoPoint::new(100, 200.0));
    println!("\n  Smooth (Hermite) curve:");
    for tick in [0, 25, 50, 75, 100] {
        println!("    tick {}: {:.1} BPM", tick, smooth.bpm_at(tick));
    }

    // Multi-segment ramp
    let mut multi = TempoMap::new();
    multi.add_point(TempoPoint::new(0, 120.0));
    multi.add_point(TempoPoint::new(500, 140.0));
    multi.add_point(TempoPoint::new(1000, 100.0));
    multi.add_point(TempoPoint::new(2000, 160.0));
    println!("\n  Multi-segment ramp (points auto-sorted):");
    for p in multi.points() {
        println!("    tick {}: {:.0} BPM", p.tick, p.bpm);
    }

    // Integrated with Pulse
    let mut pulse = Pulse::new(100.0, 24);
    pulse.tempo_map_mut().accelerando(0, 100.0, 100, 200.0);
    pulse.start().unwrap();
    println!("\n  Pulse with tempo ramp:");
    for step in [0, 25, 50, 75, 100] {
        if step > 0 {
            pulse.advance_n(step - (if step > 25 { step / 25 * 25 } else { 0 }));
        }
        println!("    tick {}: {:.1} BPM", pulse.tick(), pulse.bpm());
    }

    println!();
}

// ─── Lesson 7: Fermata ──────────────────────────────────────

fn lesson_7_fermata() {
    println!("━━━ Lesson 7: Fermata — Pause/Resume ━━━\n");
    println!("Fermata tracks pause durations, useful for musical pauses and system holds.\n");

    let mut fermata = Fermata::new();
    println!("  New fermata: active={}, has_paused={}", fermata.is_active(), fermata.has_paused());

    // Activate
    assert!(fermata.activate());
    println!("\n  Activated: active={}", fermata.is_active());

    // Double-activate fails
    assert!(!fermata.activate());
    println!("  Double-activate: false (already active)");

    // Measure pause duration
    std::thread::sleep(Duration::from_millis(10));
    let current = fermata.current_duration();
    println!("  Current duration: {:.2} ms (while active)", current.as_secs_f64() * 1000.0);

    // Deactivate
    let pause_dur = fermata.deactivate().unwrap();
    println!("\n  Deactivated: pause lasted {:.2} ms", pause_dur.as_secs_f64() * 1000.0);
    println!("  Last pause: {:.2} ms", fermata.last_pause_duration().as_secs_f64() * 1000.0);
    println!("  Has paused: {}", fermata.has_paused());

    // Multiple pauses accumulate
    fermata.activate();
    std::thread::sleep(Duration::from_millis(5));
    fermata.deactivate();
    fermata.activate();
    std::thread::sleep(Duration::from_millis(5));
    fermata.deactivate();
    println!("\n  After 3 pauses, total: {:.2} ms",
        fermata.total_pause_duration().as_secs_f64() * 1000.0);

    // Fermata through Pulse
    let mut pulse = Pulse::new(120.0, 24);
    pulse.start().unwrap();
    pulse.advance_n(5);
    println!("\n  Pulse: tick {}", pulse.tick());

    pulse.pause();
    println!("  Paused via fermata: active={}", pulse.fermata().is_active());
    assert!(pulse.advance().is_none());

    pulse.resume();
    println!("  Resumed: active={}", pulse.fermata().is_active());
    pulse.advance();
    println!("  Tick after resume: {}", pulse.tick());

    // Reset
    fermata.reset();
    println!("\n  Reset: active={}, has_paused={}, total=0",
        fermata.is_active(), fermata.has_paused());

    println!();
}

// ─── Lesson 8: Drift Correction ──────────────────────────────────────

fn lesson_8_drift_correction() {
    println!("━━━ Lesson 8: Drift Correction ━━━\n");
    println!("The internal clock tracks drift and corrects sleep durations.\n");

    // Basic clock
    let mut clock = Clock::new(Duration::from_millis(10));
    println!("  Clock interval: {} ms", clock.interval().as_millis());
    println!("  Initial drift: {} µs", clock.current_drift_us());

    // Start and tick
    clock.start();
    std::thread::sleep(Duration::from_millis(12)); // slightly late
    clock.tick();
    let drift = clock.current_drift_us();
    println!("\n  After sleeping 12ms (expected 10ms):");
    println!("  Drift: {} µs", drift);

    // Next tick adjusts for drift
    let corrected = clock.next_tick_duration();
    println!("  Corrected next tick: {} µs (was {} µs)",
        corrected.as_micros(), clock.interval().as_micros());
    println!("  Drift after correction: {} µs (reset)", clock.current_drift_us());

    // Drift stats
    let stats: DriftStats = clock.stats();
    println!("\n  Stats: corrections={}, avg_correction={} µs",
        stats.corrections, stats.avg_correction_us);

    // Through Pulse
    let mut pulse = Pulse::new(120.0, 24);
    pulse.start().unwrap();

    // Get drift stats from pulse
    let initial_stats = pulse.clock_stats();
    println!("\n  Pulse clock stats: corrections={}", initial_stats.corrections);

    // Advance some ticks
    pulse.advance_n(5);
    let next = pulse.next_tick_duration();
    println!("  Next tick duration: {} µs", next.as_micros());

    // Reset clock
    clock.reset();
    println!("\n  Clock reset: elapsed={:?}, drift={}",
        clock.elapsed(), clock.current_drift_us());

    println!();
}
