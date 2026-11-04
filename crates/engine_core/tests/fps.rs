//! The FPS meter (proposal 0021).

use std::time::Duration;

use engine_core::time::FpsMeter;

/// Runs `frames` frames of `delta`, returning every reading.
fn run(meter: &mut FpsMeter, frames: u32, delta: Duration) -> Vec<u32> {
    (0..frames).filter_map(|_| meter.frame(delta)).collect()
}

#[test]
fn a_steady_rate_reads_exactly() {
    let mut meter = FpsMeter::new();
    let readings = run(&mut meter, 1000, Duration::from_millis(10));
    assert!(!readings.is_empty());
    assert!(
        readings.iter().all(|&reading| reading == 100),
        "{readings:?}"
    );
}

#[test]
fn readings_never_come_faster_than_four_a_second() {
    let mut meter = FpsMeter::new();
    // Ten seconds at 1000 frames a second.
    let readings = run(&mut meter, 10_000, Duration::from_millis(1));
    assert_eq!(readings.len(), 40);
}

#[test]
fn nothing_is_read_before_a_quarter_second() {
    let mut meter = FpsMeter::new();
    assert_eq!(run(&mut meter, 24, Duration::from_millis(10)), []);
    assert_eq!(meter.frame(Duration::from_millis(10)), Some(100));
}

#[test]
fn an_uneven_rate_reads_steadier_than_it_runs() {
    let mut meter = FpsMeter::new();
    let mut readings = Vec::new();
    // Each quarter second alternates between 200 and 100 frames a second.
    for _ in 0..40 {
        readings.extend(run(&mut meter, 50, Duration::from_millis(5)));
        readings.extend(run(&mut meter, 25, Duration::from_millis(10)));
    }
    let settled = &readings[20..];
    let low = settled.iter().min().expect("readings");
    let high = settled.iter().max().expect("readings");
    assert!(
        high - low < 25,
        "swing of {} on a raw swing of 100",
        high - low
    );
    assert!((130..170).contains(low), "{low}");
}

#[test]
fn a_real_change_comes_through_within_seconds() {
    let mut meter = FpsMeter::new();
    let _before = run(&mut meter, 400, Duration::from_millis(5));
    let after = run(&mut meter, 500, Duration::from_millis(20));
    let last = *after.last().expect("readings");
    assert!((50..=55).contains(&last), "{last}");
}

#[test]
fn a_reset_forgets_the_old_rate() {
    let mut meter = FpsMeter::new();
    let _before = run(&mut meter, 400, Duration::from_millis(5));
    meter.reset();
    let after = run(&mut meter, 100, Duration::from_millis(20));
    assert_eq!(after[0], 50);
}
