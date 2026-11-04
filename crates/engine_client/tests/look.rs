//! Mouse look. Proposal 0015 sets out what deterministic means here.

use engine_client::{Look, LookScale};

/// Quake's default settings.
fn scale() -> LookScale {
    LookScale::from_settings(3.0, 0.022, 0.022)
}

/// A fixed, uneven sequence of raw motion.
fn motion() -> Vec<(f64, f64)> {
    (0_i32..5000)
        .map(|index| {
            let x = (index * 37) % 11 - 5;
            let y = (index * 53) % 7 - 3;
            (f64::from(x), f64::from(y))
        })
        .collect()
}

#[test]
fn reading_the_view_between_events_changes_nothing() {
    let mut read_rarely = Look::new(scale());
    let mut read_always = Look::new(scale());
    for (x, y) in motion() {
        read_rarely.motion(x, y);
        read_always.motion(x, y);
        let _frame = (read_always.yaw(), read_always.pitch());
    }
    assert_eq!(read_rarely, read_always);
}

#[test]
fn the_order_of_turns_does_not_change_yaw() {
    let mut forward = Look::new(scale());
    let mut backward = Look::new(scale());
    for (x, _) in motion() {
        forward.motion(x, 0.0);
    }
    for (x, _) in motion().into_iter().rev() {
        backward.motion(x, 0.0);
    }
    assert_eq!(forward.yaw().to_bits(), backward.yaw().to_bits());
}

#[test]
fn moving_out_and_back_returns_exactly() {
    let mut look = Look::new(LookScale::from_settings(2.37, 0.022, 0.022));
    for _ in 0..100_000 {
        look.motion(3.0, 1.0);
        look.motion(3.0, -1.0);
    }
    for _ in 0..200_000 {
        look.motion(-3.0, 0.0);
    }
    assert_eq!(look.yaw().to_bits(), 0.0_f64.to_bits());
    assert_eq!(look.pitch().to_bits(), 0.0_f64.to_bits());
}

#[test]
fn moving_right_turns_right_and_moving_down_looks_down() {
    let mut look = Look::new(scale());
    look.motion(100.0, 100.0);
    assert!((look.yaw() - (360.0 - 6.6)).abs() < 1e-9);
    assert!((look.pitch() - 6.6).abs() < 1e-9);
}

#[test]
fn pitch_stops_at_quakes_limits() {
    let mut look = Look::new(scale());
    look.motion(0.0, 1_000_000.0);
    assert!((look.pitch() - 80.0).abs() < 1e-9);
    look.motion(0.0, -1_000_000.0);
    assert!((look.pitch() + 70.0).abs() < 1e-9);
}

/// The original clamps once a frame, so this flick would end somewhere else
/// if the two events shared a frame.
#[test]
fn pitch_is_clamped_per_event() {
    let mut look = Look::new(scale());
    look.motion(0.0, 5000.0);
    look.motion(0.0, -100.0);
    assert!((look.pitch() - (80.0 - 6.6)).abs() < 1e-9);
}

#[test]
fn changing_the_scale_does_not_move_the_view() {
    let mut look = Look::new(scale());
    look.motion(123.0, 45.0);
    let (yaw, pitch) = (look.yaw(), look.pitch());
    look.set_scale(LookScale::from_settings(1.5, 0.022, 0.022));
    assert!((look.yaw() - yaw).abs() < 1e-9);
    assert!((look.pitch() - pitch).abs() < 1e-9);
    look.motion(0.0, 100.0);
    assert!((look.pitch() - pitch - 3.3).abs() < 1e-9);
}

#[test]
fn a_negative_pitch_scale_inverts_and_still_stops_at_the_limits() {
    let mut look = Look::new(LookScale::from_settings(3.0, 0.022, -0.022));
    look.motion(0.0, 100.0);
    assert!((look.pitch() + 6.6).abs() < 1e-9);
    look.motion(0.0, 1_000_000.0);
    assert!((look.pitch() + 70.0).abs() < 1e-9);
}

#[test]
fn a_zero_scale_stays_still() {
    let mut look = Look::new(LookScale::from_settings(0.0, 0.022, 0.022));
    look.motion(100.0, 100.0);
    assert_eq!(look.yaw().to_bits(), 0.0_f64.to_bits());
    assert_eq!(look.pitch().to_bits(), 0.0_f64.to_bits());
}
