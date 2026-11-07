//! What a level says of its sky.

use engine_compile::{CompileError, compile, read_map};
use engine_format::from_text;
use engine_map::{Map, Sky, Texture, USUAL_CLOUD_CURVE};

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

/// Four is the most, and more is the author's to hear about.
#[test]
fn too_many_layers_is_an_error() {
    let four = "clouds { } clouds { } clouds { } clouds { }";
    assert_eq!(sky(four).clouds.len(), 4);
    let error = level(&format!("{four} clouds {{ }}")).expect_err("too many");
    assert!(error.to_string().contains("clouds"), "{error}");
}
