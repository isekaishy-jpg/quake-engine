//! Particles, dynamic lights and beams.

use engine_client::{EffectSource, EffectWorld, Effects};
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
        effects.authored_mut().update(1.0 / 72.0, &quiet());
    }
}

/// Nothing to follow, and nothing to hit.
fn quiet() -> EffectWorld<'static> {
    EffectWorld {
        place: &|_| None,
        hit: &|_, _| None,
    }
}

/// How many particles there are.
fn count(effects: &Effects) -> usize {
    effects.authored().count()
}

/// What's drawn of the particles, each by where it is and what palette number it has, with a
/// palette that has each number in its red.
fn palette_numbers(effects: &Effects) -> Vec<(Vec3, u8)> {
    let (draws, _) = effects.authored().draws(&|_| [1.0; 3]);
    let numbers = draws.iter().map(|draw| {
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "it's a byte"
        )]
        let number = (draw.color[0].powf(1.0 / 2.2) * 255.0).round() as u8;
        (draw.origin, number)
    });
    numbers.collect()
}

/// Effects whose palette has each number in its red.
fn numbered() -> Effects {
    let mut effects = Effects::default();
    effects.set_palette(&|index| [index, 0, 0]);
    effects
}

#[test]
fn an_explosion_makes_particles_a_light_and_a_sound() {
    let mut effects = Effects::default();
    effects.update(1.0);
    effects.start(&temp(3, Vec3::new(10.0, 0.0, 0.0)));
    assert_eq!(count(&effects), 1024);
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
    assert_eq!(count(&effects), 0);
}

#[test]
fn a_gunshot_is_a_small_silent_puff() {
    let mut effects = Effects::default();
    effects.start(&temp(2, Vec3::ZERO));
    assert_eq!(count(&effects), 20);
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
    let mut effects = numbered();
    effects.start(&Effect::Particles {
        origin: Vec3::ZERO,
        direction: Vec3::ZERO,
        color: 73,
        count: 30,
    });
    let live = palette_numbers(&effects);
    assert_eq!(live.len(), 30);
    assert!(
        live.iter().all(|(_, color)| (72..80).contains(color)),
        "{live:?}"
    );
}

#[test]
fn trails_are_even_at_any_frame_rate() {
    let count_at = |steps: u32| {
        let mut effects = Effects::default();
        let mut rocket = EffectSource {
            id: 5,
            model_flags: 1,
            ..EffectSource::default()
        };
        effects.follow(&[rocket]);
        #[expect(clippy::cast_precision_loss, reason = "small counts")]
        let step = 300.0 / steps as f32;
        for index in 1..=steps {
            #[expect(clippy::cast_precision_loss, reason = "small counts")]
            let along = step * index as f32;
            rocket.origin = Vec3::new(along, 0.0, 0.0);
            effects.follow(&[rocket]);
        }
        count(&effects)
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
    assert_eq!(count(&effects), 0, "nothing yet");
    rocket.origin = Vec3::new(30.0, 0.0, 0.0);
    effects.follow(&[rocket]);
    assert_eq!(count(&effects), 10);
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
    assert_eq!(count(&effects), 8 * 8 * 14);
}

/// The first half of an explosion goes down its ramp, and is gone at its end.
#[test]
fn an_explosion_goes_down_its_colours() {
    let mut effects = numbered();
    effects.update(1.0);
    effects.start(&temp(3, Vec3::ZERO));
    assert_eq!(count(&effects), 1024);
    let first = palette_numbers(&effects);
    let ramp = [
        0x6f, 0x6d, 0x6b, 0x69, 0x67, 0x65, 0x63, 0x61, 0x6e, 0x6c, 0x6a, 0x68, 0x66,
    ];
    assert!(
        first.iter().all(|(_, color)| ramp.contains(color)),
        "{first:?}"
    );
    assert!(
        first.iter().any(|(_, color)| *color == 0x6f),
        "from the top"
    );
    run(&mut effects, 1.0, 1.55);
    let later = palette_numbers(&effects);
    assert!(later.len() < 512, "the faster half's gone: {}", later.len());
    assert!(!later.is_empty());
    assert!(later.iter().all(|(_, color)| *color < 0x6d), "{later:?}");
    run(&mut effects, 1.55, 1.81);
    assert_eq!(count(&effects), 0);
}

/// Half of an explosion speeds up, and it spreads faster than it would at its speed.
#[test]
fn half_of_an_explosion_goes_ever_faster() {
    let mut effects = Effects::default();
    effects.start(&temp(3, Vec3::ZERO));
    run(&mut effects, 0.0, 0.3);
    let (draws, _) = effects.authored().draws(&|_| [1.0; 3]);
    let furthest = draws
        .iter()
        .map(|draw| draw.origin.length())
        .fold(0.0, f32::max);
    // Its fastest is 443 a second, with 16 units to start from, so 150 at 0.3 seconds.
    assert!(furthest > 190.0, "{furthest}");
}

/// A spawn's blob is two colours, and half of it stops going sideways.
#[test]
fn a_blob_explosion_has_two_colours_and_settles() {
    let mut effects = numbered();
    effects.start(&temp(4, Vec3::ZERO));
    assert_eq!(count(&effects), 1024);
    let colors = palette_numbers(&effects);
    let warm = colors.iter().filter(|(_, color)| (66..72).contains(color));
    let cool = colors
        .iter()
        .filter(|(_, color)| (150..156).contains(color));
    assert_eq!(warm.count() + cool.count(), 1024, "{colors:?}");
    run(&mut effects, 0.0, 0.9);
    let (draws, _) = effects.authored().draws(&|_| [1.0; 3]);
    let flat = |draw: &&engine_client::EffectDraw| draw.origin.x.hypot(draw.origin.y);
    let near = draws.iter().filter(|draw| flat(draw) < 100.0).count();
    let far = draws.iter().filter(|draw| flat(draw) > 300.0).count();
    assert!(near > 300, "{near}");
    assert!(far > 300, "{far}");
    run(&mut effects, 0.9, 1.5);
    assert_eq!(count(&effects), 0, "all gone by then");
}

/// A rocket's fire rises and goes down the ramp to smoke, and a grenade's starts further on.
#[test]
fn trails_of_fire_rise_and_turn_to_smoke() {
    let fire = [0x6d, 0x6b, 6, 5, 4, 3];
    let trail_of = |model_flags: u32| {
        let mut effects = numbered();
        let mut source = EffectSource {
            id: 5,
            model_flags,
            ..EffectSource::default()
        };
        effects.follow(&[source]);
        source.origin = Vec3::new(600.0, 0.0, 0.0);
        effects.follow(&[source]);
        effects
    };
    let mut rocket = trail_of(1);
    let colors = palette_numbers(&rocket);
    assert!(
        colors.iter().all(|(_, color)| fire[..4].contains(color)),
        "{colors:?}"
    );
    assert!(colors.iter().any(|(_, color)| *color == fire[0]));
    let grenade = trail_of(2);
    let colors = palette_numbers(&grenade);
    assert!(
        colors.iter().all(|(_, color)| fire[2..].contains(color)),
        "{colors:?}"
    );
    let before: f32 = palette_numbers(&rocket).iter().map(|(at, _)| at.z).sum();
    let many = palette_numbers(&rocket).len();
    run(&mut rocket, 0.0, 0.5);
    let after = palette_numbers(&rocket);
    assert!(!after.is_empty() && after.len() <= many);
    let rose: f32 = after.iter().map(|(at, _)| at.z).sum::<f32>() / after.len() as f32;
    assert!(rose > before / many as f32 + 1.0, "{rose}");
    run(&mut rocket, 0.5, 1.3);
    assert_eq!(count(&rocket), 0, "and it's gone at the end of its ramp");
}

/// Blood falls and stays two seconds.
#[test]
fn a_gib_leaves_blood_that_falls() {
    let mut effects = numbered();
    let mut gib = EffectSource {
        id: 5,
        model_flags: 4,
        ..EffectSource::default()
    };
    effects.follow(&[gib]);
    gib.origin = Vec3::new(60.0, 0.0, 0.0);
    effects.follow(&[gib]);
    let colors = palette_numbers(&effects);
    assert!(
        colors.iter().all(|(_, color)| (67..71).contains(color)),
        "{colors:?}"
    );
    run(&mut effects, 0.0, 1.0);
    let lowest = palette_numbers(&effects)
        .iter()
        .map(|(at, _)| at.z)
        .fold(0.0, f32::min);
    assert!(lowest < -10.0, "{lowest}");
    assert!(count(&effects) > 0);
    run(&mut effects, 1.0, 2.1);
    assert_eq!(count(&effects), 0);
}

/// A tracer goes out to one side and the other, and a vore's trail hangs, in their colours.
#[test]
fn spit_goes_to_each_side() {
    let mut effects = numbered();
    let mut spit = EffectSource {
        id: 5,
        model_flags: 16,
        ..EffectSource::default()
    };
    effects.follow(&[spit]);
    spit.origin = Vec3::new(30.0, 0.0, 0.0);
    effects.follow(&[spit]);
    let colors = palette_numbers(&effects);
    assert!(
        colors.iter().all(|(_, color)| [52, 60].contains(color)),
        "{colors:?}"
    );
    run(&mut effects, 0.0, 0.25);
    let spots = palette_numbers(&effects);
    assert!(spots.iter().any(|(at, _)| at.y > 5.0), "{spots:?}");
    assert!(spots.iter().any(|(at, _)| at.y < -5.0), "{spots:?}");
    run(&mut effects, 0.25, 0.6);
    assert_eq!(count(&effects), 0);
}

/// Sparks off a small fire last a second, whatever their ramp would give them.
#[test]
fn a_spark_rises_for_a_second() {
    let mut effects = numbered();
    effects.spark(Vec3::ZERO);
    let start = palette_numbers(&effects)[0].0;
    run(&mut effects, 0.0, 0.8);
    let (place, color) = palette_numbers(&effects)[0];
    assert!(place.z > start.z + 15.0, "{place:?}");
    assert_eq!(color, 4, "most of the way down the ramp");
    run(&mut effects, 0.8, 1.05);
    assert_eq!(count(&effects), 0);
}

/// The most goes up with what the fixture's explosions need, and an author's budget doesn't hold
/// them back.
#[test]
fn a_budget_only_speaks_for_what_an_author_made() {
    let mut effects = Effects::default();
    effects.authored_mut().set_budget(10);
    effects.start(&temp(3, Vec3::ZERO));
    assert_eq!(count(&effects), 1024);
    for _ in 0..40 {
        effects.start(&temp(3, Vec3::ZERO));
    }
    assert_eq!(count(&effects), engine_client::MAX_EFFECT_PARTICLES);
}

/// Gravity follows the world's.
#[test]
fn what_falls_goes_by_the_worlds_gravity() {
    let fall = |gravity: f32| {
        let mut effects = numbered();
        effects.set_gravity(gravity);
        let mut gib = EffectSource {
            id: 5,
            model_flags: 4,
            ..EffectSource::default()
        };
        effects.follow(&[gib]);
        gib.origin = Vec3::new(60.0, 0.0, 0.0);
        effects.follow(&[gib]);
        run(&mut effects, 0.0, 1.0);
        palette_numbers(&effects)
            .iter()
            .map(|(at, _)| at.z)
            .fold(0.0, f32::min)
    };
    let (usual, heavy) = (fall(800.0), fall(1600.0));
    assert!(heavy < usual - 10.0, "{usual} and {heavy}");
}

/// An outline stands still for its two seconds, on the edges of its box.
#[test]
fn an_outline_marks_the_edges_of_a_box() {
    let mut effects = numbered();
    effects.show_seen_box(Vec3::ZERO);
    let spots = palette_numbers(&effects);
    assert_eq!(spots.len(), 12 * 7);
    assert!(spots.iter().all(|(_, color)| *color == 56), "{spots:?}");
    run(&mut effects, 0.0, 1.9);
    assert_eq!(
        palette_numbers(&effects)[0].0,
        spots[0].0,
        "it doesn't move"
    );
    run(&mut effects, 1.9, 2.1);
    assert_eq!(count(&effects), 0);
}

/// Colours come from the palette the level has.
#[test]
fn a_palette_says_what_colours_they_are() {
    let mut effects = Effects::default();
    effects.set_palette(&|_| [0, 255, 0]);
    effects.start(&temp(2, Vec3::ZERO));
    let (draws, _) = effects.authored().draws(&|_| [1.0; 3]);
    assert!(!draws.is_empty());
    assert!(
        draws.iter().all(|draw| draw.color == [0.0, 1.0, 0.0, 1.0]),
        "{draws:?}"
    );
}

/// What hits something is told of, and what doesn't isn't.
#[test]
fn hits_are_kept_for_what_wants_them() {
    let mut effects = Effects::default();
    effects.start(&temp(0, Vec3::new(1.0, 2.0, 3.0)));
    effects.start(&temp(3, Vec3::new(4.0, 5.0, 6.0)));
    effects.start(&Effect::Particles {
        origin: Vec3::new(7.0, 8.0, 9.0),
        direction: Vec3::ZERO,
        color: 73,
        count: 3,
    });
    effects.start(&temp(11, Vec3::new(0.0, 0.0, 1.0)));
    assert_eq!(
        effects.take_impacts(),
        [
            Vec3::new(1.0, 2.0, 3.0),
            Vec3::new(4.0, 5.0, 6.0),
            Vec3::new(7.0, 8.0, 9.0)
        ]
    );
    assert!(effects.take_impacts().is_empty(), "and only the once");
}
