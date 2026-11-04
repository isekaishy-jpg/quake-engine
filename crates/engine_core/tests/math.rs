//! Vector maths.

use engine_core::math::Vec3;

#[test]
fn sums_and_products() {
    let a = Vec3::new(1.0, 2.0, 3.0);
    let b = Vec3::new(4.0, 5.0, 6.0);
    assert_eq!(a + b, Vec3::new(5.0, 7.0, 9.0));
    assert_eq!(b - a, Vec3::new(3.0, 3.0, 3.0));
    assert_eq!(a * 2.0, Vec3::new(2.0, 4.0, 6.0));
    assert!((a.dot(b) - 32.0).abs() < f32::EPSILON);
}

#[test]
fn lengths_and_directions() {
    assert!((Vec3::new(3.0, 4.0, 0.0).length() - 5.0).abs() < f32::EPSILON);
    assert_eq!(
        Vec3::new(0.0, 0.0, 9.0).normalized(),
        Vec3::new(0.0, 0.0, 1.0)
    );
    assert_eq!(Vec3::ZERO.normalized(), Vec3::ZERO);
}

#[test]
fn lerp_runs_between_the_ends() {
    let a = Vec3::new(0.0, 0.0, 0.0);
    let b = Vec3::new(2.0, 4.0, 8.0);
    assert_eq!(a.lerp(b, 0.5), Vec3::new(1.0, 2.0, 4.0));
}
