//! Cameras: what they do, with the world as a simple rule.

use engine_client::{CameraKind, CameraOrder, CameraSubject, Cameras};
use engine_core::math::Vec3;

/// A player at the middle of things, facing along +X, one frame at 100 a second.
fn subject(time: f32) -> CameraSubject {
    CameraSubject {
        eye: Vec3::new(0.0, 0.0, 50.0),
        yaw: 0.0,
        pitch: 0.0,
        roll: 1.5,
        time,
        seconds: 0.01,
    }
}

/// Lines reach as far as they're asked.
fn open(_start: Vec3, _way: Vec3, most: f32) -> f32 {
    most
}

/// `kind` as an order that happens at once.
fn order(kind: CameraKind) -> CameraOrder {
    CameraOrder { kind, blend: 0.0 }
}

#[test]
fn cameras_read_from_text() {
    assert_eq!(CameraKind::parse("first"), Some(CameraKind::FirstPerson));
    assert_eq!(
        CameraKind::parse("third 64 4 16"),
        Some(CameraKind::ThirdPerson {
            boom: 64.0,
            rise: 4.0,
            side: 16.0
        })
    );
    assert_eq!(
        CameraKind::parse("scene gate"),
        Some(CameraKind::Scene("gate".to_owned()))
    );
    assert!(CameraKind::parse("").is_none());
    assert!(CameraKind::parse("third lots").is_none());
    assert!(CameraKind::parse("fixed 1 2").is_none());
    assert!(CameraKind::parse("sideways").is_none());
}

#[test]
fn first_person_is_exactly_the_eye() {
    let mut cameras = Cameras::default();
    let shot = cameras.shot(&subject(0.0), &open);
    assert_eq!(shot.origin, subject(0.0).eye);
    assert_eq!(shot.roll, 1.5);
    assert!(shot.first_person);
}

/// Runs a boom for `frames`, with `room` as the world, and says how far back it ended up.
fn boom_after(frames: u32, cameras: &mut Cameras, room: &dyn Fn(Vec3, Vec3, f32) -> f32) -> f32 {
    let mut shot = cameras.shot(&subject(0.0), room);
    for _ in 0..frames {
        shot = cameras.shot(&subject(0.0), room);
    }
    subject(0.0).eye.x - shot.origin.x
}

#[test]
fn a_boom_comes_out_to_its_length_behind_the_player() {
    let mut cameras = Cameras::default();
    cameras.order(
        &order(CameraKind::parse("third 96 0 0").expect("reads")),
        None,
        0.0,
    );
    let back = boom_after(200, &mut cameras, &open);
    assert!((back - 86.0).abs() < 1.0, "{back}");
    assert!(!cameras.shot(&subject(0.0), &open).first_person);
}

#[test]
fn a_boom_comes_in_at_once_for_a_wall_and_eases_back_out() {
    let mut cameras = Cameras::default();
    cameras.order(
        &order(CameraKind::parse("third 96 0 0").expect("reads")),
        None,
        0.0,
    );
    let _out = boom_after(200, &mut cameras, &open);
    let wall = |_: Vec3, _: Vec3, _: f32| 50.0;
    let near = boom_after(0, &mut cameras, &wall);
    assert!(near <= 40.0 + 1e-3, "in one frame: {near}");
    let shortly = boom_after(0, &mut cameras, &open);
    assert!(shortly > near && shortly < 80.0, "{near} to {shortly}");
}

#[test]
fn with_no_room_the_camera_snaps_in_to_the_eyes() {
    let mut cameras = Cameras::default();
    cameras.order(
        &order(CameraKind::parse("third").expect("reads")),
        None,
        0.0,
    );
    let wall = |_: Vec3, _: Vec3, _: f32| 20.0;
    let shot = cameras.shot(&subject(0.0), &wall);
    assert!(shot.first_person);
    assert_eq!(shot.origin, subject(0.0).eye);
}

#[test]
fn a_fixed_camera_stays_and_sways_by_what_it_says() {
    let mut cameras = Cameras::default();
    cameras.order(
        &order(CameraKind::parse("fixed 10 20 30 90 0 0").expect("reads")),
        None,
        0.0,
    );
    let still = cameras.shot(&subject(1.0), &open);
    assert_eq!(still.origin, Vec3::new(10.0, 20.0, 30.0));
    assert_eq!(still.yaw, 90.0);
    cameras.order(
        &order(CameraKind::parse("fixed 10 20 30 90 0 1").expect("reads")),
        None,
        0.0,
    );
    let swaying = cameras.shot(&subject(1.0), &open);
    assert!(swaying.yaw != 90.0 && swaying.pitch != 0.0);
    assert!(!swaying.first_person);
}

#[test]
fn a_blend_goes_the_short_way_round_and_arrives() {
    let mut cameras = Cameras::default();
    cameras.order(
        &order(CameraKind::parse("fixed 0 0 0 170 0 0").expect("reads")),
        None,
        0.0,
    );
    let _start = cameras.shot(&subject(0.0), &open);
    let to = CameraOrder {
        kind: CameraKind::parse("fixed 100 0 0 -170 0 0").expect("reads"),
        blend: 0.1,
    };
    cameras.order(&to, None, 0.0);
    let mut last = 0.0;
    for _ in 0..4 {
        let shot = cameras.shot(&subject(0.0), &open);
        assert!(shot.origin.x >= last);
        last = shot.origin.x;
        assert!(
            shot.yaw.rem_euclid(360.0) >= 169.0 && shot.yaw.rem_euclid(360.0) <= 191.0,
            "{}",
            shot.yaw
        );
    }
    for _ in 0..20 {
        last = cameras.shot(&subject(0.0), &open).origin.x;
    }
    assert_eq!(last, 100.0);
}

#[test]
fn a_scene_that_isnt_there_stays_with_the_player() {
    let mut cameras = Cameras::default();
    cameras.order(
        &order(CameraKind::parse("scene nowhere").expect("reads")),
        None,
        0.0,
    );
    assert!(cameras.shot(&subject(0.0), &open).first_person);
}
