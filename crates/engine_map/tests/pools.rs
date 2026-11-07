//! What the eye is in.

use engine_core::math::Vec3;
use engine_map::{Contents, Pool, PoolLook, look_around};

/// A pool from the origin to 100 each way, with `tint`.
fn pool(tint: [f32; 4]) -> Pool {
    let mut look = PoolLook::of(Contents::Water).expect("water has a look");
    look.tint = tint;
    Pool {
        low: Vec3::new(0.0, 0.0, 0.0),
        high: Vec3::new(100.0, 100.0, 100.0),
        look,
    }
}

#[test]
fn air_has_no_look() {
    assert!(look_around(&[], Vec3::new(0.0, 0.0, 0.0), Contents::Empty).is_none());
}

#[test]
fn the_pool_the_eye_is_in_says_its_tint() {
    let pools = [pool([1.0, 0.0, 0.0, 0.5])];
    let eye = Vec3::new(50.0, 50.0, 50.0);
    let look = look_around(&pools, eye, Contents::Water).expect("a look");
    assert_eq!(look.tint, [1.0, 0.0, 0.0, 0.5]);
}

/// Original levels have no pools, and still colour the view as they did.
#[test]
fn water_outside_any_pool_looks_as_water_usually_does() {
    let pools = [pool([1.0, 0.0, 0.0, 0.5])];
    let eye = Vec3::new(500.0, 0.0, 0.0);
    let look = look_around(&pools, eye, Contents::Water).expect("a look");
    assert_eq!(Some(look), PoolLook::of(Contents::Water));
}
