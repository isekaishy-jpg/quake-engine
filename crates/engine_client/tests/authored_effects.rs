//! Effects an author described.

use std::sync::Arc;

use engine_client::EffectSystem;
use engine_core::entity::effect_id;
use engine_core::math::Vec3;
use engine_map::{
    ColorCurve, Curve, EffectDef, EmitterShape, ParticleBlend, ParticleEmitter, ParticleKind,
};

/// A system that knows `emitters`, as an effect called `test`.
fn system(emitters: Vec<ParticleEmitter>) -> EffectSystem {
    let mut effects = EffectSystem::default();
    let def = EffectDef {
        name: "test".to_owned(),
        emitters,
    };
    effects.set_effects(&[Arc::new(def)]);
    effects
}

/// Nothing moves.
fn nowhere(_: u64) -> Option<Vec3> {
    None
}

#[test]
fn a_burst_starts_all_at_once() {
    let mut effects = system(vec![ParticleEmitter {
        burst: 30,
        ..ParticleEmitter::default()
    }]);
    effects.start(effect_id("test"), Vec3::ZERO, None, false);
    effects.update(0.01, &nowhere);
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
        effects.update(0.01, &nowhere);
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
    effects.update(0.1, &nowhere);
    assert_eq!(effects.count(), 10);
    effects.update(0.6, &nowhere);
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
        effects.update(0.05, &nowhere);
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
    effects.update(0.5, &nowhere);
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
    effects.update(0.05, &nowhere);
    assert_eq!(effects.count(), 2, "the second doesn't run by itself");
    effects.update(0.1, &nowhere);
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
        effects.update(0.05, &nowhere);
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
    effects.update(0.01, &nowhere);
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
    effects.update(0.01, &nowhere);
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
    effects.update(0.1, &place);
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
    effects.update(0.1, &nowhere);
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
    effects.update(0.1, &nowhere);
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
        short.update(0.05, &nowhere);
    }
    assert_eq!(short.count(), 0);
    let mut endless = system(vec![ParticleEmitter {
        duration: 0.0,
        ..emitter
    }]);
    endless.start(effect_id("test"), Vec3::ZERO, None, true);
    for _ in 0..20 {
        endless.update(0.05, &nowhere);
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
    };
    let mut system = EffectSystem::default();
    system.set_effects(&[Arc::new(effect)]);
    system.start(effect_id("one"), Vec3::ZERO, None, false);
    system.update(0.001, &|_| None);
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
    system.update(1.0, &|_| None);
    assert!((the_draw(&system).frame - 3.5).abs() < 0.1, "half way");
    system.update(0.99, &|_| None);
    assert!(the_draw(&system).frame <= 7.0, "and it stops on the last");
    assert!(the_draw(&system).frame > 6.5);
}

#[test]
fn a_flipbook_can_go_at_its_own_rate_and_round_again() {
    let mut system = one_emitter(|emitter| {
        emitter.flipbook = [2, 2];
        emitter.flip_rate = 10.0;
    });
    system.update(0.5, &|_| None);
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
    system.update(1.0, &|_| None);
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
