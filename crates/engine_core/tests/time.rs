//! The fixed tick clock.

use std::time::Duration;

use engine_core::time::{Tick, TickClock, TickRate};

/// A 72 Hz clock with a quarter-second catch-up limit.
fn clock() -> TickClock {
    let rate = TickRate::new(72).expect("non-zero rate");
    TickClock::new(rate, Duration::from_millis(250))
}

#[test]
fn zero_rate_is_refused() {
    assert_eq!(TickRate::new(0), None);
}

#[test]
fn one_second_gives_exactly_the_rate() {
    let mut clock = clock();
    let mut total = 0;
    for _ in 0..1000 {
        total += clock.advance(Duration::from_millis(1));
    }
    assert_eq!(total, 72);
}

#[test]
fn a_long_run_does_not_drift() {
    let mut clock = clock();
    let mut total = 0;
    // One hour in uneven 7 ms steps.
    for _ in 0..(3_600_000 / 7) {
        total += clock.advance(Duration::from_millis(7));
    }
    let elapsed_ms = (3_600_000 / 7) * 7;
    assert_eq!(total, elapsed_ms * 72 / 1000);
}

#[test]
fn a_stall_is_limited_and_time_is_dropped() {
    let mut clock = clock();
    let due = clock.advance(Duration::from_secs(10));
    assert_eq!(due, 18, "a quarter second at 72 Hz");
    assert_eq!(clock.tick(), Tick(18));
    assert_eq!(clock.advance(Duration::from_millis(1)), 0);
}

/// Frames arrive between ticks, when the clock hasn't advanced.
#[test]
fn a_frame_between_ticks_sees_the_time_since_the_clock_advanced() {
    let mut clock = clock();
    assert_eq!(clock.advance(Duration::from_millis(14)), 1);
    let start = clock.fraction();
    let later = clock.fraction_after(Duration::from_millis(7));
    assert!((later - start - 0.504).abs() < 0.001, "{later}");
    assert!((clock.fraction_after(Duration::ZERO) - start).abs() < 1e-12);
}

#[test]
fn the_fraction_stops_at_one_if_a_tick_runs_late() {
    let clock = clock();
    assert!((clock.fraction_after(Duration::from_millis(100)) - 1.0).abs() < 1e-12);
}

#[test]
fn fraction_and_time_to_next_tick_follow_the_clock() {
    let mut clock = clock();
    assert_eq!(clock.advance(Duration::from_millis(7)), 0);
    let fraction = clock.fraction();
    assert!((fraction - 0.504).abs() < 0.001, "fraction {fraction}");
    let left = clock.until_next();
    assert!(left > Duration::from_micros(6800) && left < Duration::from_micros(7000));
    assert_eq!(clock.advance(left), 1);
    assert!(clock.fraction() < 0.001);
}
