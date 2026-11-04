//! Drawing with a real device. Each test skips itself if there's no Vulkan.

mod common;

use std::sync::Arc;

use common::map;
use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_map::{AliasCorner, AliasFrame, AliasModel, AliasVertex};
use engine_render::{Capture, EntityDraw, Renderer, Target, View};

/// A renderer with no window, or `None` with no Vulkan device. Any other
/// failure fails the test, so a broken shader can't pass as a missing device.
fn renderer() -> Option<Renderer> {
    match Renderer::new(Target::Offscreen, (64, 64)) {
        Ok(renderer) => Some(renderer),
        Err(error) if error.is_no_device() => None,
        Err(error) => panic!("{error}"),
    }
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
    renderer.load_map(Arc::new(map(1))).expect("load");
    let mut view = view_from_above();
    view.origin = Vec3::new(-32.0, 64.0, -48.0);
    view.pitch = 0.0;
    let capture = renderer.capture(&view, &[], &[], 64, 64).expect("capture");
    assert!(lit_pixels(&capture, 0..32) > 0, "the top half shows it");
    assert_eq!(lit_pixels(&capture, 32..64), 0, "the bottom half doesn't");
    view.origin.z = 48.0;
    let capture = renderer.capture(&view, &[], &[], 64, 64).expect("capture");
    assert_eq!(lit_pixels(&capture, 0..32), 0, "the top half doesn't");
    assert!(lit_pixels(&capture, 32..64) > 0, "the bottom half shows it");
}

/// With yaw 0 looking along x, y runs to the left of the picture.
#[test]
fn what_is_left_of_the_view_is_at_the_left_of_the_picture() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let mut view = view_from_above();
    view.origin = Vec3::new(-32.0, -96.0, 48.0);
    view.pitch = 0.0;
    let capture = renderer.capture(&view, &[], &[], 64, 64).expect("capture");
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
fn overlay_text_appears_in_the_top_right_corner_only() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let view = view_from_above();
    renderer.set_overlay_text("88");
    let capture = renderer
        .capture(&view, &[], &[], 320, 180)
        .expect("capture");
    let white = |x: std::ops::Range<usize>, y: std::ops::Range<usize>| {
        let pixels = capture.rgba.as_chunks::<4>().0;
        y.flat_map(|row| x.clone().map(move |column| row * 320 + column))
            .filter(|&at| pixels[at][..3] == [255, 255, 255])
            .count()
    };
    assert!(white(280..320, 0..30) > 0, "the corner shows it");
    assert_eq!(white(0..280, 0..180), 0, "the rest doesn't");
    renderer.set_overlay_text("");
    let capture = renderer
        .capture(&view, &[], &[], 320, 180)
        .expect("capture");
    let pixels = capture.rgba.as_chunks::<4>().0;
    assert!(
        pixels.iter().all(|pixel| pixel[..3] != [255, 255, 255]),
        "it goes away"
    );
}

/// A model is drawn with its position added, and not at all unless listed.
#[test]
fn a_brush_model_draws_where_its_instance_is() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let mut two = map(2);
    two.tree.leaves[1].face_count = 0;
    two.tree.leaf_faces.clear();
    renderer.load_map(Arc::new(two)).expect("load");
    let view = view_from_above();
    let lit = |capture: &Capture| centre(capture)[1] > 32;
    let empty = renderer.capture(&view, &[], &[], 64, 64).expect("capture");
    assert!(!lit(&empty), "unlisted");
    let mut instance = ModelInstance {
        model: 1,
        origin: Vec3::ZERO,
        visible: true,
        solid: true,
    };
    let shown = renderer
        .capture(&view, &[instance], &[], 64, 64)
        .expect("capture");
    assert!(lit(&shown), "in place");
    instance.origin = Vec3::new(1000.0, 0.0, 0.0);
    let moved = renderer
        .capture(&view, &[instance], &[], 64, 64)
        .expect("capture");
    assert!(!lit(&moved), "moved away");
    instance.origin = Vec3::ZERO;
    instance.visible = false;
    let hidden = renderer
        .capture(&view, &[instance], &[], 64, 64)
        .expect("capture");
    assert!(!lit(&hidden), "hidden");
}

#[test]
fn a_capture_has_the_size_asked_for() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    let capture = renderer
        .capture(&view_from_above(), &[], &[], 32, 16)
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
    renderer.load_map(Arc::new(map(1))).expect("load");
    let capture = renderer
        .capture(&view_from_above(), &[], &[], 64, 64)
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
    renderer.load_map(Arc::new(map(1))).expect("load");
    let mut view = view_from_above();
    view.styles[0] = 0.0;
    let capture = renderer.capture(&view, &[], &[], 64, 64).expect("capture");
    assert_eq!(centre(&capture)[..3], [0, 0, 0]);
}

#[test]
fn a_view_with_the_surface_behind_shows_the_clear_colour() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let mut view = view_from_above();
    view.pitch = -89.9;
    let capture = renderer.capture(&view, &[], &[], 64, 64).expect("capture");
    assert!(centre(&capture)[0] < 16);
}

/// A flat white triangle in the x-y plane, 400 units across, in one pose.
fn triangle() -> AliasModel {
    let vertex = |x, y| AliasVertex {
        position: [x, y, 0],
        normal: [127, 0, 0],
    };
    let corner = |vertex| AliasCorner {
        vertex,
        uv: [0.5, 0.5],
    };
    AliasModel {
        scale: Vec3::new(4.0, 4.0, 1.0),
        offset: Vec3::new(-200.0, -200.0, 0.0),
        skin_width: 1,
        skin_height: 1,
        skins: vec![vec![200, 200, 200, 255]],
        corners: vec![corner(0), corner(1), corner(2)],
        poses: vec![vec![vertex(0, 0), vertex(100, 0), vertex(50, 100)]],
        frames: vec![AliasFrame {
            first_pose: 0,
            pose_count: 1,
            pose_ends: Vec::new(),
        }],
        spins: false,
        flags: 0,
    }
}

/// A draw of `model` under the view, in neutral light.
fn draw_of(model: u32) -> EntityDraw {
    EntityDraw {
        model,
        origin: Vec3::new(64.0, 64.0, 32.0),
        ambient: 1.0,
        ..EntityDraw::default()
    }
}

#[test]
fn a_model_draws_where_it_is_with_its_light() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let number = renderer.add_model(Arc::new(triangle())).expect("add");
    let lit = renderer
        .capture(&view_from_above(), &[], &[draw_of(number)], 64, 64)
        .expect("capture");
    let [r, ..] = centre(&lit);
    assert!(
        (198..=202).contains(&r),
        "neutral light shows the skin: {r}"
    );
    let dim = EntityDraw {
        ambient: 0.5,
        ..draw_of(number)
    };
    let dark = renderer
        .capture(&view_from_above(), &[], &[dim], 64, 64)
        .expect("capture");
    assert!((98..=102).contains(&centre(&dark)[0]));
    let away = EntityDraw {
        origin: Vec3::new(5000.0, 64.0, 32.0),
        ..draw_of(number)
    };
    let gone = renderer
        .capture(&view_from_above(), &[], &[away], 64, 64)
        .expect("capture");
    assert_ne!(centre(&gone)[0], r);
}

#[test]
fn a_model_kept_in_front_shows_through_what_is_nearer() {
    let Some(mut renderer) = renderer() else {
        return;
    };
    renderer.load_map(Arc::new(map(1))).expect("load");
    let number = renderer.add_model(Arc::new(triangle())).expect("add");
    let below = EntityDraw {
        origin: Vec3::new(64.0, 64.0, -32.0),
        ..draw_of(number)
    };
    let hidden = renderer
        .capture(&view_from_above(), &[], &[below], 64, 64)
        .expect("capture");
    assert_eq!(centre(&hidden)[0], 255, "the map's square is nearer");
    let in_front = EntityDraw {
        in_front: true,
        ..below
    };
    let shown = renderer
        .capture(&view_from_above(), &[], &[in_front], 64, 64)
        .expect("capture");
    assert!((198..=202).contains(&centre(&shown)[0]));
}
