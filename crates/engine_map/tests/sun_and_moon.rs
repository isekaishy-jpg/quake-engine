//! A sun that moves, and a moon on the far side of the sky.

use engine_core::math::Vec3;
use engine_map::{RISING, Sky};

/// A sky with a white sun and a blue moon.
fn sky() -> Sky {
    Sky {
        sun_color: [1.0, 1.0, 1.0],
        moon_color: [0.2, 0.3, 0.5],
        ..Sky::default()
    }
}

/// A sun that's up lights from where it is, at its strength.
#[test]
fn a_sun_that_is_up_lights() {
    let (toward, color) = sky()
        .sun_or_moon(Vec3::new(0.0, 0.0, 2.0), 0.5)
        .expect("light");
    assert_eq!(toward, Vec3::new(0.0, 0.0, 1.0));
    assert_eq!(color, [0.5, 0.5, 0.5]);
}

/// Once it's set, it's the moon, from the far side of the sky.
#[test]
fn at_night_it_is_the_moon() {
    let (toward, color) = sky()
        .sun_or_moon(Vec3::new(0.0, 0.0, -1.0), 1.0)
        .expect("light");
    assert_eq!(toward, Vec3::new(0.0, 0.0, 1.0));
    assert_eq!(color, [0.2, 0.3, 0.5]);
}

/// With no moon, a night is dark.
#[test]
fn with_no_moon_there_is_none() {
    let dark = Sky {
        moon_color: [0.0; 3],
        ..sky()
    };
    assert!(dark.sun_or_moon(Vec3::new(0.0, 0.0, -1.0), 1.0).is_none());
}

/// Light comes in and goes out over the horizon, and at it there's none.
#[test]
fn light_fades_at_the_horizon() {
    let level = sky().sun_or_moon(Vec3::new(1.0, 0.0, 0.0), 1.0);
    assert!(level.is_none());
    let low = Vec3::new(1.0, 0.0, RISING * 0.5);
    let (_, color) = sky().sun_or_moon(low, 1.0).expect("light");
    assert!(color[0] > 0.3 && color[0] < 0.6, "{color:?}");
    let below = Vec3::new(1.0, 0.0, -RISING * 0.5);
    let (toward, color) = sky().sun_or_moon(below, 1.0).expect("light");
    assert!(toward.z > 0.0);
    assert!(color[2] > 0.1 && color[2] < 0.4, "{color:?}");
}
