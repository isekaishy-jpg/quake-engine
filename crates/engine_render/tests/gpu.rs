//! Drawing with a real device. Each test skips itself if there's no Vulkan.

mod common;

use common::map;
use engine_core::math::Vec3;
use engine_render::{Capture, Renderer, Target, View};

/// A renderer with no window, or `None` with no Vulkan device.
fn renderer() -> Option<Renderer> {
    Renderer::new(Target::Offscreen, (64, 64)).ok()
}

/// A view from above the square, looking straight down at its middle.
fn view_from_above() -> View {
    View {
        origin: Vec3::new(64.0, 64.0, 64.0),
        yaw: 0.0,
        pitch: 89.9,
        fov: 90.0,
        time: 0.0,
        styles: [1.0; 64],
    }
}

/// The pixel at the centre of a capture.
fn centre(capture: &Capture) -> [u8; 4] {
    let at = ((capture.height / 2 * capture.width + capture.width / 2) * 4) as usize;
    [
        capture.rgba[at],
        capture.rgba[at + 1],
        capture.rgba[at + 2],
        capture.rgba[at + 3],
    ]
}

/// How many pixels in rows `rows` aren't the clear colour.
fn lit_pixels(capture: &Capture, rows: std::ops::Range<u32>) -> usize {
    let row_bytes = capture.width as usize * 4;
    let bytes = &capture.rgba[rows.start as usize * row_bytes..rows.end as usize * row_bytes];
    bytes
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[1] > 32)
        .count()
}

/// Looking level from below the square's height puts it at the top of the
/// picture, and looking level from above puts it at the bottom. This is the
/// test that catches an upside-down picture.
#[test]
fn what_is_above_the_view_is_at_the_top_of_the_picture() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(map(1)).expect("load");
    let mut view = view_from_above();
    view.origin = Vec3::new(-32.0, 64.0, -48.0);
    view.pitch = 0.0;
    let capture = renderer.capture(&view, 64, 64).expect("capture");
    assert!(lit_pixels(&capture, 0..32) > 0, "the top half shows it");
    assert_eq!(lit_pixels(&capture, 32..64), 0, "the bottom half doesn't");
    view.origin.z = 48.0;
    let capture = renderer.capture(&view, 64, 64).expect("capture");
    assert_eq!(lit_pixels(&capture, 0..32), 0, "the top half doesn't");
    assert!(lit_pixels(&capture, 32..64) > 0, "the bottom half shows it");
}

/// With yaw 0 looking along x, y runs to the left of the picture.
#[test]
fn what_is_left_of_the_view_is_at_the_left_of_the_picture() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(map(1)).expect("load");
    let mut view = view_from_above();
    view.origin = Vec3::new(-32.0, -96.0, 48.0);
    view.pitch = 0.0;
    let capture = renderer.capture(&view, 64, 64).expect("capture");
    let lit_at = |columns: std::ops::Range<usize>| {
        let pixels = capture.rgba.as_chunks::<4>().0;
        pixels
            .chunks(64)
            .flat_map(|row| &row[columns.clone()])
            .filter(|pixel| pixel[1] > 32)
            .count()
    };
    assert!(lit_at(0..32) > 0, "the left half shows it");
    assert_eq!(lit_at(32..64), 0, "the right half doesn't");
}

#[test]
fn a_capture_has_the_size_asked_for() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let capture = renderer
        .capture(&view_from_above(), 32, 16)
        .expect("capture");
    assert_eq!((capture.width, capture.height), (32, 16));
    assert_eq!(capture.rgba.len(), 32 * 16 * 4);
}

/// The sample is 200, 100, 50. Doubled over 255 on a white texture, that's
/// the original's overbright rule.
#[test]
fn a_lit_white_surface_shows_its_doubled_light() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(map(1)).expect("load");
    let capture = renderer
        .capture(&view_from_above(), 64, 64)
        .expect("capture");
    let [r, g, b, _] = centre(&capture);
    assert_eq!(r, 255, "400 of 255 clamps");
    assert!((199..=201).contains(&g), "green {g}");
    assert!((99..=101).contains(&b), "blue {b}");
}

#[test]
fn a_dark_style_darkens_the_surface() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(map(1)).expect("load");
    let mut view = view_from_above();
    view.styles[0] = 0.0;
    let capture = renderer.capture(&view, 64, 64).expect("capture");
    assert_eq!(centre(&capture)[..3], [0, 0, 0]);
}

#[test]
fn a_view_with_the_surface_behind_shows_the_clear_colour() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(map(1)).expect("load");
    let mut view = view_from_above();
    view.pitch = -89.9;
    let capture = renderer.capture(&view, 64, 64).expect("capture");
    assert!(centre(&capture)[0] < 16);
}
