//! Reading sprite files.

use engine_map::SpriteFacing;
use fixture_quake::{Palette, parse_spr};

/// A palette where every colour is the same.
fn palette() -> Palette {
    Palette::parse(&[9; 768]).expect("768 bytes")
}

/// A sprite file's header, for `frames` frames.
fn header(kind: i32, frames: i32) -> Vec<u8> {
    let mut bytes = b"IDSP".to_vec();
    for value in [1_i32, kind] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&1.0_f32.to_le_bytes());
    for value in [2_i32, 2, frames] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&0.0_f32.to_le_bytes());
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    bytes
}

/// A two by one picture with a hole on the right.
fn image(bytes: &mut Vec<u8>) {
    for value in [-1_i32, 1, 2, 1] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    bytes.extend_from_slice(&[7, 255]);
}

#[test]
fn single_frames_keep_their_place_and_holes() {
    let mut bytes = header(2, 1);
    bytes.extend_from_slice(&0_i32.to_le_bytes());
    image(&mut bytes);
    let sprite = parse_spr(&bytes, &palette()).expect("a sprite");
    assert_eq!(sprite.facing, SpriteFacing::View);
    assert_eq!(sprite.frames.len(), 1);
    let image = &sprite.images[0];
    assert_eq!((image.left, image.top), (-1, 1));
    assert_eq!((image.width, image.height), (2, 1));
    assert_eq!(image.rgba, [9, 9, 9, 255, 9, 9, 9, 0]);
}

#[test]
fn groups_cycle_through_their_pictures() {
    let mut bytes = header(0, 1);
    bytes.extend_from_slice(&1_i32.to_le_bytes());
    bytes.extend_from_slice(&2_i32.to_le_bytes());
    for end in [0.1_f32, 0.3] {
        bytes.extend_from_slice(&end.to_le_bytes());
    }
    image(&mut bytes);
    image(&mut bytes);
    let sprite = parse_spr(&bytes, &palette()).expect("a sprite");
    assert_eq!(sprite.images.len(), 2);
    let frame = &sprite.frames[0];
    assert_eq!(frame.image_at(0.05), 0);
    assert_eq!(frame.image_at(0.2), 1);
    assert_eq!(frame.image_at(0.35), 0);
}

#[test]
fn other_files_are_refused() {
    assert!(parse_spr(b"nothing like a sprite", &palette()).is_none());
    let mut short = header(2, 1);
    short.extend_from_slice(&0_i32.to_le_bytes());
    assert!(parse_spr(&short, &palette()).is_none());
}
