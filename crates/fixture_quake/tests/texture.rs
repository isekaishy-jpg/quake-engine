//! Quake's texture rules.

use engine_map::TextureKind;
use fixture_quake::{LIGHT_STYLE_COUNT, Palette, light_styles, texture_kind, without_stray_tail};

/// A BSP header of one 4 byte lump right after the header, then `tail`.
fn bsp_with_tail(tail: &[u8]) -> Vec<u8> {
    let header = 4 + 15 * 8;
    let mut bytes = vec![0; header];
    bytes[4..8].copy_from_slice(&u32::try_from(header).expect("small").to_le_bytes());
    bytes[8..12].copy_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&[1, 2, 3, 4]);
    bytes.extend_from_slice(tail);
    bytes
}

/// `r1m1` from Dissolution of Eternity has stray bytes after its last lump.
#[test]
fn stray_bytes_after_the_lumps_are_removed() {
    let bytes = bsp_with_tail(&[51, 96, 206, 17, 9, 9]);
    assert_eq!(without_stray_tail(&bytes).len(), bytes.len() - 6);
}

#[test]
fn a_bspx_section_stays() {
    let bytes = bsp_with_tail(b"BSPX\0\0\0\0");
    assert_eq!(without_stray_tail(&bytes).len(), bytes.len());
}

#[test]
fn a_file_too_short_for_a_header_is_left_for_the_parser() {
    assert_eq!(without_stray_tail(&[1, 2, 3]), [1, 2, 3]);
}

/// `gl_model.c` and later tools choose behaviour by name.
#[test]
fn names_choose_the_kind() {
    assert_eq!(texture_kind("sky4"), TextureKind::Sky);
    assert_eq!(texture_kind("*water0"), TextureKind::Liquid);
    assert_eq!(texture_kind("{grate"), TextureKind::Cutout);
    assert_eq!(texture_kind("clip"), TextureKind::Hidden);
    assert_eq!(texture_kind("TRIGGER"), TextureKind::Hidden);
    assert_eq!(texture_kind("wall5_4"), TextureKind::Solid);
    assert_eq!(texture_kind("+0button"), TextureKind::Solid);
}

#[test]
fn a_palette_is_exactly_768_bytes() {
    assert!(Palette::parse(&[0; 768]).is_some());
    assert!(Palette::parse(&[0; 767]).is_none());
}

/// `world.qc`: style 0 never changes, and `m` is normal brightness.
#[test]
fn style_zero_is_steady_at_normal() {
    for tick in 0..100 {
        assert!((light_styles(f64::from(tick) * 0.1)[0] - 1.0).abs() < f32::EPSILON);
    }
}

/// Style 1 is `mmnmmommommnonmmonqnmmo`, at ten letters a second.
#[test]
fn styles_step_ten_times_a_second() {
    let dark = light_styles(0.0)[1];
    let bright = light_styles(0.25)[1];
    assert!((dark - 1.0).abs() < f32::EPSILON);
    assert!((bright - 13.0 / 12.0).abs() < 1e-6, "the third letter is n");
}

#[test]
fn styles_past_the_table_stay_normal() {
    let styles = light_styles(3.3);
    assert_eq!(styles.len(), LIGHT_STYLE_COUNT);
    assert!((styles[40] - 1.0).abs() < f32::EPSILON);
}
