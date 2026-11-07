//! What a level says of its sky.

use engine_compile::{CompileError, compile, read_map};
use engine_format::from_text;
use engine_map::{Look, Map, Sky, Texture, ToneCurve, USUAL_CLOUD_CURVE};

/// A level with `text` in its sky.
fn level(text: &str) -> Result<Map, CompileError> {
    let text = format!(
        "format 1 map
entity world {{ type = worldspawn }}
sky {{ {text} }}
brush {{ material = rock cuboid = -64 -64 -96 64 64 -64 }}
"
    );
    let working = read_map(&from_text(&text).expect("parses"))?;
    compile(&working, &mut |_: &str| Texture::default())
}

/// The sky of a level with `text` in it.
fn sky(text: &str) -> Sky {
    level(text).expect("compiles").sky.expect("has a sky")
}

/// How a level with `world` in its world block brings light down.
fn tone_of(world: &str) -> Result<ToneCurve, CompileError> {
    let text = format!(
        "format 1 map
entity world {{ type = worldspawn {world} }}
brush {{ material = rock cuboid = -64 -64 -96 64 64 -64 }}
"
    );
    let working = read_map(&from_text(&text).expect("parses"))?;
    let map = compile(&working, &mut |_: &str| Texture::default())?;
    Ok(map.look.tone)
}

/// A level rolls off unless it says otherwise.
#[test]
fn a_level_says_how_light_comes_down() {
    assert_eq!(tone_of(""), Ok(ToneCurve::Neutral));
    assert_eq!(tone_of("tone = neutral"), Ok(ToneCurve::Neutral));
    assert_eq!(tone_of("tone = film"), Ok(ToneCurve::Film));
    assert_eq!(tone_of("tone = hold"), Ok(ToneCurve::Hold));
    let error = tone_of("tone = blue").expect_err("no such curve");
    assert!(error.to_string().contains("blue"), "{error}");
}

/// What's carried over from the fixture, with no level to say, looks as it always did.
#[test]
fn a_map_with_no_say_holds() {
    assert_eq!(Look::default().tone, ToneCurve::Hold);
}

/// A sky can have a moon.
#[test]
fn a_sky_can_have_a_moon() {
    let moon = sky("moon_color = 0.2 0.3 0.4 moon_size = 0.003");
    assert_eq!(moon.moon_color, [0.2, 0.3, 0.4]);
    assert!((moon.moon_size - 0.003).abs() < 0.0001);
    assert_eq!(sky("").moon_color, [0.0; 3]);
}

/// A sky needn't have clouds.
#[test]
fn a_sky_can_be_clear() {
    assert!(sky("zenith = 0 0 1").clouds.is_empty());
}

/// A layer says nothing and gets what's usual, which stays with the view.
#[test]
fn a_layer_has_usual_values() {
    let clouds = sky("clouds { }").clouds;
    assert_eq!(clouds.len(), 1);
    assert!((clouds[0].curve - USUAL_CLOUD_CURVE).abs() < 0.0001);
    assert!((clouds[0].height).abs() < 0.0001);
    assert!((clouds[0].lit).abs() < 0.0001);
    assert!((clouds[0].coverage - 0.5).abs() < 0.0001);
}

/// Layers keep their order, and each has its own values.
#[test]
fn layers_are_read_in_order() {
    let clouds = sky(
        "clouds { coverage = 0.2 height = 3000 curve = 0.5 lit = 0.25 scale = 2 }
        clouds { coverage = 0.8 color = 1 0 0 speed = 0.5 0.25 }",
    )
    .clouds;
    assert_eq!(clouds.len(), 2);
    assert!((clouds[0].coverage - 0.2).abs() < 0.0001);
    assert!((clouds[0].height - 3000.0).abs() < 0.0001);
    assert!((clouds[0].curve - 0.5).abs() < 0.0001);
    assert!((clouds[0].lit - 0.25).abs() < 0.0001);
    assert!((clouds[0].scale - 2.0).abs() < 0.0001);
    assert!((clouds[1].coverage - 0.8).abs() < 0.0001);
    assert_eq!(clouds[1].color, [1.0, 0.0, 0.0]);
    assert_eq!(clouds[1].speed, [0.5, 0.25]);
}

/// Speed is two numbers, and older levels gave it three.
#[test]
fn speed_takes_two_or_three_numbers() {
    let clouds = sky("clouds { speed = 0.5 0.25 } clouds { speed = 0.75 0.125 0 }").clouds;
    assert_eq!(clouds[0].speed, [0.5, 0.25]);
    assert_eq!(clouds[1].speed, [0.75, 0.125]);
}

/// What's out of range is brought into it.
#[test]
fn a_layer_is_kept_in_range() {
    let clouds = sky("clouds { height = -5 curve = 3 lit = 2 }").clouds;
    assert!(clouds[0].height.abs() < 0.0001);
    assert!((clouds[0].curve - 1.0).abs() < 0.0001);
    assert!((clouds[0].lit - 1.0).abs() < 0.0001);
}

/// Each layer's picture goes with that layer.
#[test]
fn a_layer_keeps_its_own_picture() {
    let clouds = sky("clouds { } clouds { image = wisps } clouds { } clouds { }").clouds;
    assert!(clouds[0].image.is_none());
    assert!(clouds[1].image.is_some());
    assert!(clouds[2].image.is_none());
    assert!(clouds[3].image.is_none());
}

/// Rays are for an author to ask for.
#[test]
fn a_sky_has_no_rays_unless_asked() {
    assert!(sky("clouds { }").rays.is_none());
}

/// Rays by the clouds have no picture, and stay in the sky unless told.
#[test]
fn rays_can_go_by_the_clouds() {
    let rays = sky("rays = clouds clouds { }").rays.expect("rays");
    assert!(rays.picture.is_none());
    assert!(!rays.into_level);
    assert!((rays.strength - 1.0).abs() < 0.0001);
}

/// Anything else is a picture, and needs no clouds.
#[test]
fn rays_can_go_by_a_picture() {
    let rays = sky(
        "rays = shafts rays_size = 512 rays_speed = 0.5 0.25 rays_strength = 2 rays_flare = 0.5",
    )
    .rays
    .expect("rays");
    assert!(rays.picture.is_some());
    assert!((rays.size - 512.0).abs() < 0.0001);
    assert!((rays.flare - 0.5).abs() < 0.0001);
    assert_eq!(rays.speed, [0.5, 0.25]);
    assert!((rays.strength - 2.0).abs() < 0.0001);
}

/// They come into the level if an author says so.
#[test]
fn rays_can_reach_the_level() {
    let reach = |text: &str| sky(text).rays.expect("rays").into_level;
    assert!(reach("rays = shafts rays_reach = level"));
    assert!(!reach("rays = shafts rays_reach = sky"));
}

/// What can't mean anything is the author's to hear about.
#[test]
fn rays_that_make_no_sense_are_errors() {
    let error = level("rays = clouds").expect_err("no clouds");
    assert!(error.to_string().contains("clouds"), "{error}");
    let error = level("rays = shafts rays_reach = moon").expect_err("no such reach");
    assert!(error.to_string().contains("moon"), "{error}");
}

/// What's out of range is brought into it.
#[test]
fn rays_are_kept_in_range() {
    let rays = sky("rays = shafts rays_strength = -1 rays_size = 0 rays_flare = -1")
        .rays
        .expect("rays");
    assert!(rays.strength.abs() < 0.0001);
    assert!((rays.size - 1.0).abs() < 0.0001);
    assert!(rays.flare.abs() < 0.0001);
}

/// Four is the most, and more is the author's to hear about.
#[test]
fn too_many_layers_is_an_error() {
    let four = "clouds { } clouds { } clouds { } clouds { }";
    assert_eq!(sky(four).clouds.len(), 4);
    let error = level(&format!("{four} clouds {{ }}")).expect_err("too many");
    assert!(error.to_string().contains("clouds"), "{error}");
}
