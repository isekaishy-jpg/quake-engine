//! Reading fog in places.

use engine_compile::{compile, read_map};
use engine_core::math::Vec3;
use engine_format::from_text;
use engine_map::{FogVolume, Texture};

/// What a level with `block` in it says.
fn volumes(block: &str) -> Vec<FogVolume> {
    let text = format!(
        "format 1 map
entity world {{ type = worldspawn }}
brush {{ material = rock cuboid = -64 -64 -96 64 64 -64 }}
{block}
"
    );
    let working = read_map(&from_text(&text).expect("parses")).expect("reads");
    let map = compile(&working, &mut |_: &str| Texture::default()).expect("compiles");
    map.fog_volumes
}

#[test]
fn a_volume_has_what_it_says() {
    let found = volumes(
        "fog_volume {
    low = -1 -2 -3
    high = 4 5 6
    color = 0.1 0.2 0.3
    density = 0.5
    edge = 7
    wisps = 0.25
    wisp_size = 30
    drift = 8 9 10
    lit = true
}",
    );
    let [volume] = found[..] else {
        panic!("one of them");
    };
    assert_eq!(volume.low, Vec3::new(-1.0, -2.0, -3.0));
    assert_eq!(volume.high, Vec3::new(4.0, 5.0, 6.0));
    assert_eq!(volume.color, [0.1, 0.2, 0.3]);
    assert_eq!(volume.density, 0.5);
    assert_eq!(volume.edge, 7.0);
    assert_eq!(volume.wisps, 0.25);
    assert_eq!(volume.wisp_size, 30.0);
    assert_eq!(volume.drift, Vec3::new(8.0, 9.0, 10.0));
    assert!(volume.lit);
}

/// Corners can go either way round, and a level can have as many as it wants.
#[test]
fn corners_can_be_either_way_round() {
    let found = volumes(
        "fog_volume { low = 4 5 6 high = -1 -2 -3 }
fog_volume { low = 0 0 0 high = 1 1 1 }",
    );
    assert_eq!(found.len(), 2);
    assert_eq!(found[0].low, Vec3::new(-1.0, -2.0, -3.0));
    assert_eq!(found[0].high, Vec3::new(4.0, 5.0, 6.0));
}

/// Wisps go from none to all.
#[test]
fn wisps_are_a_share() {
    let found = volumes("fog_volume { low = 0 0 0 high = 1 1 1 wisps = 3 }");
    assert_eq!(found[0].wisps, 1.0);
}

/// Only what changes along a ray has to be gone through in steps.
#[test]
fn even_fog_with_hard_sides_is_a_sum() {
    let plain = volumes("fog_volume { low = 0 0 0 high = 1 1 1 edge = 0 }");
    assert!(!plain[0].marched());
    for field in ["edge = 8", "edge = 0 wisps = 0.5", "edge = 0 lit = true"] {
        let block = format!("fog_volume {{ low = 0 0 0 high = 1 1 1 {field} }}");
        assert!(volumes(&block)[0].marched(), "{field}");
    }
}

/// A box needs its corners.
#[test]
fn a_volume_says_where_it_is() {
    let text = "format 1 map
fog_volume { high = 1 1 1 }
";
    let told = read_map(&from_text(text).expect("parses")).expect_err("turned down");
    assert!(told.to_string().contains("low"), "{told}");
}
