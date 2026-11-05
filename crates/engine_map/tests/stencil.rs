//! Ray stencils: boxes from authored lines, and moving lights.

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{RayStencil, STENCIL_GRID, stencil_boxes};

/// A white picture of `width` by `height`, with red columns at `columns`.
fn picture(width: usize, height: usize, columns: &[usize]) -> Vec<u8> {
    let mut rgba = vec![255; width * height * 4];
    for y in 0..height {
        for x in columns {
            let at = (y * width + x) * 4;
            rgba[at..at + 4].copy_from_slice(&[255, 0, 0, 255]);
        }
    }
    rgba
}

/// A stencil of one unit edges with `boxes`.
fn stencil(boxes: Vec<[f32; 4]>) -> RayStencil {
    RayStencil {
        corner: Vec3::ZERO,
        across: Vec3::new(16.0, 0.0, 0.0),
        down: Vec3::new(0.0, 0.0, -16.0),
        light: Vec3::new(0.0, 64.0, 0.0),
        sun: false,
        path: Vec::new(),
        period: 0.0,
        strength: 1.0,
        reach: 256.0,
        image: Arc::new(Vec::new()),
        boxes,
    }
}

/// With no lines, a stencil is one box.
#[test]
fn no_lines_make_one_box() {
    let boxes = stencil_boxes(8, 8, &picture(8, 8, &[]));
    assert_eq!(boxes, vec![[0.0, 0.0, 1.0, 1.0]]);
}

/// Each area a red line closes off is its own box.
#[test]
fn red_lines_split_boxes() {
    let boxes = stencil_boxes(8, 4, &picture(8, 4, &[3]));
    assert_eq!(boxes, vec![[0.0, 0.0, 0.375, 1.0], [0.5, 0.0, 1.0, 1.0]]);
}

/// Black lines are the frame, not a grouping, so panes stay in one box.
#[test]
fn black_frames_do_not_split_boxes() {
    let mut rgba = picture(8, 8, &[]);
    for y in 0..8 {
        let at = (y * 8 + 4) * 4;
        rgba[at..at + 4].copy_from_slice(&[0, 0, 0, 255]);
    }
    assert_eq!(stencil_boxes(8, 8, &rgba).len(), 1);
}

/// All red leaves the whole stencil as one box, rather than none.
#[test]
fn all_lines_still_leave_a_box() {
    let boxes = stencil_boxes(2, 2, &picture(2, 2, &[0, 1]));
    assert_eq!(boxes, vec![[0.0, 0.0, 1.0, 1.0]]);
}

/// A closed box closes only its own cells.
#[test]
fn a_mask_closes_one_box() {
    let shape = stencil(vec![[0.0, 0.0, 0.5, 1.0], [0.5, 0.0, 1.0, 1.0]]);
    let mut mask = [0.0; STENCIL_GRID * STENCIL_GRID];
    shape.fill_mask(&[0.25, 1.0], &mut mask);
    assert!((mask[0] - 0.25).abs() < 0.001);
    assert!((mask[STENCIL_GRID - 1] - 1.0).abs() < 0.001);
    assert!((mask[STENCIL_GRID * STENCIL_GRID - 1] - 1.0).abs() < 0.001);
}

/// A light with no path stays still, and one with a path goes round it.
#[test]
fn a_light_moves_along_its_path() {
    let mut shape = stencil(Vec::new());
    assert_eq!(shape.light_at(5.0), shape.light);
    shape.path = vec![Vec3::ZERO, Vec3::new(10.0, 0.0, 0.0)];
    shape.period = 2.0;
    assert_eq!(shape.light_at(0.0), Vec3::ZERO);
    assert_eq!(shape.light_at(0.5), Vec3::new(5.0, 0.0, 0.0));
    assert_eq!(shape.light_at(1.0), Vec3::new(10.0, 0.0, 0.0));
    assert_eq!(shape.light_at(2.0), Vec3::ZERO);
}

/// The test point of a box is its middle on the surface.
#[test]
fn a_box_has_a_point_on_the_surface() {
    let shape = stencil(Vec::new());
    assert_eq!(
        shape.point_in([0.0, 0.0, 0.5, 0.5]),
        Vec3::new(4.0, 0.0, -4.0)
    );
}
