//! Working out maps from a picture.

use engine_compile::{DeriveSettings, decode_png, derive_maps, encode_png};

/// How a surface is usually worked out.
const SETTINGS: DeriveSettings = DeriveSettings {
    strength: 1.0,
    shine: 0.5,
};

/// A picture of `side` by `side`, grey, from a function of position.
fn picture(side: u32, value: impl Fn(u32, u32) -> u8) -> Vec<u8> {
    let mut out = Vec::new();
    for y in 0..side {
        for x in 0..side {
            let grey = value(x, y);
            out.extend_from_slice(&[grey, grey, grey, 255]);
        }
    }
    out
}

/// The normal at `x` and `y`.
fn normal_at(normal: &[u8], side: u32, x: u32, y: u32) -> [u8; 3] {
    let at = ((y * side + x) * 4) as usize;
    [normal[at], normal[at + 1], normal[at + 2]]
}

/// A flat picture is a flat surface, however bright.
#[test]
fn a_flat_picture_is_flat() {
    let maps = derive_maps(64, 64, &picture(64, |_, _| 180), SETTINGS);
    assert_eq!(normal_at(&maps.normal, 64, 10, 10), [128, 128, 255]);
}

/// The sides of a bright ridge lean away from each other, in the same sense as
/// the normals drawn by `content/generate.py`.
#[test]
fn a_ridge_has_sides_that_lean_apart() {
    let side = 64;
    let bright = |x: u32, _: u32| if (28..36).contains(&x) { 230 } else { 60 };
    let maps = derive_maps(side, side, &picture(side, bright), SETTINGS);
    let rising = normal_at(&maps.normal, side, 27, 30);
    let falling = normal_at(&maps.normal, side, 36, 30);
    assert!(rising[0] < 110, "{rising:?}");
    assert!(falling[0] > 146, "{falling:?}");
    assert!((120..=136).contains(&rising[1]), "{rising:?}");
}

/// What tiles still tiles: a ridge on the edge shows on both sides of the seam.
#[test]
fn slopes_carry_over_the_seam() {
    let side = 64;
    let bright = |x: u32, _: u32| if x < 4 { 230 } else { 60 };
    let maps = derive_maps(side, side, &picture(side, bright), SETTINGS);
    let before = normal_at(&maps.normal, side, side - 1, 30);
    assert!(before[0] < 110, "{before:?}");
}

/// A big soft change of tone isn't shape, so it barely leans.
#[test]
fn broad_tone_is_not_relief() {
    let side = 256;
    let tone = |x: u32, y: u32| {
        let wave = (f64::from(x) / 256.0 * std::f64::consts::TAU).sin();
        let grain = if (x / 8 + y / 8).is_multiple_of(2) {
            6
        } else {
            -6
        };
        #[expect(clippy::cast_possible_truncation, reason = "within 60 of nothing")]
        let level = (wave * 60.0).round() as i32;
        u8::try_from(120 + grain + level).expect("a grey")
    };
    let maps = derive_maps(side, side, &picture(side, tone), SETTINGS);
    // Where the wave is steepest, a slope from tone alone would lean one way.
    let leans: i32 = (0..side)
        .map(|y| i32::from(normal_at(&maps.normal, side, 2, y)[0]) - 128)
        .sum();
    assert!(leans.abs() < 256 * 8, "{leans}");
}

/// What stands proud shines more than what's sunk, and a dull surface doesn't.
#[test]
fn what_stands_proud_shines_more() {
    let side = 64;
    let bright = |x: u32, _: u32| if (20..44).contains(&x) { 200 } else { 60 };
    let image = picture(side, bright);
    let maps = derive_maps(side, side, &image, SETTINGS);
    let at = |x: u32| maps.specular[((10 * side + x) * 4) as usize];
    assert!(at(32) > at(4), "{} {}", at(32), at(4));
    let dull = DeriveSettings {
        shine: 0.0,
        ..SETTINGS
    };
    let maps = derive_maps(side, side, &image, dull);
    assert_eq!(maps.specular[0], 0);
}

/// A map written as a PNG reads back as it was.
#[test]
fn maps_survive_being_written() {
    let image = picture(16, |x, y| ((x * 13 + y * 7) % 251) as u8);
    let bytes = encode_png(16, 16, &image).expect("writes");
    let (width, height, read) = decode_png(&bytes).expect("reads");
    assert_eq!((width, height), (16, 16));
    assert_eq!(read, image);
}
