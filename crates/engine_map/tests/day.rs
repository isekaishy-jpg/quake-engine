//! A day: where the sun is at an hour, and sets of light keyed through it.

use engine_core::math::Vec3;
use engine_map::{DayCycle, LightKey, LightSet, Map, Sky, Zone, ZonePlane};

/// Whether two colours are near enough the same.
fn near(a: [f32; 3], b: [f32; 3]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < 0.001)
}

/// The sun comes up at six, is highest at noon, and goes down at six, under the ground at night.
#[test]
fn the_sun_goes_round_in_a_day() {
    let day = DayCycle::default();
    assert!(day.sun_toward(6.0).z.abs() < 0.001);
    assert!(day.sun_toward(18.0).z.abs() < 0.001);
    let noon = day.sun_toward(12.0);
    assert!(
        (noon.z - 60.0_f32.to_radians().sin()).abs() < 0.001,
        "{noon:?}"
    );
    assert!(day.sun_toward(0.0).z < -0.5);
    assert!(day.sun_toward(9.0).z > 0.0 && day.sun_toward(9.0).z < noon.z);
    assert!((day.sun_toward(9.0).length() - 1.0).abs() < 0.001);
}

/// It comes up on one side and goes down on the other, and the heading turns that.
#[test]
fn the_sun_rises_where_its_heading_says() {
    let day = DayCycle::default();
    assert!(day.sun_toward(6.0).x > 0.99);
    assert!(day.sun_toward(18.0).x < -0.99);
    let turned = DayCycle {
        heading: 90.0,
        ..DayCycle::default()
    };
    let rise = turned.sun_toward(6.0);
    assert!(rise.y > 0.99, "{rise:?}");
}

/// A day goes round in its length, and stands still with none.
#[test]
fn a_day_takes_its_length() {
    let day = DayCycle {
        length: 240.0,
        ..DayCycle::default()
    };
    assert!((day.hour_after(12.0, 60.0) - 18.0).abs() < 0.001);
    assert!((day.hour_after(20.0, 60.0) - 2.0).abs() < 0.001);
    let still = DayCycle::default();
    assert!((still.hour_after(12.0, 1000.0) - 12.0).abs() < 0.001);
}

/// Keys are blended between, going round midnight.
#[test]
fn a_set_blends_between_its_keys() {
    let set = LightSet {
        name: "test".to_owned(),
        keys: vec![
            LightKey {
                hour: 6.0,
                sun: Some([0.0, 0.0, 0.0]),
                ..LightKey::default()
            },
            LightKey {
                hour: 18.0,
                sun: Some([1.0, 1.0, 1.0]),
                ..LightKey::default()
            },
        ],
    };
    let sky = Sky::default();
    assert!(near(set.at(12.0, &sky).sun, [0.5; 3]));
    assert!(near(set.at(0.0, &sky).sun, [0.5; 3]), "round midnight");
    assert!(near(set.at(18.0, &sky).sun, [1.0; 3]));
    assert!(near(set.at(21.0, &sky).sun, [0.75; 3]));
}

/// What a key leaves out comes from the keys that have it, and what none has from the sky.
#[test]
fn a_set_fills_in_what_keys_leave_out() {
    let set = LightSet {
        name: "test".to_owned(),
        keys: vec![
            LightKey {
                hour: 0.0,
                fog: Some([0.0, 0.0, 0.0]),
                zenith: Some([1.0, 0.0, 0.0]),
                ..LightKey::default()
            },
            LightKey {
                hour: 12.0,
                fog: Some([1.0, 1.0, 1.0]),
                ..LightKey::default()
            },
        ],
    };
    let sky = Sky::default();
    let look = set.at(6.0, &sky);
    assert!(near(look.fog.expect("fog"), [0.5; 3]));
    assert!(near(look.zenith, [1.0, 0.0, 0.0]), "only one key has it");
    assert!(near(look.horizon, sky.horizon), "no key has it");
}

/// A sky's own day is dark at night, and the sky as it is at noon.
#[test]
fn a_sky_makes_a_day_of_its_own() {
    let sky = Sky::default();
    let set = LightSet::from_sky(&sky);
    let noon = set.at(12.0, &sky);
    assert!(near(noon.zenith, sky.zenith));
    assert!(near(noon.sun, sky.sun_color));
    let night = set.at(0.0, &sky);
    assert!(night.zenith.iter().sum::<f32>() < sky.zenith.iter().sum::<f32>() * 0.2);
    let dusk = set.at(18.0, &sky);
    assert!(dusk.sun[0] > dusk.sun[2] * 1.5, "warm: {:?}", dusk.sun);
}

/// A look is put on a sky, with its sun where it's said to be.
#[test]
fn a_look_goes_on_a_sky() {
    let sky = Sky {
        clouds: vec![engine_map::Clouds {
            color: [1.0, 0.8, 0.6],
            ..engine_map::Clouds::default()
        }],
        ..Sky::default()
    };
    let mut look = LightSet::from_sky(&sky).at(0.0, &sky);
    look.clouds = [0.5; 3];
    let toward = Vec3::new(0.0, 0.0, -1.0);
    let now = look.on(&sky, toward);
    assert_eq!(now.sun_direction, toward);
    assert!(near(now.zenith, look.zenith));
    assert!(
        near(now.clouds[0].color, [0.5, 0.4, 0.3]),
        "clouds take the day's colour"
    );
}

/// A box from `low` to `high`, as a zone's shape.
fn cube(low: f32, high: f32) -> Vec<ZonePlane> {
    let axes = [
        Vec3::new(1.0, 0.0, 0.0),
        Vec3::new(0.0, 1.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
    ];
    axes.iter()
        .flat_map(|axis| {
            [
                ZonePlane {
                    normal: *axis,
                    distance: high,
                },
                ZonePlane {
                    normal: *axis * -1.0,
                    distance: -low,
                },
            ]
        })
        .collect()
}

/// A set that's one sun colour all day.
fn all_day(name: &str, sun: [f32; 3]) -> LightSet {
    LightSet {
        name: name.to_owned(),
        keys: vec![LightKey {
            hour: 12.0,
            sun: Some(sun),
            ..LightKey::default()
        }],
    }
}

/// A level with a day, a set of its own, and a zone with another, from 0 to 1000.
fn level() -> Map {
    Map {
        sky: Some(Sky {
            sun_color: [1.0, 1.0, 1.0],
            ..Sky::default()
        }),
        day: Some(DayCycle::default()),
        light_sets: vec![
            all_day("world", [1.0, 0.0, 0.0]),
            all_day("cave", [0.0, 0.0, 1.0]),
        ],
        light_set: Some("world".to_owned()),
        zones: vec![Zone {
            name: "cave".to_owned(),
            shapes: vec![cube(0.0, 1000.0)],
            size: 1000.0,
            light_set: Some("cave".to_owned()),
            light_blend: 100.0,
            ..Zone::default()
        }],
        ..Map::default()
    }
}

/// How far into a zone a point is, from its nearest side.
#[test]
fn a_zone_knows_how_far_in() {
    let zone = &level().zones[0];
    assert!((zone.depth_of(Vec3::new(500.0, 500.0, 50.0)).expect("in") - 50.0).abs() < 0.001);
    assert!(zone.depth_of(Vec3::new(-10.0, 500.0, 500.0)).is_none());
}

/// A level's own set is its light, and a zone's blends in by how far in.
#[test]
fn a_zone_blends_its_set_in() {
    let map = level();
    let outside = map
        .day_at(12.0, Vec3::new(-50.0, 0.0, 0.0), None)
        .expect("a day");
    assert!(near(outside.look.sun, [1.0, 0.0, 0.0]));
    let edge = map
        .day_at(12.0, Vec3::new(500.0, 500.0, 50.0), None)
        .expect("a day");
    assert!(near(edge.look.sun, [0.5, 0.0, 0.5]));
    let deep = map
        .day_at(12.0, Vec3::new(500.0, 500.0, 500.0), None)
        .expect("a day");
    assert!(near(deep.look.sun, [0.0, 0.0, 1.0]));
}

/// With no set, a level's day is made from its sky.
#[test]
fn a_level_with_no_set_has_its_skys() {
    let map = Map {
        light_set: None,
        zones: Vec::new(),
        ..level()
    };
    let noon = map
        .day_at(12.0, Vec3::new(0.0, 0.0, 0.0), None)
        .expect("a day");
    assert!(near(noon.look.sun, [1.0, 1.0, 1.0]));
}

/// With no day, or no sky, there's nothing to say.
#[test]
fn no_day_says_nothing() {
    let map = Map {
        day: None,
        ..level()
    };
    assert!(map.day_at(12.0, Vec3::new(0.0, 0.0, 0.0), None).is_none());
    let map = Map {
        sky: None,
        ..level()
    };
    assert!(map.day_at(12.0, Vec3::new(0.0, 0.0, 0.0), None).is_none());
}

/// The sun's baked light goes down with it, and its colour's the day's over what it was baked.
#[test]
fn the_suns_baked_light_follows_the_day() {
    let map = level();
    let eye = Vec3::new(-50.0, 0.0, 0.0);
    let noon = map.day_at(12.0, eye, None).expect("a day");
    assert!(near(noon.sun_tint, [1.0, 0.0, 0.0]));
    let night = map.day_at(0.0, eye, None).expect("a day");
    assert!(near(night.sun_tint, [0.0; 3]));
}

/// The sun goes where something's put it, and the sky shows it there.
#[test]
fn a_sun_can_be_put_somewhere() {
    let map = level();
    let toward = Vec3::new(0.0, 1.0, 1.0);
    let day = map
        .day_at(12.0, Vec3::new(-50.0, 0.0, 0.0), Some(toward))
        .expect("a day");
    assert!((day.toward - toward.normalized()).length() < 0.001);
    assert!((day.sky.sun_direction - toward.normalized()).length() < 0.001);
}
