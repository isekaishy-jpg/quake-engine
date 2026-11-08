//! A normal map's mips, which keep how much the bumps under each texel agree.

use engine_map::{normal_chain, normal_of_texel, texel_of_normals};

/// A normal map `width` by `height`, each texel from `texel`.
fn picture(width: u32, height: u32, texel: impl Fn(u32, u32) -> [u8; 4]) -> Vec<u8> {
    (0..height)
        .flat_map(|y| (0..width).map(move |x| (x, y)))
        .flat_map(|(x, y)| texel(x, y))
        .collect()
}

/// A normal map that faces straight out keeps that, and that its bumps all agree, all the way
/// down.
#[test]
fn a_flat_normal_map_stays_flat() {
    let chain = normal_chain(8, 4, &picture(8, 4, |_, _| [128, 128, 200, 255]));
    let sizes: Vec<_> = chain
        .iter()
        .map(|level| (level.width, level.height))
        .collect();
    assert_eq!(sizes, [(8, 4), (4, 2), (2, 1), (1, 1)]);
    for level in &chain {
        for texel in level.rgba.as_chunks::<4>().0 {
            assert_eq!(*texel, [128, 128, 255, 255]);
        }
    }
}

/// Bumps leaning two ways in turn average to facing out, and say they spread: the average of two
/// units leaning half each way is 0.87 long.
#[test]
fn bumps_that_spread_say_so_further_down() {
    let leaning = |x: u32, _| {
        if x.is_multiple_of(2) {
            [191, 128, 0, 255]
        } else {
            [64, 128, 0, 255]
        }
    };
    let chain = normal_chain(4, 4, &picture(4, 4, leaning));
    assert_eq!(
        chain[0].rgba[..4],
        [191, 128, 255, 255],
        "the first level as it was"
    );
    let below = &chain[1].rgba;
    assert!(below[0].abs_diff(128) <= 1 && below[1] == 128, "{below:?}");
    // They lean as far each way, so the average is as long as how far each rises.
    let length = normal_of_texel(191, 128)[2] * 255.0;
    assert!(
        (f32::from(below[2]) - length).abs() <= 1.0,
        "{} {length}",
        below[2]
    );
    assert!(below[2] < 230, "{}", below[2]);
}

/// How much they agree is of every texel of the first level under one, not only the four just
/// above: two levels down, four ways of leaning are as spread as they were.
#[test]
fn how_much_bumps_agree_comes_from_the_first_level() {
    let four_ways = |x: u32, y: u32| match (x % 2, y % 2) {
        (0, 0) => [191, 128, 0, 0],
        (1, 0) => [64, 128, 0, 100],
        (0, _) => [128, 191, 0, 100],
        _ => [128, 64, 0, 108],
    };
    let chain = normal_chain(4, 4, &picture(4, 4, four_ways));
    let one = chain[1].rgba[2];
    let two = chain[2].rgba[2];
    assert!(one.abs_diff(two) <= 1, "{one} {two}");
    assert!(one < 230, "{one}");
    assert_eq!(chain[2].rgba[3], 77, "alpha is averaged");
}

/// An odd last column is left out, as halving goes.
#[test]
fn an_odd_last_column_is_left_out() {
    let edge = |x: u32, _| {
        if x == 2 {
            [64, 128, 0, 255]
        } else {
            [191, 128, 0, 255]
        }
    };
    let chain = normal_chain(3, 2, &picture(3, 2, edge));
    assert_eq!((chain[1].width, chain[1].height), (1, 1));
    assert_eq!(
        chain[1].rgba[2], 255,
        "the first two agree, and the third is left out"
    );
}

/// A texel says which way its normals face on average, and how long the average is.
#[test]
fn a_texel_is_its_normals_direction_and_how_much_they_agree() {
    assert_eq!(texel_of_normals([0.0, 0.0, 1.0], 9), [128, 128, 255, 9]);
    assert_eq!(texel_of_normals([0.0, 0.0, 0.5], 9), [128, 128, 128, 9]);
    assert_eq!(
        texel_of_normals([0.0; 3], 9),
        [128, 128, 0, 9],
        "none at all"
    );
    let [x, y, z] = normal_of_texel(128, 128);
    assert!(x.abs() < 0.01 && y.abs() < 0.01 && (z - 1.0).abs() < 0.001);
}
