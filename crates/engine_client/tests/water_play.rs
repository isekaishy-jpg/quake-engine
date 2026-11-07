//! What going through water does to it.

use engine_client::WaterPlay;
use engine_core::math::Vec3;
use engine_map::{Contents, Map, Pool, PoolLook};

/// A pool from the origin to 500 each way, with its top at 0.
fn pool(look: impl Fn(&mut PoolLook)) -> Map {
    let mut own = PoolLook::of(Contents::Water).expect("a liquid");
    look(&mut own);
    Map {
        pools: vec![Pool {
            low: Vec3::new(0.0, 0.0, -200.0),
            high: Vec3::new(500.0, 500.0, 0.0),
            look: own,
        }],
        water_sounds: vec!["splash.wav".to_owned()],
        ..Map::default()
    }
}

/// A sixtieth of a second.
const STEP: f32 = 1.0 / 60.0;

/// Where it's all seen from.
const EYE: Vec3 = Vec3::new(100.0, 100.0, 50.0);

#[test]
fn going_in_splashes_once() {
    let map = pool(|look| {
        look.splash = 7;
        look.splash_sound = 1;
    });
    let mut play = WaterPlay::default();
    let high = Vec3::new(100.0, 100.0, 40.0);
    let events = play.follow(&map, &[(1, high)], &[], (EYE, STEP));
    assert!(events.effects.is_empty(), "it's only just been seen");
    let low = Vec3::new(100.0, 100.0, 10.0);
    let events = play.follow(&map, &[(1, low)], &[], (EYE, STEP));
    assert_eq!(events.effects, [(7, Vec3::new(100.0, 100.0, 0.0))]);
    assert_eq!(events.sounds, [(0, Vec3::new(100.0, 100.0, 0.0))]);
    assert!(events.pushes[0].strength > 2.0, "a big ring");
    let lower = Vec3::new(100.0, 100.0, 0.0);
    let events = play.follow(&map, &[(1, lower)], &[], (EYE, STEP));
    assert!(events.effects.is_empty(), "and only when it goes in");
}

#[test]
fn drifting_in_slowly_does_not_splash() {
    let map = pool(|look| look.splash = 7);
    let mut play = WaterPlay::default();
    let high = Vec3::new(100.0, 100.0, 24.5);
    let _first = play.follow(&map, &[(1, high)], &[], (EYE, STEP));
    let low = Vec3::new(100.0, 100.0, 23.9);
    let events = play.follow(&map, &[(1, low)], &[], (EYE, STEP));
    assert!(events.effects.is_empty());
    assert!(events.pushes.is_empty());
}

#[test]
fn going_through_leaves_a_wake_turned_the_way_it_goes() {
    let map = pool(|look| {
        look.trail = 3;
        look.trail_size = 50.0;
    });
    let mut play = WaterPlay::default();
    let from = Vec3::new(100.0, 100.0, -10.0);
    let _first = play.follow(&map, &[(1, from)], &[], (EYE, STEP));
    let to = Vec3::new(100.0, 105.0, -10.0);
    let events = play.follow(&map, &[(1, to)], &[], (EYE, STEP));
    let [push] = events.pushes[..] else {
        panic!("one push: {events:?}");
    };
    assert_eq!(push.place, Vec3::new(100.0, 105.0, 0.0), "on the surface");
    assert_eq!(push.reach, 50.0);
    assert_eq!(push.picture, 3);
    assert!(
        (push.turn - std::f32::consts::FRAC_PI_2).abs() < 0.01,
        "{}",
        push.turn
    );
    assert!(push.strength > 0.0 && push.strength <= 0.12);
}

#[test]
fn standing_still_leaves_nothing() {
    let map = pool(|_| {});
    let mut play = WaterPlay::default();
    let still = Vec3::new(100.0, 100.0, -10.0);
    let _first = play.follow(&map, &[(1, still)], &[], (EYE, STEP));
    let events = play.follow(&map, &[(1, still)], &[], (EYE, STEP));
    assert!(events.pushes.is_empty());
}

#[test]
fn going_fast_throws_up_spray_now_and_then() {
    let map = pool(|look| look.spray = 9);
    let mut play = WaterPlay::default();
    let mut sprays = 0;
    for step in 0..60_u16 {
        let place = Vec3::new(10.0 + f32::from(step) * 7.0, 100.0, -10.0);
        let events = play.follow(&map, &[(1, place)], &[], (EYE, STEP));
        sprays += events.effects.len();
    }
    assert!((5..=8).contains(&sprays), "{sprays}");
}

#[test]
fn what_is_far_over_or_under_a_surface_does_nothing_to_it() {
    let map = pool(|look| look.splash = 7);
    let mut play = WaterPlay::default();
    for (from, to) in [(100.0, 80.0), (-100.0, -120.0)] {
        let _first = play.follow(
            &map,
            &[(1, Vec3::new(100.0, 100.0, from))],
            &[],
            (EYE, STEP),
        );
        let events = play.follow(&map, &[(1, Vec3::new(120.0, 100.0, to))], &[], (EYE, STEP));
        assert!(events.pushes.is_empty(), "{events:?}");
    }
}

#[test]
fn a_shot_on_the_surface_leaves_a_ring() {
    let map = pool(|_| {});
    let mut play = WaterPlay::default();
    let hits = [Vec3::new(50.0, 50.0, 4.0), Vec3::new(50.0, 50.0, 100.0)];
    let events = play.follow(&map, &[], &hits, (EYE, STEP));
    let [push] = events.pushes[..] else {
        panic!("only the one on the water: {events:?}");
    };
    assert_eq!(push.place, Vec3::new(50.0, 50.0, 0.0));
    assert_eq!(push.picture, 0);
}

#[test]
fn rain_makes_rings_near_the_view_at_its_rate() {
    let map = pool(|look| look.rain_rings = 120.0);
    let mut play = WaterPlay::default();
    let mut rings = 0;
    for _ in 0..60 {
        let events = play.follow(&map, &[], &[], (EYE, STEP));
        for push in &events.pushes {
            assert!((push.place - Vec3::new(100.0, 100.0, 0.0)).length() < 700.0);
            assert_eq!(push.place.z, 0.0);
        }
        rings += events.pushes.len();
    }
    // Some of where it looks has no water.
    assert!((10..=120).contains(&rings), "{rings}");
}

#[test]
fn what_has_gone_is_forgotten() {
    let map = pool(|look| look.splash = 7);
    let mut play = WaterPlay::default();
    let high = Vec3::new(100.0, 100.0, 40.0);
    let _first = play.follow(&map, &[(1, high)], &[], (EYE, STEP));
    let _gone = play.follow(&map, &[], &[], (EYE, STEP));
    let low = Vec3::new(100.0, 100.0, 10.0);
    let events = play.follow(&map, &[(1, low)], &[], (EYE, STEP));
    assert!(
        events.effects.is_empty(),
        "it might have come from anywhere"
    );
}

#[test]
fn a_dry_level_is_left_alone() {
    let mut play = WaterPlay::default();
    let map = Map::default();
    let events = play.follow(&map, &[(1, Vec3::ZERO)], &[Vec3::ZERO], (EYE, STEP));
    assert!(events.pushes.is_empty());
}

/// A wake is as strong a second at any frame rate.
#[test]
fn a_wake_is_the_same_at_any_frame_rate() {
    let map = pool(|_| {});
    let strength_at = |rate: u16| {
        let mut play = WaterPlay::default();
        let seconds = 1.0 / f32::from(rate);
        let mut sum = 0.0;
        for step in 0..rate {
            let place = Vec3::new(10.0 + f32::from(step) * 200.0 * seconds, 100.0, -10.0);
            let events = play.follow(&map, &[(1, place)], &[], (EYE, seconds));
            sum += events.pushes.iter().map(|push| push.strength).sum::<f32>();
        }
        sum
    };
    let (slow, fast) = (strength_at(60), strength_at(240));
    assert!((slow - fast).abs() < slow * 0.1, "{slow} and {fast}");
}
