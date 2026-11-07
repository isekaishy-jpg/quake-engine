//! Effects an author described.

use std::sync::Arc;

use engine_client::{EffectSettings, EffectSystem, EffectWorld};
use engine_core::entity::effect_id;
use engine_core::math::Vec3;
use engine_map::{
    ColorCurve, Curve, EffectDef, EmitterShape, ParticleBlend, ParticleCollide, ParticleEmitter,
    ParticleForce, ParticleKind,
};

/// Nothing to follow, and nothing to hit.
fn quiet() -> EffectWorld<'static> {
    EffectWorld {
        place: &|_| None,
        hit: &|_, _| None,
    }
}

/// A floor at height 0.
fn floor(from: Vec3, to: Vec3) -> Option<(Vec3, Vec3)> {
    if from.z >= 0.0 && to.z < 0.0 {
        let share = from.z / (from.z - to.z);
        return Some((from + (to - from) * share, Vec3::new(0.0, 0.0, 1.0)));
    }
    None
}

/// A system that knows `emitters`, as an effect called `test`.
fn system(emitters: Vec<ParticleEmitter>) -> EffectSystem {
    let mut effects = EffectSystem::default();
    let def = EffectDef {
        name: "test".to_owned(),
        emitters,
        timeline: Vec::new(),
    };
    effects.set_effects(&[Arc::new(def)]);
    effects
}

#[test]
fn a_burst_starts_all_at_once() {
    let mut effects = system(vec![ParticleEmitter {
        burst: 30,
        ..ParticleEmitter::default()
    }]);
    effects.start(effect_id("test"), Vec3::ZERO, None, false);
    effects.update(0.01, &quiet());
    assert_eq!(effects.count(), 30);
}

#[test]
fn a_rate_starts_so_many_a_second() {
    let mut effects = system(vec![ParticleEmitter {
        rate: 100.0,
        life: [10.0, 10.0],
        ..ParticleEmitter::default()
    }]);
    effects.start(effect_id("test"), Vec3::ZERO, None, true);
    for _ in 0..100 {
        effects.update(0.01, &quiet());
    }
    let count = effects.count();
    assert!((95..=101).contains(&count), "{count}");
}

#[test]
fn particles_go_when_their_life_is_up() {
    let mut effects = system(vec![ParticleEmitter {
        burst: 10,
        life: [0.5, 0.5],
        ..ParticleEmitter::default()
    }]);
    effects.start(effect_id("test"), Vec3::ZERO, None, false);
    effects.update(0.1, &quiet());
    assert_eq!(effects.count(), 10);
    effects.update(0.6, &quiet());
    assert_eq!(effects.count(), 0);
}

#[test]
fn gravity_pulls_them_down() {
    let mut effects = system(vec![ParticleEmitter {
        burst: 1,
        life: [5.0, 5.0],
        gravity: 1.0,
        ..ParticleEmitter::default()
    }]);
    effects.start(effect_id("test"), Vec3::ZERO, None, false);
    for _ in 0..10 {
        effects.update(0.05, &quiet());
    }
    let (draws, _) = effects.draws(&|_| [1.0; 3]);
    assert!(draws[0].origin.z < -50.0, "{:?}", draws[0].origin);
}

#[test]
fn size_and_colour_change_over_a_life() {
    let mut effects = system(vec![ParticleEmitter {
        burst: 1,
        life: [1.0, 1.0],
        size: Curve(vec![(0.0, 0.0), (1.0, 10.0)]),
        color: ColorCurve(vec![
            (0.0, [1.0, 0.0, 0.0, 1.0]),
            (1.0, [0.0, 0.0, 1.0, 1.0]),
        ]),
        ..ParticleEmitter::default()
    }]);
    effects.start(effect_id("test"), Vec3::ZERO, None, false);
    effects.update(0.5, &quiet());
    let (draws, _) = effects.draws(&|_| [1.0; 3]);
    assert!((draws[0].size - 5.0).abs() < 1.5, "{}", draws[0].size);
    assert!(
        draws[0].color[0] > 0.2 && draws[0].color[2] > 0.2,
        "{:?}",
        draws[0].color
    );
}

#[test]
fn one_emitter_can_start_another_where_a_particle_ends() {
    let mut effects = system(vec![
        ParticleEmitter {
            name: "first".to_owned(),
            burst: 2,
            life: [0.1, 0.1],
            on_death: Some("second".to_owned()),
            ..ParticleEmitter::default()
        },
        ParticleEmitter {
            name: "second".to_owned(),
            burst: 5,
            rate: 1000.0,
            life: [10.0, 10.0],
            ..ParticleEmitter::default()
        },
    ]);
    effects.start(effect_id("test"), Vec3::ZERO, None, false);
    effects.update(0.05, &quiet());
    assert_eq!(effects.count(), 2, "the second doesn't run by itself");
    effects.update(0.1, &quiet());
    assert_eq!(effects.count(), 10, "five for each of the first");
}

#[test]
fn a_ribbon_is_a_line_through_where_it_has_been() {
    let mut effects = system(vec![ParticleEmitter {
        burst: 1,
        kind: ParticleKind::Ribbon,
        life: [5.0, 5.0],
        speed: [100.0, 100.0],
        ..ParticleEmitter::default()
    }]);
    effects.start(effect_id("test"), Vec3::ZERO, None, false);
    for _ in 0..5 {
        effects.update(0.05, &quiet());
    }
    let (draws, _) = effects.draws(&|_| [1.0; 3]);
    assert_eq!(draws.len(), 5);
    assert_eq!(draws[0].end, draws[1].origin, "each joins the next");
}

#[test]
fn a_light_comes_out_as_a_light() {
    let mut effects = system(vec![ParticleEmitter {
        burst: 1,
        kind: ParticleKind::Light,
        reach: 100.0,
        color: ColorCurve(vec![(0.0, [1.0, 0.5, 0.0, 1.0])]),
        ..ParticleEmitter::default()
    }]);
    effects.start(effect_id("test"), Vec3::new(1.0, 2.0, 3.0), None, false);
    effects.update(0.01, &quiet());
    let (draws, lights) = effects.draws(&|_| [1.0; 3]);
    assert!(draws.is_empty());
    assert_eq!(lights.len(), 1);
    assert_eq!(lights[0].color, [1.0, 0.5, 0.0]);
    assert_eq!(lights[0].reach, 100.0);
}

#[test]
fn what_adds_says_so() {
    let mut effects = system(vec![ParticleEmitter {
        burst: 1,
        blend: ParticleBlend::Add,
        ..ParticleEmitter::default()
    }]);
    effects.start(effect_id("test"), Vec3::ZERO, None, false);
    effects.update(0.01, &quiet());
    assert!(effects.draws(&|_| [1.0; 3]).0[0].adds);
}

#[test]
fn an_effect_follows_what_its_attached_to() {
    let mut effects = system(vec![ParticleEmitter {
        rate: 1000.0,
        life: [5.0, 5.0],
        ..ParticleEmitter::default()
    }]);
    effects.start(effect_id("test"), Vec3::ZERO, Some(7), true);
    let place = |entity: u64| (entity == 7).then_some(Vec3::new(500.0, 0.0, 0.0));
    let world = EffectWorld {
        place: &place,
        hit: &|_, _| None,
    };
    effects.update(0.1, &world);
    let (draws, _) = effects.draws(&|_| [1.0; 3]);
    assert!(draws.iter().all(|draw| draw.origin.x > 400.0));
}

#[test]
fn what_the_gpu_works_out_never_reaches_the_cpu() {
    let mut effects = system(vec![ParticleEmitter {
        gpu: true,
        rate: 1000.0,
        life: [2.0, 2.0],
        shape: EmitterShape::Ball(30.0),
        ..ParticleEmitter::default()
    }]);
    effects.start(effect_id("test"), Vec3::ZERO, None, true);
    effects.update(0.1, &quiet());
    assert_eq!(effects.count(), 0);
    let batches = effects.gpu_emitters();
    assert_eq!(batches.len(), 1);
    assert_eq!(batches[0].count, 2000, "a rate for as long as one lives");
    assert_eq!(batches[0].radius, 30.0);
}

#[test]
fn an_effect_with_no_such_name_does_nothing() {
    let mut effects = system(vec![ParticleEmitter::default()]);
    effects.start(effect_id("nothing"), Vec3::ZERO, None, false);
    effects.update(0.1, &quiet());
    assert_eq!(effects.count(), 0);
}

#[test]
fn a_short_effect_ends_and_a_lasting_one_doesnt() {
    let emitter = ParticleEmitter {
        rate: 100.0,
        duration: 0.1,
        life: [0.1, 0.1],
        ..ParticleEmitter::default()
    };
    let mut short = system(vec![emitter.clone()]);
    short.start(effect_id("test"), Vec3::ZERO, None, false);
    for _ in 0..20 {
        short.update(0.05, &quiet());
    }
    assert_eq!(short.count(), 0);
    let mut endless = system(vec![ParticleEmitter {
        duration: 0.0,
        ..emitter
    }]);
    endless.start(effect_id("test"), Vec3::ZERO, None, true);
    for _ in 0..20 {
        endless.update(0.05, &quiet());
    }
    assert!(endless.count() > 0);
}

/// One emitter, with `set` done to it.
fn one_emitter(set: impl Fn(&mut ParticleEmitter)) -> EffectSystem {
    let mut emitter = ParticleEmitter {
        name: "it".to_owned(),
        burst: 1,
        life: [2.0, 2.0],
        ..ParticleEmitter::default()
    };
    set(&mut emitter);
    let effect = EffectDef {
        name: "one".to_owned(),
        emitters: vec![emitter],
        timeline: Vec::new(),
    };
    let mut system = EffectSystem::default();
    system.set_effects(&[Arc::new(effect)]);
    system.start(effect_id("one"), Vec3::ZERO, None, false);
    system.update(0.001, &quiet());
    system
}

/// The one thing drawn.
fn the_draw(system: &EffectSystem) -> engine_client::EffectDraw {
    system.draws(&|_| [1.0; 3]).0[0]
}

#[test]
fn a_flipbook_goes_through_its_frames_over_a_life() {
    let mut system = one_emitter(|emitter| emitter.flipbook = [4, 2]);
    assert!(the_draw(&system).frame < 0.1);
    assert_eq!(the_draw(&system).sheet, [4, 2]);
    system.update(1.0, &quiet());
    assert!((the_draw(&system).frame - 3.5).abs() < 0.1, "half way");
    system.update(0.99, &quiet());
    assert!(the_draw(&system).frame <= 7.0, "and it stops on the last");
    assert!(the_draw(&system).frame > 6.5);
}

#[test]
fn a_flipbook_can_go_at_its_own_rate_and_round_again() {
    let mut system = one_emitter(|emitter| {
        emitter.flipbook = [2, 2];
        emitter.flip_rate = 10.0;
    });
    system.update(0.5, &quiet());
    assert!(
        (the_draw(&system).frame - 1.0).abs() < 0.1,
        "{}",
        the_draw(&system).frame
    );
}

#[test]
fn a_particle_starts_turned_and_goes_on_turning() {
    let mut system = one_emitter(|emitter| {
        emitter.rotation = [90.0, 90.0];
        emitter.spin = [180.0, 180.0];
    });
    let start = the_draw(&system).rotation;
    assert!(
        (start - std::f32::consts::FRAC_PI_2).abs() < 0.02,
        "{start}"
    );
    system.update(1.0, &quiet());
    let turned = the_draw(&system).rotation - start;
    assert!((turned - std::f32::consts::PI).abs() < 0.02, "{turned}");
}

#[test]
fn what_is_stretched_is_a_line_along_how_it_moves() {
    let system = one_emitter(|emitter| {
        emitter.speed = [100.0, 100.0];
        emitter.spread = 0.0;
        emitter.stretch = 0.5;
    });
    let draw = the_draw(&system);
    assert_eq!(draw.kind, ParticleKind::Ribbon);
    assert!((draw.end.z - draw.origin.z - 50.0).abs() < 1.0, "{draw:?}");
    assert!(draw.end.x.abs() < 1.0);
}

#[test]
fn what_is_lit_takes_the_light_where_it_is_and_nothing_else_does() {
    let lit = one_emitter(|emitter| emitter.lit = true);
    let color = lit.draws(&|_| [0.25, 0.5, 1.0]).0[0].color;
    assert_eq!(color, [0.25, 0.5, 1.0, 1.0]);
    let plain = one_emitter(|_| ());
    let color = plain.draws(&|_| [0.25, 0.5, 1.0]).0[0].color;
    assert_eq!(color, [1.0; 4]);
}

#[test]
fn what_a_particle_is_drawn_with_comes_from_its_emitter() {
    let system = one_emitter(|emitter| {
        emitter.picture = 3;
        emitter.shader_number = 2;
        emitter.soft = 24.0;
    });
    let draw = the_draw(&system);
    assert_eq!((draw.picture, draw.shader), (3, 2));
    assert!((draw.soft - 24.0).abs() < f32::EPSILON);
}

/// What falls straight down from `high`, with `set` done to it.
fn dropped(high: f32, set: impl Fn(&mut ParticleEmitter)) -> EffectSystem {
    let mut emitter = ParticleEmitter {
        name: "drop".to_owned(),
        burst: 1,
        life: [5.0, 5.0],
        gravity: 1.0,
        ..ParticleEmitter::default()
    };
    set(&mut emitter);
    let mut system = system(vec![emitter]);
    system.start(effect_id("test"), Vec3::new(0.0, 0.0, high), None, false);
    let world = EffectWorld {
        place: &|_| None,
        hit: &floor,
    };
    system.update(0.001, &world);
    system
}

/// Runs `system` on for `seconds` over the floor.
fn fall(system: &mut EffectSystem, seconds: f32) {
    let world = EffectWorld {
        place: &|_| None,
        hit: &floor,
    };
    let mut left = seconds;
    while left > 0.0 {
        system.update(1.0 / 60.0, &world);
        left -= 1.0 / 60.0;
    }
}

#[test]
fn what_goes_through_goes_through() {
    let mut system = dropped(40.0, |_| ());
    fall(&mut system, 1.0);
    assert!(the_draw(&system).origin.z < -100.0);
}

#[test]
fn what_bounces_comes_up_again_and_stays_above() {
    let mut system = dropped(40.0, |emitter| {
        emitter.collide = ParticleCollide::Bounce;
        emitter.bounce = 0.6;
    });
    let mut lowest = f32::MAX;
    let mut last = 40.0;
    let mut rose = false;
    for _ in 0..120 {
        fall(&mut system, 1.0 / 60.0);
        let z = the_draw(&system).origin.z;
        lowest = lowest.min(z);
        rose |= z > last + 0.01;
        last = z;
    }
    assert!(lowest >= 0.0, "{lowest}");
    assert!(rose, "it came up");
}

#[test]
fn what_sticks_stays_and_faces_out() {
    let mut system = dropped(40.0, |emitter| {
        emitter.collide = ParticleCollide::Stick;
        emitter.kind = ParticleKind::Decal;
    });
    fall(&mut system, 1.0);
    let draw = the_draw(&system);
    assert!(draw.origin.z.abs() < 1.0, "{draw:?}");
    fall(&mut system, 1.0);
    assert!(
        (the_draw(&system).origin.z - draw.origin.z).abs() < 0.01,
        "it stayed"
    );
    assert!(
        (draw.end.z - 1.0).abs() < 0.01,
        "and faces away from what it hit"
    );
}

#[test]
fn what_dies_on_hitting_goes_and_starts_what_it_says() {
    let mut system = system(vec![
        ParticleEmitter {
            name: "drop".to_owned(),
            burst: 1,
            life: [5.0, 5.0],
            gravity: 1.0,
            collide: ParticleCollide::Die,
            on_hit: Some("splash".to_owned()),
            ..ParticleEmitter::default()
        },
        ParticleEmitter {
            name: "splash".to_owned(),
            burst: 6,
            life: [5.0, 5.0],
            ..ParticleEmitter::default()
        },
    ]);
    system.start(effect_id("test"), Vec3::new(0.0, 0.0, 40.0), None, false);
    assert_eq!(system.count(), 0);
    fall(&mut system, 0.05);
    assert_eq!(system.count(), 1, "only the drop");
    fall(&mut system, 1.0);
    assert_eq!(system.count(), 6, "it's gone, and the splash is there");
    assert!(the_draw(&system).origin.z.abs() < 1.0);
}

#[test]
fn a_timeline_sets_things_off_when_it_says() {
    let mut effect = EffectDef {
        name: "test".to_owned(),
        emitters: vec![ParticleEmitter {
            name: "bang".to_owned(),
            burst: 3,
            life: [9.0, 9.0],
            ..ParticleEmitter::default()
        }],
        timeline: vec![(0.5, "bang".to_owned()), (1.0, "bang".to_owned())],
    };
    effect.emitters[0].rate = 0.0;
    let mut system = EffectSystem::default();
    system.set_effects(&[Arc::new(effect)]);
    system.start(effect_id("test"), Vec3::ZERO, None, false);
    fall(&mut system, 0.4);
    assert_eq!(system.count(), 0, "not yet");
    fall(&mut system, 0.2);
    assert_eq!(system.count(), 3);
    fall(&mut system, 0.6);
    assert_eq!(system.count(), 6, "and once more");
    fall(&mut system, 1.0);
    assert_eq!(system.count(), 6, "and no more");
}

#[test]
fn an_attractor_draws_things_in_and_only_within_its_reach() {
    let force = ParticleForce::Attract {
        at: Vec3::new(100.0, 0.0, 0.0),
        strength: 50.0,
        reach: 0.0,
    };
    let pull = force.push(Vec3::ZERO, Vec3::ZERO, 0.0);
    assert!(
        (pull.x - 50.0).abs() < 0.01 && pull.y.abs() < 0.01,
        "{pull:?}"
    );
    let reaching = ParticleForce::Attract {
        at: Vec3::new(100.0, 0.0, 0.0),
        strength: 50.0,
        reach: 80.0,
    };
    assert_eq!(reaching.push(Vec3::ZERO, Vec3::ZERO, 0.0), Vec3::ZERO);
    let near = reaching.push(Vec3::new(60.0, 0.0, 0.0), Vec3::ZERO, 0.0);
    assert!((near.x - 25.0).abs() < 0.01, "half way in: {near:?}");
}

#[test]
fn a_vortex_pushes_round_its_line() {
    let force = ParticleForce::Vortex {
        at: Vec3::ZERO,
        axis: Vec3::new(0.0, 0.0, 1.0),
        strength: 30.0,
        inward: 10.0,
    };
    let push = force.push(Vec3::new(100.0, 0.0, 40.0), Vec3::ZERO, 0.0);
    assert!((push.y - 30.0).abs() < 0.01, "round: {push:?}");
    assert!((push.x + 10.0).abs() < 0.01, "and in: {push:?}");
    assert!(push.z.abs() < 0.01, "and not along it: {push:?}");
}

#[test]
fn wind_blows_and_comes_and_goes() {
    let steady = ParticleForce::Wind {
        blowing: Vec3::new(10.0, 0.0, 0.0),
        gusts: 0.0,
    };
    assert_eq!(steady.push(Vec3::ZERO, Vec3::ZERO, 3.0).x, 10.0);
    let gusty = ParticleForce::Wind {
        blowing: Vec3::new(10.0, 0.0, 0.0),
        gusts: 1.0,
    };
    let (one, two) = (
        gusty.push(Vec3::ZERO, Vec3::ZERO, 1.0).x,
        gusty.push(Vec3::ZERO, Vec3::ZERO, 3.0).x,
    );
    assert!((one - two).abs() > 1.0, "{one} and {two}");
}

/// Where everything `shape` starts is, from many.
fn started_in(shape: EmitterShape, outward: bool) -> Vec<(Vec3, Vec3)> {
    let mut system = system(vec![ParticleEmitter {
        name: "many".to_owned(),
        burst: 300,
        life: [9.0, 9.0],
        speed: [10.0, 10.0],
        shape,
        outward,
        spread: 0.0,
        ..ParticleEmitter::default()
    }]);
    system.start(effect_id("test"), Vec3::ZERO, None, false);
    system.update(0.0001, &quiet());
    let first = system.draws(&|_| [1.0; 3]).0;
    system.update(1.0, &quiet());
    let second = system.draws(&|_| [1.0; 3]).0;
    first
        .iter()
        .zip(second)
        .map(|(a, b)| (a.origin, b.origin - a.origin))
        .collect()
}

#[test]
fn a_ring_is_a_ring_and_a_disc_is_filled() {
    let ring = started_in(EmitterShape::Ring(50.0), false);
    for (at, _) in &ring {
        assert!((at.length() - 50.0).abs() < 0.5, "{at:?}");
        assert!(at.z.abs() < 0.5);
    }
    let disc = started_in(EmitterShape::Disc(50.0), false);
    let inside = disc.iter().filter(|(at, _)| at.length() < 25.0).count();
    assert!(
        inside > 40 && inside < 120,
        "a quarter of its area: {inside}"
    );
    assert!(disc.iter().all(|(at, _)| at.length() <= 50.5));
}

#[test]
fn a_cone_widens_going_up_and_a_line_is_straight() {
    let cone = started_in(EmitterShape::Cone(40.0, 100.0), false);
    for (at, _) in &cone {
        let flat = at.x.hypot(at.y);
        assert!(at.z >= -0.5 && at.z <= 100.5, "{at:?}");
        assert!(flat <= at.z * 0.4 + 0.5, "inside it: {at:?}");
    }
    let line = started_in(EmitterShape::Line(Vec3::new(30.0, 0.0, 0.0)), false);
    for (at, _) in &line {
        assert!(
            at.x.abs() <= 30.5 && at.y.abs() < 0.5 && at.z.abs() < 0.5,
            "{at:?}"
        );
    }
}

#[test]
fn outward_goes_away_from_the_middle() {
    for (at, moved) in started_in(EmitterShape::Ring(50.0), true) {
        let out = at.normalized().dot(moved.normalized());
        assert!(out > 0.99, "{at:?} {moved:?}");
    }
}

/// An effect that makes one each frame for as long as it's going, called `test`.
fn steady(handle: u32, settings: EffectSettings) -> EffectSystem {
    let mut system = system(vec![ParticleEmitter {
        name: "steady".to_owned(),
        rate: 60.0,
        life: [9.0, 9.0],
        ..ParticleEmitter::default()
    }]);
    system.start_as(
        effect_id("test"),
        Vec3::ZERO,
        None,
        false,
        (handle, settings),
    );
    system
}

#[test]
fn stopping_an_effect_ends_it_and_fades_what_it_made() {
    let mut system = steady(7, EffectSettings::default());
    fall(&mut system, 1.0);
    let many = system.count();
    assert!(many > 50, "{many}");
    system.stop(7, 1.0);
    fall(&mut system, 0.5);
    assert_eq!(system.count(), many, "no more, and they're fading");
    assert_eq!(system.going(), 0);
    let alpha = the_draw(&system).color[3];
    assert!(alpha > 0.3 && alpha < 0.7, "{alpha}");
    fall(&mut system, 0.6);
    assert_eq!(system.count(), 0, "and gone");
}

#[test]
fn stopping_at_once_leaves_nothing_and_only_that_one_stops() {
    let mut system = steady(7, EffectSettings::default());
    system.start_as(
        effect_id("test"),
        Vec3::ZERO,
        None,
        false,
        (8, EffectSettings::default()),
    );
    fall(&mut system, 0.5);
    let both = system.count();
    system.stop(7, 0.0);
    assert!(system.count() < both, "its own are gone");
    assert!(system.count() > 0, "and the other's aren't");
    fall(&mut system, 0.02);
    assert_eq!(system.going(), 1);
    system.stop(99, 1.0);
    assert_eq!(system.going(), 1, "nothing's called that");
}

#[test]
fn an_instance_has_its_own_colour_size_and_strength() {
    let own = EffectSettings {
        color: [1.0, 0.5, 0.25],
        size: 2.0,
        strength: 0.5,
    };
    let mut system = steady(1, own);
    fall(&mut system, 1.0);
    let draw = the_draw(&system);
    assert_eq!(draw.color[..3], [1.0, 0.5, 0.25]);
    assert!((draw.size - 8.0).abs() < 0.01, "{}", draw.size);
    assert!(
        (25..=35).contains(&system.count()),
        "half as many: {}",
        system.count()
    );
}

#[test]
fn far_off_an_emitter_thins_out_to_nothing() {
    let made_from = |eye: Vec3| {
        let mut system = system(vec![ParticleEmitter {
            name: "steady".to_owned(),
            rate: 60.0,
            life: [9.0, 9.0],
            far: 1000.0,
            ..ParticleEmitter::default()
        }]);
        system.set_eye(eye);
        system.start(effect_id("test"), Vec3::ZERO, None, false);
        fall(&mut system, 1.0);
        system.count()
    };
    assert!(made_from(Vec3::new(100.0, 0.0, 0.0)) > 50);
    let half = made_from(Vec3::new(750.0, 0.0, 0.0));
    assert!((20..=40).contains(&half), "{half}");
    assert_eq!(made_from(Vec3::new(1200.0, 0.0, 0.0)), 0);
}

#[test]
fn with_the_budget_going_what_is_far_gives_way_first() {
    let made_at = |origin: Vec3| {
        let mut system = system(vec![ParticleEmitter {
            name: "steady".to_owned(),
            rate: 2000.0,
            life: [9.0, 9.0],
            ..ParticleEmitter::default()
        }]);
        system.set_budget(1000);
        system.start(effect_id("test"), origin, None, false);
        fall(&mut system, 2.0);
        system.count()
    };
    let near = made_at(Vec3::ZERO);
    let far = made_at(Vec3::new(4000.0, 0.0, 0.0));
    assert!(far < near, "{far} and {near}");
    assert!(far <= 1050, "it holds to its budget: {far}");
}

#[test]
fn a_force_moves_what_it_is_on() {
    let mut system = dropped(40.0, |emitter| {
        emitter.gravity = 0.0;
        emitter.forces = vec![ParticleForce::Wind {
            blowing: Vec3::new(100.0, 0.0, 0.0),
            gusts: 0.0,
        }];
    });
    fall(&mut system, 1.0);
    let x = the_draw(&system).origin.x;
    assert!((40.0..60.0).contains(&x), "half of a second's push: {x}");
}

/// What's stuck to a wall stays there, with nothing under it, and gravity doesn't take it.
#[test]
fn what_sticks_to_a_wall_stays_up() {
    let wall = |from: Vec3, to: Vec3| {
        (from.x <= 10.0 && to.x > 10.0)
            .then_some((Vec3::new(10.0, 0.0, to.z), Vec3::new(-1.0, 0.0, 0.0)))
    };
    let world = EffectWorld {
        place: &|_| None,
        hit: &wall,
    };
    let mut emitter = ParticleEmitter {
        name: "thrown".to_owned(),
        burst: 1,
        life: [9.0, 9.0],
        speed: [100.0, 100.0],
        direction: Vec3::new(1.0, 0.0, 0.0),
        spread: 0.0,
        gravity: 1.0,
        collide: ParticleCollide::Stick,
        ..ParticleEmitter::default()
    };
    emitter.kind = ParticleKind::Decal;
    let mut system = system(vec![emitter]);
    system.start(effect_id("test"), Vec3::ZERO, None, false);
    for _ in 0..30 {
        system.update(1.0 / 60.0, &world);
    }
    let stuck = the_draw(&system).origin;
    assert!((stuck.x - 9.75).abs() < 0.1, "{stuck:?}");
    for _ in 0..60 {
        system.update(1.0 / 60.0, &world);
    }
    assert!(
        (the_draw(&system).origin.z - stuck.z).abs() < 0.01,
        "it stayed up"
    );
}

/// An emitter compute runs.
fn computed(rate: f32, burst: u32) -> ParticleEmitter {
    ParticleEmitter {
        sim: true,
        rate,
        burst,
        life: [2.0, 2.0],
        ..ParticleEmitter::default()
    }
}

/// How many start in compute over `steps` of a hundredth of a second, and the slots they asked for.
fn computed_starts(effects: &mut EffectSystem, steps: u32) -> (u32, u32) {
    let mut started = 0;
    let mut slots = 0;
    for _ in 0..steps {
        effects.update(0.01, &quiet());
        for (sim, _) in effects.sims() {
            started += sim.spawn;
            slots = sim.count;
        }
    }
    (started, slots)
}

/// What compute runs starts by the same sums as the rest, and none of it's on the CPU.
#[test]
fn compute_is_told_how_many_start() {
    let mut effects = system(vec![computed(100.0, 30)]);
    effects.start(effect_id("test"), Vec3::ZERO, None, true);
    let (started, slots) = computed_starts(&mut effects, 100);
    assert!((128..=131).contains(&started), "{started}");
    assert!(slots >= 230, "room for all that can be alive: {slots}");
    assert_eq!(effects.count(), 0, "none on the CPU");
}

/// An instance that's stronger makes more, and has room for them.
#[test]
fn compute_makes_as_many_as_an_instance_says() {
    let mut effects = system(vec![computed(100.0, 30)]);
    let settings = EffectSettings {
        strength: 0.5,
        ..EffectSettings::default()
    };
    effects.start_as(effect_id("test"), Vec3::ZERO, None, true, (0, settings));
    let (started, slots) = computed_starts(&mut effects, 100);
    assert!((63..=66).contains(&started), "{started}");
    assert!(slots >= 130, "{slots}");
    let (sim, _) = effects.sims()[0];
    assert_eq!(sim.settings, settings);
    let mut effects = system(vec![computed(100.0, 30)]);
    let settings = EffectSettings {
        strength: 2.0,
        ..EffectSettings::default()
    };
    effects.start_as(effect_id("test"), Vec3::ZERO, None, true, (0, settings));
    let (started, slots) = computed_starts(&mut effects, 100);
    assert!((258..=261).contains(&started), "{started}");
    assert!(slots >= 460, "all of it: {slots}");
}

/// Each emitter is its own, from frame to frame, and steps differ.
#[test]
fn computed_emitters_keep_their_names_and_differ() {
    let mut effects = system(vec![computed(10.0, 0), computed(10.0, 0)]);
    effects.start(effect_id("test"), Vec3::ZERO, None, true);
    effects.update(0.01, &quiet());
    let first: Vec<_> = effects
        .sims()
        .iter()
        .map(|(sim, _)| (sim.id, sim.seed))
        .collect();
    assert_eq!(first.len(), 2);
    assert_ne!(first[0].0, first[1].0, "two emitters");
    assert_ne!(first[0].1, first[1].1, "that don't throw alike");
    effects.update(0.01, &quiet());
    let next: Vec<_> = effects
        .sims()
        .iter()
        .map(|(sim, _)| (sim.id, sim.seed))
        .collect();
    assert_eq!(first[0].0, next[0].0, "the same one");
    assert_ne!(first[0].1, next[0].1, "a step on");
    effects.start(effect_id("test"), Vec3::ZERO, None, true);
    effects.update(0.01, &quiet());
    let both: Vec<_> = effects.sims().iter().map(|(sim, _)| sim.id).collect();
    assert_eq!(both.len(), 4);
    assert!(
        !both[..2].contains(&both[2]),
        "a new start is another: {both:?}"
    );
}

/// Told to stop, it starts no more and fades, then its slots go.
#[test]
fn computed_particles_fade_when_told_to_stop() {
    let mut effects = system(vec![computed(100.0, 0)]);
    let settings = EffectSettings::default();
    effects.start_as(effect_id("test"), Vec3::ZERO, None, true, (7, settings));
    effects.update(0.1, &quiet());
    assert!(effects.sims()[0].0.spawn > 0);
    assert!((effects.sims()[0].0.fade - 1.0).abs() < 0.001);
    effects.stop(7, 1.0);
    assert_eq!(effects.sims()[0].0.spawn, 0, "at once");
    effects.update(0.5, &quiet());
    let (sim, _) = effects.sims()[0];
    assert_eq!(sim.spawn, 0);
    assert!((sim.fade - 0.5).abs() < 0.01, "{}", sim.fade);
    effects.update(0.6, &quiet());
    assert!(effects.sims().is_empty(), "and then there's no more of it");
}

/// Stopped with no time to fade, it's gone.
#[test]
fn computed_particles_stopped_at_once_are_gone() {
    let mut effects = system(vec![computed(100.0, 0)]);
    let settings = EffectSettings::default();
    effects.start_as(effect_id("test"), Vec3::ZERO, None, true, (7, settings));
    effects.update(0.1, &quiet());
    effects.stop(7, 0.0);
    effects.update(0.01, &quiet());
    assert!(effects.sims().is_empty());
}

/// What another emitter starts begins where that one was, so it isn't run from the middle.
#[test]
fn what_starts_from_another_is_not_run_by_compute() {
    let first = ParticleEmitter {
        name: "first".to_owned(),
        burst: 1,
        life: [0.01, 0.01],
        on_death: Some("second".to_owned()),
        ..ParticleEmitter::default()
    };
    let second = ParticleEmitter {
        name: "second".to_owned(),
        burst: 4,
        life: [1.0, 1.0],
        sim: true,
        ..ParticleEmitter::default()
    };
    let mut effects = system(vec![first, second]);
    effects.start(effect_id("test"), Vec3::ZERO, None, false);
    effects.update(0.001, &quiet());
    assert!(effects.sims().is_empty());
    effects.update(0.1, &quiet());
    assert_eq!(effects.count(), 4, "it still starts");
}

/// One that's waiting or done starts nothing, and it stays until what it made has gone.
#[test]
fn compute_waits_for_a_delay_and_stops_at_a_duration() {
    let mut effects = system(vec![ParticleEmitter {
        delay: 0.5,
        duration: 0.5,
        ..computed(100.0, 0)
    }]);
    effects.start(effect_id("test"), Vec3::ZERO, None, false);
    let (early, _) = computed_starts(&mut effects, 40);
    assert_eq!(early, 0);
    let (during, _) = computed_starts(&mut effects, 60);
    assert!((45..=55).contains(&during), "{during}");
    let (late, _) = computed_starts(&mut effects, 50);
    assert_eq!(late, 0);
    assert!(!effects.sims().is_empty(), "what it made still goes on");
    effects.update(3.0, &quiet());
    assert!(effects.sims().is_empty());
}

/// New ones take over from what's going, which starts again, and what's made stays as it was.
#[test]
fn changed_effects_take_over_what_is_going() {
    let old = ParticleEmitter {
        burst: 5,
        life: [10.0, 10.0],
        ..ParticleEmitter::default()
    };
    let mut effects = system(vec![old.clone()]);
    effects.start(effect_id("test"), Vec3::ZERO, None, true);
    effects.update(0.1, &quiet());
    assert_eq!(effects.count(), 5);
    let new = ParticleEmitter {
        burst: 7,
        ..old.clone()
    };
    let extra = ParticleEmitter {
        burst: 2,
        ..old.clone()
    };
    let def = EffectDef {
        name: "test".to_owned(),
        emitters: vec![new, extra],
        timeline: Vec::new(),
    };
    effects.change_effects(&[Arc::new(def)]);
    assert_eq!(effects.count(), 5, "they aren't touched");
    effects.update(0.1, &quiet());
    assert_eq!(effects.count(), 14, "and it's started over as it now is");
    effects.update(20.0, &quiet());
    assert_eq!(effects.count(), 0);
    effects.start(effect_id("test"), Vec3::ZERO, None, false);
    effects.update(0.1, &quiet());
    assert_eq!(effects.count(), 9, "what starts now is new");
}

/// An effect that's gone from the files goes on until it's over, and can't start again.
#[test]
fn an_effect_that_is_gone_goes_on_until_it_ends() {
    let mut effects = system(vec![ParticleEmitter {
        burst: 5,
        life: [10.0, 10.0],
        ..ParticleEmitter::default()
    }]);
    effects.start(effect_id("test"), Vec3::ZERO, None, false);
    effects.update(0.1, &quiet());
    effects.change_effects(&[]);
    effects.update(0.1, &quiet());
    assert_eq!(effects.count(), 5);
    effects.start(effect_id("test"), Vec3::ZERO, None, false);
    effects.update(0.1, &quiet());
    assert_eq!(effects.count(), 5);
}
