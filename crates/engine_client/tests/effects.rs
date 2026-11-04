//! Particles, dynamic lights and beams.

use engine_client::{EffectSource, Effects, Particles, Trail};
use engine_core::entity::Effect;
use engine_core::math::Vec3;

/// A temporary entity at `origin`.
fn temp(kind: u8, origin: Vec3) -> Effect {
    Effect::Temp {
        kind,
        origin,
        end: origin,
        entity: 0,
    }
}

/// Runs `effects` on to `until`, in steps of a 72nd of a second.
fn run(effects: &mut Effects, from: f32, until: f32) {
    let mut time = from;
    while time < until {
        time += 1.0 / 72.0;
        effects.update(time);
    }
}

#[test]
fn an_explosion_makes_particles_a_light_and_a_sound() {
    let mut effects = Effects::default();
    effects.update(1.0);
    effects.start(&temp(3, Vec3::new(10.0, 0.0, 0.0)));
    assert_eq!(effects.particles().live().len(), 1024);
    let light = effects.lights().next().expect("a light");
    assert!((light.radius - 350.0).abs() < 0.001);
    let sounds = effects.take_sounds();
    assert_eq!(sounds.len(), 1);
    assert_eq!(sounds[0].name, "weapons/r_exp3.wav");
}

#[test]
fn an_explosion_fades_away() {
    let mut effects = Effects::default();
    effects.update(1.0);
    effects.start(&temp(3, Vec3::ZERO));
    run(&mut effects, 1.0, 1.25);
    let light = effects.lights().next().expect("still lit");
    assert!(light.radius < 300.0, "shrinks: {}", light.radius);
    run(&mut effects, 1.25, 2.0);
    assert_eq!(effects.lights().count(), 0);
    assert!(effects.particles().live().is_empty());
}

#[test]
fn a_gunshot_is_a_small_silent_puff() {
    let mut effects = Effects::default();
    effects.start(&temp(2, Vec3::ZERO));
    assert_eq!(effects.particles().live().len(), 20);
    assert!(effects.take_sounds().is_empty());
}

#[test]
fn spikes_hitting_walls_make_a_noise() {
    let mut effects = Effects::default();
    for _ in 0..50 {
        effects.start(&temp(0, Vec3::ZERO));
    }
    let names: Vec<String> = effects.take_sounds().into_iter().map(|s| s.name).collect();
    assert_eq!(names.len(), 50);
    assert!(names.iter().any(|name| name == "weapons/tink1.wav"));
    assert!(names.iter().any(|name| name.contains("ric")));
}

#[test]
fn a_blood_puff_keeps_its_colour_band() {
    let mut effects = Effects::default();
    effects.start(&Effect::Particles {
        origin: Vec3::ZERO,
        direction: Vec3::ZERO,
        color: 73,
        count: 30,
    });
    let live = effects.particles().live();
    assert_eq!(live.len(), 30);
    assert!(
        live.iter()
            .all(|particle| (72..80).contains(&particle.color))
    );
}

#[test]
fn trails_are_even_at_any_frame_rate() {
    let count_at = |steps: u32| {
        let mut particles = Particles::default();
        let mut carried = 0.0;
        #[expect(clippy::cast_precision_loss, reason = "small counts")]
        let step = 300.0 / steps as f32;
        for index in 0..steps {
            #[expect(clippy::cast_precision_loss, reason = "small counts")]
            let from = Vec3::new(step * index as f32, 0.0, 0.0);
            let to = from + Vec3::new(step, 0.0, 0.0);
            particles.trail(from, to, Trail::Rocket, &mut carried);
        }
        particles.live().len()
    };
    let (slow, fast) = (count_at(20), count_at(2000));
    assert!((99..=101).contains(&slow), "{slow}");
    assert!(slow.abs_diff(fast) <= 1, "{slow} and {fast}");
}

#[test]
fn a_rocket_leaves_a_trail_and_a_light() {
    let mut effects = Effects::default();
    effects.update(1.0);
    let mut rocket = EffectSource {
        id: 5,
        model_flags: 1,
        ..EffectSource::default()
    };
    effects.follow(&[rocket]);
    assert!(effects.particles().live().is_empty(), "nothing yet");
    rocket.origin = Vec3::new(30.0, 0.0, 0.0);
    effects.follow(&[rocket]);
    assert_eq!(effects.particles().live().len(), 10);
    assert_eq!(effects.lights().count(), 1);
}

#[test]
fn a_following_light_is_moved_not_added() {
    let mut effects = Effects::default();
    effects.update(1.0);
    let mut flash = EffectSource {
        id: 3,
        effects: 2,
        ..EffectSource::default()
    };
    effects.follow(&[flash]);
    flash.origin = Vec3::new(50.0, 0.0, 0.0);
    effects.follow(&[flash]);
    assert_eq!(effects.lights().count(), 1);
    let light = effects.lights().next().expect("a light");
    assert!((light.origin.x - 68.0).abs() < 0.001, "{:?}", light.origin);
    assert!((light.min_light - 32.0).abs() < 0.001);
}

#[test]
fn an_entity_with_a_light_can_be_the_first_entity() {
    let mut effects = Effects::default();
    effects.update(1.0);
    effects.follow(&[EffectSource {
        id: 0,
        effects: 8,
        ..EffectSource::default()
    }]);
    effects.start(&temp(3, Vec3::ZERO));
    assert_eq!(
        effects.lights().count(),
        2,
        "an explosion takes its own slot"
    );
}

#[test]
fn a_beam_runs_from_end_to_end_in_pieces() {
    let mut effects = Effects::default();
    effects.update(1.0);
    effects.start(&Effect::Temp {
        kind: 6,
        origin: Vec3::ZERO,
        end: Vec3::new(100.0, 0.0, 0.0),
        entity: 9,
    });
    let pieces = effects.beam_pieces(1, Vec3::ZERO);
    assert_eq!(pieces.len(), 4);
    assert_eq!(pieces[0].model, "progs/bolt2.mdl");
    assert!((pieces[1].origin.x - 30.0).abs() < 0.001);
    assert!(pieces[0].angles.y.abs() < 0.001);
}

#[test]
fn a_players_beam_starts_at_the_player_and_goes_away() {
    let mut effects = Effects::default();
    effects.update(1.0);
    let beam = Effect::Temp {
        kind: 6,
        origin: Vec3::ZERO,
        end: Vec3::new(100.0, 0.0, 0.0),
        entity: 9,
    };
    effects.start(&beam);
    effects.start(&beam);
    let pieces = effects.beam_pieces(9, Vec3::new(40.0, 0.0, 0.0));
    assert_eq!(pieces.len(), 2, "one beam, from the player");
    assert!((pieces[0].origin.x - 40.0).abs() < 0.001);
    run(&mut effects, 1.0, 1.3);
    assert!(effects.beam_pieces(9, Vec3::ZERO).is_empty());
}

#[test]
fn a_teleport_splash_rises_from_a_cloud() {
    let mut effects = Effects::default();
    effects.start(&temp(11, Vec3::ZERO));
    assert_eq!(effects.particles().live().len(), 8 * 8 * 14);
}
