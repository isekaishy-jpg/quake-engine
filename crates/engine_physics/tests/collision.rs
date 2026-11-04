//! Solids and traces, on a small room.

mod common;

use common::room;
use engine_core::math::Vec3;
use engine_physics::{CollisionWorld, DIST_EPSILON, build_solids};

#[test]
fn a_room_has_six_solid_walls() {
    let map = room();
    let model = &map.models[0];
    let solids = build_solids(&map.tree, model.hulls[0], model.mins, model.maxs);
    assert_eq!(solids.len(), 6);
    assert!(solids.iter().all(|solid| solid.len() == 8), "each is a box");
}

#[test]
fn every_solid_loads() {
    assert_eq!(CollisionWorld::new(room()).solid_counts(), (6, 0));
}

#[test]
fn a_clear_trace_reaches_its_end() {
    let world = CollisionWorld::new(room());
    let end = Vec3::new(100.0, 50.0, 0.0);
    let trace = world.trace(Vec3::ZERO, end);
    assert!((trace.fraction - 1.0).abs() < f32::EPSILON);
    assert_eq!(trace.end, end);
    assert_eq!(trace.normal, Vec3::ZERO);
}

/// `pmovetst.c`: a trace stops `DIST_EPSILON` short of the plane.
#[test]
fn a_trace_stops_just_short_of_a_wall() {
    let world = CollisionWorld::new(room());
    let trace = world.trace(Vec3::ZERO, Vec3::new(512.0, 0.0, 0.0));
    assert!(
        (trace.end.x - (256.0 - DIST_EPSILON)).abs() < 1e-3,
        "{}",
        trace.end.x
    );
    assert_eq!(trace.normal, Vec3::new(-1.0, 0.0, 0.0));
    assert!(!trace.start_solid);
}

#[test]
fn a_slanted_trace_keeps_the_margin_measured_from_the_plane() {
    let world = CollisionWorld::new(room());
    let trace = world.trace(Vec3::ZERO, Vec3::new(512.0, 512.0, 0.0));
    assert!(
        (trace.end.x - (256.0 - DIST_EPSILON)).abs() < 1e-3,
        "{}",
        trace.end.x
    );
}

#[test]
fn the_stopping_point_is_never_in_solid() {
    let world = CollisionWorld::new(room());
    let trace = world.trace(Vec3::ZERO, Vec3::new(0.0, 0.0, -512.0));
    assert!(!world.in_solid(trace.end));
    let next = world.trace(trace.end, trace.end + Vec3::new(0.0, 0.0, -10.0));
    assert!(next.fraction < 0.01, "a second try stays put");
    assert!(!world.in_solid(next.end));
}

/// A ray ignores a hit at its very start, so this one sees nothing.
#[test]
fn a_trace_starting_exactly_on_a_wall_still_stops() {
    let world = CollisionWorld::new(room());
    let on_wall = Vec3::new(-256.0, 0.0, 0.0);
    assert!(!world.in_solid(on_wall));
    let trace = world.trace(on_wall, Vec3::new(-300.0, 0.0, 0.0));
    assert!(!world.in_solid(trace.end));
    assert!(trace.fraction < 0.01, "{}", trace.fraction);
    assert!(trace.normal.x > 0.0);
}

#[test]
fn starting_in_solid_is_reported() {
    let world = CollisionWorld::new(room());
    let inside = Vec3::new(300.0, 0.0, 0.0);
    let out = world.trace(inside, Vec3::ZERO);
    assert!(out.start_solid && !out.all_solid);
    let stuck = world.trace(inside, Vec3::new(310.0, 0.0, 0.0));
    assert!(stuck.start_solid && stuck.all_solid);
    assert_eq!(stuck.end, inside);
}

#[test]
fn a_zero_length_trace_is_clear_in_the_open() {
    let world = CollisionWorld::new(room());
    let trace = world.trace(Vec3::ZERO, Vec3::ZERO);
    assert!((trace.fraction - 1.0).abs() < f32::EPSILON);
}
