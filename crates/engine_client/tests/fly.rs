//! The fly camera.

use engine_client::{Buttons, FlyCamera};
use engine_core::math::Vec3;

/// One tick at 72 Hz.
const TICK: f32 = 1.0 / 72.0;

/// Buttons with one held.
fn holding(name: &str) -> Buttons {
    let mut buttons = Buttons::default();
    assert!(buttons.set(name, true));
    buttons
}

#[test]
fn forward_follows_the_view() {
    let mut camera = FlyCamera::new(Vec3::ZERO);
    camera.tick(holding("forward"), 0.0, 0.0, 1.0);
    assert!((camera.position().x - 400.0).abs() < 1e-3);
    let mut camera = FlyCamera::new(Vec3::ZERO);
    camera.tick(holding("forward"), 0.0, 90.0, 1.0);
    assert!(
        (camera.position().z + 400.0).abs() < 1e-3,
        "positive pitch looks down"
    );
}

#[test]
fn right_is_right_of_the_view() {
    let mut camera = FlyCamera::new(Vec3::ZERO);
    camera.tick(holding("moveright"), 0.0, 0.0, 1.0);
    assert!((camera.position().y + 400.0).abs() < 1e-3);
}

#[test]
fn up_ignores_the_view() {
    let mut camera = FlyCamera::new(Vec3::ZERO);
    camera.tick(holding("moveup"), 45.0, 45.0, 1.0);
    assert!((camera.position().z - 400.0).abs() < 1e-3);
}

#[test]
fn diagonals_are_no_faster() {
    let mut buttons = holding("forward");
    assert!(buttons.set("moveright", true));
    let mut camera = FlyCamera::new(Vec3::ZERO);
    camera.tick(buttons, 0.0, 0.0, 1.0);
    assert!((camera.position().length() - 400.0).abs() < 1e-3);
}

#[test]
fn speed_multiplies_and_nothing_held_stays_put() {
    let mut buttons = holding("forward");
    assert!(buttons.set("speed", true));
    let mut camera = FlyCamera::new(Vec3::ZERO);
    camera.tick(buttons, 0.0, 0.0, 1.0);
    assert!((camera.position().x - 1000.0).abs() < 1e-3);
    let start = camera.position();
    camera.tick(Buttons::default(), 0.0, 0.0, 1.0);
    assert_eq!(camera.position(), start);
}

#[test]
fn drawing_interpolates_within_the_last_tick() {
    let mut camera = FlyCamera::new(Vec3::ZERO);
    camera.tick(holding("forward"), 0.0, 0.0, TICK);
    let full = camera.position().x;
    assert_eq!(camera.view_position(0.0), Vec3::ZERO);
    assert!((camera.view_position(0.5).x - full * 0.5).abs() < 1e-4);
    assert_eq!(camera.view_position(1.0), camera.position());
}

#[test]
fn unknown_buttons_are_reported() {
    assert!(!Buttons::default().set("attack", true));
}
