//! A cut-out's mips, which stay as solid as it is.

use engine_map::{MipLevel, PixelForm, keep_coverage, mip_chain};

/// A picture `side` texels a side, solid in bars a texel wide every `every` texels, with holes
/// between.
fn bars(side: u32, every: u32) -> Vec<u8> {
    (0..side * side)
        .flat_map(|at| {
            let alpha = if (at % side).is_multiple_of(every) {
                255
            } else {
                0
            };
            [90, 90, 90, alpha]
        })
        .collect()
}

/// The share of `level`'s texels that are solid with `cutoff`.
fn solid(level: &MipLevel, cutoff: f32) -> f32 {
    let texels = level.rgba.as_chunks::<4>().0;
    let count = texels
        .iter()
        .filter(|texel| f32::from(texel[3]) >= cutoff * 255.0)
        .count();
    count as f32 / texels.len() as f32
}

/// Solid in scattered spots, about `share` of it, as leaves are: a picture `side` texels a side.
fn spots(side: u32, share: f32) -> Vec<u8> {
    (0..side * side)
        .flat_map(|at: u32| {
            let mut hashed = at ^ (at >> 16);
            hashed = hashed.wrapping_mul(0x7feb_352d);
            hashed ^= hashed >> 15;
            hashed = hashed.wrapping_mul(0x846c_a68b);
            hashed = (hashed ^ (hashed >> 16)) & 0xffff;
            let alpha = if (hashed as f32) < share * 65536.0 {
                255
            } else {
                0
            };
            [90, 90, 90, alpha]
        })
        .collect()
}

/// Scattered spots thin away as mips are made, and are kept as solid as they were, close enough
/// that leaves far off are as thick as near.
#[test]
fn spots_stay_as_solid_as_they_were() {
    let mut chain = mip_chain(32, 32, spots(32, 0.3));
    let wanted = solid(&chain[0], 0.5);
    assert!(solid(&chain[3], 0.5) < wanted / 2.0, "averaged, they thin");
    let first = chain[0].clone();
    keep_coverage(&mut chain, 0.5);
    assert_eq!(chain[0], first, "the first level is as it was");
    for level in &chain[1..4] {
        let share = solid(level, 0.5);
        assert!(
            (share - wanted).abs() < 0.07,
            "{}: {share} {wanted}",
            level.width
        );
    }
    let colour = |level: &MipLevel| level.rgba[..3].to_vec();
    assert_eq!(colour(&chain[2]), [90, 90, 90], "only alpha changes");
}

/// Thin bars average away to nothing, and are kept, as near as whole texels let them: halved,
/// half the texels are bars at half, so they can be none or half, and as near either way they're
/// half.
#[test]
fn thin_bars_dont_go() {
    let mut chain = mip_chain(16, 16, bars(16, 4));
    assert!((solid(&chain[0], 0.5) - 0.25).abs() < 0.01);
    assert_eq!(solid(&chain[1], 0.5), 0.0, "averaged, they're gone");
    keep_coverage(&mut chain, 0.5);
    assert!(
        (solid(&chain[1], 0.5) - 0.5).abs() < 0.01,
        "{}",
        solid(&chain[1], 0.5)
    );
}

/// With another cutoff, it's that cutoff that's kept to.
#[test]
fn the_cutoff_is_the_materials() {
    let mut chain = mip_chain(32, 32, spots(32, 0.3));
    let wanted = solid(&chain[0], 0.8);
    keep_coverage(&mut chain, 0.8);
    let share = solid(&chain[1], 0.8);
    assert!((share - wanted).abs() < 0.07, "{share} {wanted}");
}

/// Compressed levels can't be changed, and are left as they are.
#[test]
fn compressed_levels_are_left_alone() {
    let mut chain = mip_chain(8, 8, bars(8, 4));
    for level in &mut chain {
        level.form = PixelForm::Bc7Srgb;
    }
    let before = chain.clone();
    keep_coverage(&mut chain, 0.5);
    assert_eq!(chain, before, "none of it can be read");
    let mut mixed = mip_chain(32, 32, spots(32, 0.3));
    mixed[1].form = PixelForm::Bc7Srgb;
    let compressed = mixed[1].clone();
    keep_coverage(&mut mixed, 0.5);
    assert_eq!(mixed[1], compressed, "a compressed level below the first");
    let wanted = solid(&mixed[0], 0.5);
    assert!(
        (solid(&mixed[2], 0.5) - wanted).abs() < 0.07,
        "and the rest kept"
    );
    keep_coverage(&mut [], 0.5);
}
