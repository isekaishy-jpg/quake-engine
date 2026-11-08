//! Following someone from zone to zone.

use engine_client::ZoneTracker;
use engine_core::math::Vec3;
use engine_map::{Map, Zone, ZonePlane};

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

/// A town, with a market inside it.
fn town() -> Map {
    Map {
        zones: vec![
            Zone {
                name: "town".to_owned(),
                title: Some("The Town".to_owned()),
                shapes: vec![cube(0.0, 1000.0)],
                size: 1000.0,
                music: Some("4".to_owned()),
                sound: Some("wind.wav".to_owned()),
                ..Zone::default()
            },
            Zone {
                name: "market".to_owned(),
                title: Some("The Market".to_owned()),
                parent: Some("town".to_owned()),
                shapes: vec![cube(400.0, 600.0)],
                size: 200.0,
                light: Some([0.5, 0.5, 0.5]),
                ..Zone::default()
            },
        ],
        ..Map::default()
    }
}

const OUTSIDE: Vec3 = Vec3::new(-50.0, -50.0, -50.0);
const IN_TOWN: Vec3 = Vec3::new(100.0, 100.0, 100.0);
const IN_MARKET: Vec3 = Vec3::new(500.0, 500.0, 500.0);

#[test]
fn the_inner_zone_wins() {
    let map = town();
    assert_eq!(map.zone_at(OUTSIDE), None);
    assert_eq!(map.zone_at(IN_TOWN), Some(0));
    assert_eq!(map.zone_at(IN_MARKET), Some(1));
}

#[test]
fn priority_wins_over_size() {
    let mut map = town();
    map.zones[0].priority = 1;
    assert_eq!(map.zone_at(IN_MARKET), Some(0));
}

#[test]
fn a_sub_zone_takes_what_it_doesnt_set_from_its_parent() {
    let map = town();
    let settings = map.zone_settings(1);
    assert_eq!(settings.music.as_deref(), Some("4"));
    assert_eq!(settings.sound.as_deref(), Some("wind.wav"));
    assert_eq!(settings.light, Some([0.5; 3]));
}

#[test]
fn a_circle_of_parents_ends() {
    let mut map = town();
    map.zones[0].parent = Some("market".to_owned());
    assert_eq!(map.zone_line(1).len(), 2);
}

#[test]
fn going_in_shows_a_title_and_starts_what_it_says() {
    let map = town();
    let mut zones = ZoneTracker::default();
    let first = zones.update(&map, OUTSIDE, 0.1).expect("a start");
    assert_eq!(first.title, None);
    let change = zones.update(&map, IN_TOWN, 0.1).expect("a change");
    assert_eq!(change.title.as_deref(), Some("The Town"));
    assert_eq!(change.music, Some(Some("4".to_owned())));
    assert_eq!(change.sound, Some(Some("wind.wav".to_owned())));
    assert_eq!(zones.update(&map, IN_TOWN, 0.1), None, "nothing's changed");
}

#[test]
fn a_sub_zone_shows_its_title_and_coming_out_shows_nothing() {
    let map = town();
    let mut zones = ZoneTracker::default();
    let _start = zones.update(&map, IN_TOWN, 0.1);
    let going_in = zones.update(&map, IN_MARKET, 0.1).expect("a change");
    assert_eq!(going_in.title.as_deref(), Some("The Market"));
    assert_eq!(going_in.music, None, "the same music goes on");
    let coming_out = zones.update(&map, IN_TOWN, 0.1).expect("a change");
    assert_eq!(coming_out.title, None);
}

#[test]
fn light_settles_over_time_and_goes_back() {
    let map = town();
    let mut zones = ZoneTracker::default();
    let _start = zones.update(&map, IN_TOWN, 0.1);
    assert_eq!(zones.light(), [1.0; 3]);
    let _in = zones.update(&map, IN_MARKET, 0.1);
    assert!(zones.light()[0] > 0.9, "it's only begun");
    for _ in 0..200 {
        let _on = zones.update(&map, IN_MARKET, 0.1);
    }
    assert!((zones.light()[0] - 0.5).abs() < 0.01);
    for _ in 0..200 {
        let _on = zones.update(&map, OUTSIDE, 0.1);
    }
    assert!((zones.light()[0] - 1.0).abs() < 0.01);
}

#[test]
fn leaving_for_nowhere_leaves_the_music_and_stops_the_sound() {
    let map = town();
    let mut zones = ZoneTracker::default();
    let _start = zones.update(&map, IN_TOWN, 0.1);
    let change = zones.update(&map, OUTSIDE, 0.1).expect("a change");
    assert_eq!(change.music, None);
    assert_eq!(change.sound, Some(None));
}

/// How far exposure goes by itself is a zone's where it says, or what's round it says, and the
/// level's elsewhere.
#[test]
fn exposure_goes_as_far_as_the_zone_says() {
    let mut map = town();
    map.zones[0].exposure_range = Some([0.0, 2.0]);
    map.zones[1].exposure_range = Some([-1.0, 0.5]);
    let mut zones = ZoneTracker::default();
    let _start = zones.update(&map, OUTSIDE, 0.1);
    assert_eq!(zones.exposure_range(), None);
    let _in = zones.update(&map, IN_TOWN, 0.1);
    assert_eq!(zones.exposure_range(), Some([0.0, 2.0]));
    let _market = zones.update(&map, IN_MARKET, 0.1);
    assert_eq!(zones.exposure_range(), Some([-1.0, 0.5]));
    map.zones[1].exposure_range = None;
    let _again = zones.update(&map, IN_MARKET, 0.1);
    assert_eq!(
        zones.exposure_range(),
        Some([0.0, 2.0]),
        "from the town round it"
    );
}
