//! Telling what can be given a hull from what can't.

use engine_core::math::Vec3;
use engine_physics::is_sliver;

/// What's nearly flat, in any direction, is a sliver, and a real solid isn't.
#[test]
fn slivers_are_told_from_solids() {
    let v = Vec3::new;
    // From a real map: half a thousandth of a unit thick.
    let sliver = [
        v(4544.0, 3710.6277, 1176.0),
        v(4544.0, 3710.6277, 1190.0),
        v(4544.0, 3710.6274, 1190.0),
        v(4544.0, 3710.6274, 1176.0),
        v(4544.0005, 3710.6277, 1190.0),
        v(4544.0005, 3710.6277, 1176.0),
    ];
    assert!(is_sliver(&sliver));
    // Thin and slanted, which no box round it would show.
    let slanted = [
        v(0.0, 0.0, 0.0),
        v(100.0, 100.0, 0.0),
        v(100.0, 100.0, 50.0),
        v(0.0, 0.0, 50.0),
        v(0.01, -0.01, 0.0),
    ];
    assert!(is_sliver(&slanted));
    let solid = [
        v(0.0, 0.0, 0.0),
        v(1.0, 0.0, 0.0),
        v(0.0, 1.0, 0.0),
        v(0.0, 0.0, 1.0),
    ];
    assert!(!is_sliver(&solid));
    assert!(is_sliver(&[]));
}
