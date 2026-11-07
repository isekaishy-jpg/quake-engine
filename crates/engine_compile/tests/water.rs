//! What a level says of its water.

use engine_compile::{CompileError, compile, read_map};
use engine_core::entity::effect_id;
use engine_format::from_text;
use engine_map::{Map, Texture};

/// A level with `text` in it.
fn level(text: &str) -> Result<Map, CompileError> {
    let text = format!(
        "format 1 map
entity world {{ type = worldspawn }}
brush {{ material = rock cuboid = -64 -64 -96 64 64 -64 }}
{text}
"
    );
    let working = read_map(&from_text(&text).expect("parses"))?;
    compile(&working, &mut |_: &str| Texture::default())
}

/// Each body has the look it says, and those that don't say have the one with no name.
#[test]
fn a_brush_says_which_water_it_is() {
    let map = level(
        "water { absorption = 0.1 }
water canal { absorption = 0.2 deep = 0.5 view_effect = motes }
water pond { absorption = 0.3 }
brush { material = rock contents = water water = pond cuboid = 0 0 0 10 10 10 }
brush { material = rock contents = water cuboid = 20 0 0 30 10 10 }
brush { material = rock contents = water water = canal cuboid = 40 0 0 50 10 10 }",
    )
    .expect("compiles");
    let looks: Vec<f32> = map.pools.iter().map(|pool| pool.look.absorption).collect();
    assert_eq!(looks, [0.3, 0.1, 0.2]);
    assert_eq!(map.pools[2].look.deep, 0.5);
    assert_eq!(map.pools[2].look.view_effect, effect_id("motes"));
    assert_eq!(map.pools[0].look.view_effect, 0);
}

/// A name nobody gave is a mistake.
#[test]
fn a_water_has_to_be_there() {
    let told =
        level("brush { material = rock contents = water water = gone cuboid = 0 0 0 9 9 9 }");
    let told = told.expect_err("turned down").to_string();
    assert!(told.contains("gone"), "{told}");
}

/// With no water that fits, it's what such a liquid usually is.
#[test]
fn a_liquid_with_nothing_said_has_its_usual_look() {
    let map = level(
        "water canal { absorption = 0.2 }
brush { material = rock contents = slime cuboid = 0 0 0 10 10 10 }",
    )
    .expect("compiles");
    assert_eq!(map.pools[0].look.absorption, 0.02);
}
