//! Finding what a ray might touch.

use engine_compile::RayGrid;

/// What's asked about along a line.
fn seen(grid: &RayGrid, a: [f64; 3], b: [f64; 3]) -> Vec<usize> {
    let mut found = Vec::new();
    grid.any_near(a, b, |index| {
        found.push(index);
        false
    });
    found.sort_unstable();
    found
}

/// A line only hears about what's near it.
#[test]
fn a_ray_passes_what_is_far_away() {
    let grid = RayGrid::new(
        [
            ([0.0, 0.0, 0.0], [64.0, 64.0, 64.0]),
            ([9000.0, 9000.0, 0.0], [9064.0, 9064.0, 64.0]),
        ]
        .into_iter(),
    );
    assert_eq!(seen(&grid, [10.0, 10.0, 0.0], [90.0, 90.0, 0.0]), [0]);
    assert_eq!(
        seen(&grid, [9000.0, 9000.0, 0.0], [9001.0, 9001.0, 9.0]),
        [1]
    );
}

/// What stands across many cells is still only asked about once, however the
/// line crosses it, as glass tints what passes through it only once.
#[test]
fn something_wide_is_asked_about_once() {
    let wide = ([-3000.0, -3000.0, 0.0], [3000.0, 3000.0, 64.0]);
    let grid = RayGrid::new([wide].into_iter());
    assert_eq!(seen(&grid, [-5000.0, 0.0, 0.0], [5000.0, 0.0, 0.0]), [0]);
    assert_eq!(seen(&grid, [0.0, 0.0, 0.0], [10.0, 10.0, 0.0]), [0]);
    assert_eq!(
        seen(&grid, [2500.0, 2500.0, 0.0], [-2500.0, -2500.0, 0.0]),
        [0]
    );
    assert_eq!(
        seen(&grid, [-2500.0, 2500.0, 0.0], [2500.0, -2500.0, 0.0]),
        [0]
    );
}

/// Asking stops at the first that says so.
#[test]
fn asking_stops_when_it_is_answered() {
    let square = ([0.0, 0.0, 0.0], [64.0, 64.0, 64.0]);
    let grid = RayGrid::new([square, square, square].into_iter());
    let mut asked = 0;
    let found = grid.any_near([0.0; 3], [1.0; 3], |_| {
        asked += 1;
        true
    });
    assert!(found);
    assert_eq!(asked, 1);
}

/// With nothing in it, nothing's near.
#[test]
fn an_empty_grid_has_nothing_near() {
    let grid = RayGrid::new(std::iter::empty());
    assert!(!grid.any_near([0.0; 3], [9000.0; 3], |_| true));
}
