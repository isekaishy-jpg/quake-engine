//! The time of day, which the server keeps.

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{DayCycle, Map};
use engine_sim::Simulation;

/// A level with a day of `length` seconds, starting at `start`.
fn level(day: Option<DayCycle>) -> Arc<Map> {
    let mut map = (*engine_test_maps::room()).clone();
    map.day = day;
    Arc::new(map)
}

/// A level with no day has no time of day.
#[test]
fn no_day_has_no_time() {
    let sim = Simulation::new(level(None), Vec3::ZERO);
    assert!(sim.time_of_day().is_none());
}

/// A level starts at its hour, and a script or the console can set another.
#[test]
fn a_day_starts_at_its_hour_and_can_be_set() {
    let day = DayCycle {
        start: 9.0,
        ..DayCycle::default()
    };
    let mut sim = Simulation::new(level(Some(day)), Vec3::ZERO);
    assert!((sim.time_of_day().expect("a day") - 9.0).abs() < 0.001);
    assert!(sim.day_set().is_none());
    sim.set_time_of_day(27.0);
    assert!(
        (sim.time_of_day().expect("a day") - 3.0).abs() < 0.001,
        "round the day"
    );
    assert_eq!(sim.day_set().map(|(hour, _)| hour), Some(3.0));
}

/// A saved game keeps what the day was set to.
#[test]
fn a_saved_game_keeps_the_day() {
    let day = DayCycle::default();
    let mut sim = Simulation::new(level(Some(day)), Vec3::ZERO);
    sim.set_time_of_day(20.0);
    let restored = Simulation::restored(level(Some(day)), sim.save());
    assert_eq!(restored.day_set(), sim.day_set());
}
