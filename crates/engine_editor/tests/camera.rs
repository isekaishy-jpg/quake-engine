//! The editor's camera: flying, and turning between the world and the screen.

use engine_core::math::Vec3;
use engine_editor::camera::{Camera, Keys};

/// A camera at the origin looking along x.
fn camera() -> Camera {
    Camera {
        origin: Vec3::ZERO,
        yaw: 0.0,
        pitch: 0.0,
    }
}

/// A point straight ahead shows in the middle.
#[test]
fn ahead_is_the_middle_of_the_view() {
    let at = camera().project(Vec3::new(100.0, 0.0, 0.0), (800.0, 600.0));
    let (x, y) = at.expect("in front");
    assert!(
        (x - 400.0).abs() < 0.01 && (y - 300.0).abs() < 0.01,
        "{x} {y}"
    );
}

/// What's behind has nowhere to show.
#[test]
fn behind_shows_nowhere() {
    assert_eq!(
        camera().project(Vec3::new(-100.0, 0.0, 0.0), (800.0, 600.0)),
        None
    );
}

/// Left and up are as the screen has them.
#[test]
fn the_screen_is_the_right_way_round() {
    let size = (800.0, 600.0);
    let (x, _) = camera()
        .project(Vec3::new(100.0, 50.0, 0.0), size)
        .expect("in front");
    assert!(x < 400.0, "left of the view is the left of the screen");
    let (_, y) = camera()
        .project(Vec3::new(100.0, 0.0, 50.0), size)
        .expect("in front");
    assert!(y < 300.0, "up is the top of the screen");
}

/// A pixel's ray goes through what shows at that pixel, in any window shape,
/// and after turning.
#[test]
fn a_ray_goes_back_through_its_pixel() {
    let view = Camera {
        origin: Vec3::new(10.0, -20.0, 30.0),
        yaw: 35.0,
        pitch: 20.0,
    };
    for size in [(800.0, 600.0), (1200.0, 500.0), (500.0, 900.0)] {
        for pixel in [
            (100.0, 80.0),
            (size.0 * 0.5, size.1 * 0.5),
            (size.0 - 9.0, size.1 - 40.0),
        ] {
            let point = view.origin + view.ray(pixel, size) * 300.0;
            let (x, y) = view.project(point, size).expect("in front");
            assert!(
                (x - pixel.0).abs() < 0.05 && (y - pixel.1).abs() < 0.05,
                "{x} {y}"
            );
        }
    }
}

/// Flying goes the way the camera looks, faster with shift.
#[test]
fn flying_follows_the_view() {
    let mut view = camera();
    let forward = Keys {
        forward: true,
        ..Keys::default()
    };
    view.fly(forward, 1.0);
    assert!(
        view.origin.x > 100.0 && view.origin.y.abs() < 0.01,
        "{:?}",
        view.origin
    );
    let slow = view.origin.x;
    let mut quick = camera();
    quick.fly(
        Keys {
            fast: true,
            ..forward
        },
        1.0,
    );
    assert!(quick.origin.x > slow * 2.0);
}

/// Looking can't go past straight up or down.
#[test]
fn looking_stops_at_straight_up() {
    let mut view = camera();
    view.look(0.0, 100_000.0);
    assert!((view.pitch - 89.0).abs() < 0.001);
    view.look(0.0, -100_000.0);
    assert!((view.pitch + 89.0).abs() < 0.001);
}

/// A step goes along the floor even when the camera looks down, and a turn alone stays put.
#[test]
fn a_step_stays_on_the_floor() {
    let mut camera = camera();
    camera.pitch = 40.0;
    camera.step(100.0, 0.0);
    assert!(
        (camera.origin.x - 100.0).abs() < 0.01,
        "{:?}",
        camera.origin
    );
    assert!(camera.origin.z.abs() < 0.01, "{:?}", camera.origin);
    camera.step(0.0, 90.0);
    assert!((camera.yaw - 90.0).abs() < 0.01);
    assert!(
        (camera.origin.x - 100.0).abs() < 0.01,
        "{:?}",
        camera.origin
    );
}
