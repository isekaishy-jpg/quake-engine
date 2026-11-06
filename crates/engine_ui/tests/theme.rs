//! The engine's own look, none of which needs its art here.

use engine_ui::{KitPiece, TextMeasure, UiAnimation, nine_slices, text_width, wrapped};

/// Every character ten wide.
struct Even;

impl TextMeasure for Even {
    fn advance(&self, _: char) -> f32 {
        10.0
    }
}

#[test]
fn text_is_as_wide_as_its_characters() {
    assert_eq!(text_width(&Even, "abcd"), 40.0);
}

#[test]
fn lines_break_between_words() {
    assert_eq!(wrapped(&Even, "one two three", 80.0), ["one two", "three"]);
}

#[test]
fn a_line_break_stays_one() {
    assert_eq!(wrapped(&Even, "one\ntwo", 200.0), ["one", "two"]);
}

#[test]
fn a_word_too_wide_for_a_line_is_broken() {
    assert_eq!(wrapped(&Even, "abcdefgh", 30.0), ["abc", "def", "gh"]);
}

/// A frame 128 square with corners of 12, at one pixel to a unit.
fn panel() -> KitPiece {
    KitPiece {
        rect: [4.0, 4.0, 128.0, 128.0],
        slices: [12.0, 12.0, 12.0, 12.0],
    }
}

#[test]
fn a_frames_corners_keep_their_size() {
    let slices = nine_slices(&panel(), [0.0, 0.0, 600.0, 320.0], 1.0, 1.0);
    assert_eq!(slices.len(), 9);
    let (source, target) = slices[0];
    assert_eq!(source, [4.0, 4.0, 16.0, 16.0]);
    assert_eq!(target, [0.0, 0.0, 12.0, 12.0]);
    let (source, target) = slices[8];
    assert_eq!(source, [120.0, 120.0, 132.0, 132.0]);
    assert_eq!(target, [588.0, 308.0, 600.0, 320.0]);
}

#[test]
fn a_frames_middle_stretches() {
    let slices = nine_slices(&panel(), [0.0, 0.0, 600.0, 320.0], 1.0, 1.0);
    let (source, target) = slices[4];
    assert_eq!(source, [16.0, 16.0, 120.0, 120.0]);
    assert_eq!(target, [12.0, 12.0, 588.0, 308.0]);
}

#[test]
fn corners_follow_the_screens_scale_and_not_the_atlass() {
    let doubled = KitPiece {
        rect: [0.0, 0.0, 256.0, 256.0],
        slices: [24.0, 24.0, 24.0, 24.0],
    };
    let slices = nine_slices(&doubled, [0.0, 0.0, 600.0, 320.0], 2.0, 1.0);
    assert_eq!(slices[0].0, [0.0, 0.0, 24.0, 24.0], "from the 2x atlas");
    assert_eq!(
        slices[0].1,
        [0.0, 0.0, 12.0, 12.0],
        "twelve units on screen"
    );
}

/// Two frames of a tenth of a second.
fn blink(loops: bool) -> UiAnimation {
    UiAnimation {
        frames: vec![("a".to_owned(), 0.1), ("b".to_owned(), 0.1)],
        loops,
    }
}

#[test]
fn a_picture_moves_through_its_frames() {
    let animation = blink(false);
    assert_eq!(animation.frame_at(0.05), Some("a"));
    assert_eq!(animation.frame_at(0.15), Some("b"));
}

#[test]
fn one_shot_stops_after_its_last_frame() {
    assert_eq!(blink(false).frame_at(0.25), None);
}

#[test]
fn a_loop_goes_round() {
    assert_eq!(blink(true).frame_at(0.25), Some("a"));
}
