//! Models that move in steps, shown going evenly.

use engine_client::{DrawnEntity, ModelGlide};
use engine_core::entity::EntityState;
use engine_core::math::Vec3;

/// A frame at 200 a second.
const FRAME: f32 = 0.005;

/// Something at `x` along, facing `yaw`, showing `frame`.
fn at(id: u64, x: f32, yaw: f32, frame: u16) -> DrawnEntity {
    DrawnEntity {
        state: EntityState {
            id,
            frame,
            origin: Vec3::new(x, 0.0, 0.0),
            angles: Vec3::new(0.0, yaw, 0.0),
            ..EntityState::default()
        },
        from_frame: frame,
        blend: 1.0,
    }
}

/// Where something at `x` shows, `seconds` on.
fn shown(glide: &mut ModelGlide, x: f32, seconds: f32) -> f32 {
    let mut drawn = [at(1, x, 0.0, 0)];
    glide.smooth(&mut drawn, seconds);
    drawn[0].state.origin.x
}

/// A monster's walk: 10 units every tenth of a second, seen at 200 frames a second.
fn walked(glide: &mut ModelGlide, steps: u32) -> Vec<f32> {
    let mut shown_at = Vec::new();
    for frame in 0..steps * 20 {
        #[expect(clippy::cast_precision_loss, reason = "a few steps")]
        let x = (frame / 20) as f32 * 10.0;
        shown_at.push(shown(glide, x, FRAME));
    }
    shown_at
}

/// Something new is where it is, with nothing to catch up with.
#[test]
fn what_is_new_shows_where_it_is() {
    let mut glide = ModelGlide::default();
    assert!((shown(&mut glide, 50.0, FRAME) - 50.0).abs() < 0.001);
}

/// A step's shown going there, and not as a hop.
#[test]
fn a_step_is_shown_as_a_walk() {
    let mut glide = ModelGlide::default();
    for _ in 0..30 {
        shown(&mut glide, 0.0, FRAME);
    }
    let first = shown(&mut glide, 10.0, FRAME);
    assert!(first < 1.0, "{first}");
    for _ in 0..9 {
        shown(&mut glide, 10.0, FRAME);
    }
    let middle = shown(&mut glide, 10.0, FRAME);
    assert!((middle - 5.0).abs() < 1.0, "{middle}");
    for _ in 0..30 {
        shown(&mut glide, 10.0, FRAME);
    }
    assert!((shown(&mut glide, 10.0, FRAME) - 10.0).abs() < 0.001);
}

/// Steps one after another are one even walk, moving on every frame.
#[test]
fn steps_make_an_even_walk() {
    let mut glide = ModelGlide::default();
    let places = walked(&mut glide, 10);
    let moves: Vec<f32> = places[25..]
        .windows(2)
        .map(|pair| pair[1] - pair[0])
        .collect();
    let most = moves.iter().copied().fold(0.0, f32::max);
    let least = moves.iter().copied().fold(f32::MAX, f32::min);
    assert!(most < 1.0, "no hops: {most}");
    assert!(least > 0.0, "never stops: {least}");
}

/// What moves every tick is caught up with once it's going, and doesn't lag from then on.
#[test]
fn what_keeps_moving_is_left_alone() {
    let mut glide = ModelGlide::default();
    let mut x = 0.0;
    for frame in 0..60 {
        x += 1.0;
        let place = shown(&mut glide, x, FRAME);
        if frame > 20 {
            assert!((place - x).abs() < 0.001, "{place} {x}");
        }
    }
}

/// Long frames aren't taken for standing still, so what's moving on each isn't left behind.
#[test]
fn a_slow_machine_does_not_lag() {
    let mut glide = ModelGlide::default();
    let mut x = 0.0;
    for frame in 0..50 {
        x += 10.0;
        let place = shown(&mut glide, x, 0.05);
        if frame > 4 {
            assert!((place - x).abs() < 0.001, "{place} {x}");
        }
    }
}

/// A step that comes before the last one's done carries on from where it's shown.
#[test]
fn a_step_can_come_early() {
    let mut glide = ModelGlide::default();
    for _ in 0..30 {
        shown(&mut glide, 0.0, FRAME);
    }
    let mut last = 0.0;
    for frame in 0..80 {
        // Every 14 frames, which is sooner than a step takes.
        let x = f32::from(u8::try_from(frame / 14 + 1).expect("few")) * 10.0;
        let place = shown(&mut glide, x, FRAME);
        assert!(place - last < 2.0, "hopped from {last} to {place}");
        last = place;
    }
}

/// A teleport goes straight there.
#[test]
fn a_teleport_is_not_walked() {
    let mut glide = ModelGlide::default();
    for _ in 0..30 {
        shown(&mut glide, 0.0, FRAME);
    }
    assert!((shown(&mut glide, 5000.0, FRAME) - 5000.0).abs() < 0.001);
}

/// A monster that turns as it steps is shown turning, the short way round.
#[test]
fn a_turn_is_shown_going_round() {
    let mut glide = ModelGlide::default();
    let face = |glide: &mut ModelGlide, yaw| {
        let mut drawn = [at(1, 0.0, yaw, 0)];
        glide.smooth(&mut drawn, FRAME);
        drawn[0].state.angles.y
    };
    for _ in 0..30 {
        face(&mut glide, 350.0);
    }
    for _ in 0..9 {
        face(&mut glide, 10.0);
    }
    let yaw = face(&mut glide, 10.0).rem_euclid(360.0);
    assert!(!(20.0..=340.0).contains(&yaw), "{yaw}");
    assert!(yaw > 350.0 || yaw < 10.0, "partway: {yaw}");
}

/// A new frame blends in over a tenth of a second, from the one before.
#[test]
fn a_frame_blends_in() {
    let mut glide = ModelGlide::default();
    let pose = |glide: &mut ModelGlide, frame| {
        let mut drawn = [at(1, 0.0, 0.0, frame)];
        glide.smooth(&mut drawn, FRAME);
        (drawn[0].from_frame, drawn[0].blend)
    };
    for _ in 0..30 {
        pose(&mut glide, 3);
    }
    assert_eq!(pose(&mut glide, 3), (3, 1.0));
    let (from, blend) = pose(&mut glide, 4);
    assert_eq!(from, 3);
    assert!(blend < 0.1, "{blend}");
    for _ in 0..9 {
        pose(&mut glide, 4);
    }
    let (from, blend) = pose(&mut glide, 4);
    assert_eq!(from, 3);
    assert!((blend - 0.5).abs() < 0.1, "{blend}");
    for _ in 0..12 {
        pose(&mut glide, 4);
    }
    assert!((pose(&mut glide, 4).1 - 1.0).abs() < 0.001);
}

/// Each keeps to itself, and what's gone is forgotten, so coming back isn't a step from before.
#[test]
fn what_goes_is_forgotten() {
    let mut glide = ModelGlide::default();
    for _ in 0..30 {
        let mut both = [at(1, 0.0, 0.0, 0), at(2, 100.0, 0.0, 0)];
        glide.smooth(&mut both, FRAME);
    }
    let mut both = [at(1, 10.0, 0.0, 0), at(2, 100.0, 0.0, 0)];
    glide.smooth(&mut both, FRAME);
    assert!(both[0].state.origin.x < 1.0);
    assert!((both[1].state.origin.x - 100.0).abs() < 0.001);
    let mut gone = [at(1, 10.0, 0.0, 0)];
    glide.smooth(&mut gone, FRAME);
    let mut back = [at(1, 10.0, 0.0, 0), at(2, 150.0, 0.0, 0)];
    glide.smooth(&mut back, FRAME);
    assert!((back[1].state.origin.x - 150.0).abs() < 0.001);
}
