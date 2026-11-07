//! How your own shot is drawn: at once, then easing towards what's on your screen.

use engine_client::{DEFAULT_SHOT_CORRECTION, own_shot_time};

/// The gap, in seconds, between your commands and what's on your screen, for a ping in
/// milliseconds with the usual 50 ms of interpolation.
fn gap_at(ping: f64) -> f64 {
    ping / 1000.0 + 0.05
}

/// How far behind your commands the shot is drawn at `age`, for a gap of `gap`.
fn behind(gap: f64, age: f64) -> f64 {
    100.0 - own_shot_time(100.0, gap, age, 1.0)
}

#[test]
fn a_shot_leaves_when_you_fire() {
    for ping in [0.0, 50.0, 150.0, 300.0] {
        let held = behind(gap_at(ping), 0.0);
        assert!(held <= 0.008 + 1e-9, "{ping} ms: held {held}");
    }
}

#[test]
fn on_a_good_connection_a_shot_ends_up_with_what_you_see() {
    // A hit looks right when the shot is on the same time as the enemy on your screen.
    let gap = gap_at(30.0);
    let settled = behind(gap, 1.0);
    assert!((settled - gap).abs() < 1e-9, "{settled} of {gap}");
}

/// A shot easing back is flying slower than it is, and past about a fifth that shows.
#[test]
fn a_shot_never_looks_much_slower_than_it_is() {
    for ping in [0.0, 30.0, 80.0, 150.0, 300.0, 500.0] {
        let gap = gap_at(ping);
        let mut age: f64 = 0.0;
        while age < 0.6 {
            let step = 0.01;
            let lost = behind(gap, age + step) - behind(gap, age);
            assert!(
                lost / step <= 0.2 + 1e-6,
                "{ping} ms at {age}: {}",
                lost / step
            );
            age += step;
        }
    }
}

#[test]
fn a_correction_of_nothing_leaves_a_shot_on_your_commands() {
    let time = own_shot_time(100.0, 0.2, 1.0, 0.0);
    assert!((100.0 - time - 0.008).abs() < 1e-9, "{time}");
}

#[test]
fn a_shot_is_never_ahead_of_your_commands() {
    assert!(own_shot_time(100.0, 0.0, 1.0, 1.0) <= 100.0);
}

/// The aim is half of the gap: closing all of it is too much to ask of a shot.
#[test]
fn by_default_a_shot_closes_half_of_the_gap() {
    assert!((DEFAULT_SHOT_CORRECTION - 0.5).abs() < 1e-9);
    let gap = gap_at(30.0);
    let settled = 100.0 - own_shot_time(100.0, gap, 1.0, DEFAULT_SHOT_CORRECTION);
    assert!((settled - gap * 0.5).abs() < 1e-9, "{settled} of {gap}");
}
