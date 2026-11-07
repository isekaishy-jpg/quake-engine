//! Bodies of liquid of any shape.

use engine_core::math::Vec3;
use engine_map::{Contents, Map, NO_WATER, PoolLook, WaterSheet};

/// A sheet of 3 by 3 samples 10 apart from the origin, rising by 5 each way along x.
fn slope() -> WaterSheet {
    WaterSheet {
        origin: [0.0, 0.0],
        cell: 10.0,
        size: [3, 3],
        heights: vec![0.0, 5.0, 10.0, 0.0, 5.0, 10.0, 0.0, 5.0, 10.0],
        depth: 8.0,
        contents: Contents::Water,
        look: PoolLook::of(Contents::Water).expect("a liquid"),
    }
}

#[test]
fn a_sheet_is_as_high_as_its_samples_between_them() {
    let sheet = slope();
    assert_eq!(sheet.height_at(0.0, 0.0), Some(0.0));
    assert_eq!(sheet.height_at(15.0, 5.0), Some(7.5));
    assert_eq!(sheet.height_at(20.0, 20.0), Some(10.0));
    assert_eq!(sheet.height_at(-1.0, 5.0), None);
    assert_eq!(sheet.height_at(5.0, -1.0), None);
    assert_eq!(sheet.height_at(31.0, 5.0), None);
}

#[test]
fn a_sheet_holds_what_is_under_it_and_not_too_far_under() {
    let sheet = slope();
    assert!(sheet.holds(Vec3::new(15.0, 5.0, 7.0)));
    assert!(
        sheet.holds(Vec3::new(15.0, 5.0, 0.0)),
        "its depth goes by the surface"
    );
    assert!(!sheet.holds(Vec3::new(15.0, 5.0, 8.0)), "over it");
    assert!(!sheet.holds(Vec3::new(15.0, 5.0, -1.0)), "under it");
    assert!(!sheet.holds(Vec3::new(50.0, 5.0, 5.0)), "off it");
}

#[test]
fn where_there_is_no_water_a_sheet_stops() {
    let mut sheet = slope();
    for at in [0, 3, 6] {
        sheet.heights[at] = NO_WATER;
    }
    assert_eq!(sheet.height_at(0.0, 5.0), None);
    assert_eq!(
        sheet.height_at(5.0, 5.0),
        Some(5.0),
        "by those that have it"
    );
    assert_eq!(
        sheet.triangles().len(),
        2 * 2 * 3 / 2 * 2,
        "half of its cells"
    );
    let (low, high) = sheet.bounds();
    assert_eq!(low.z, 5.0 - 8.0);
    assert_eq!(high.z, 10.0);
}

#[test]
fn a_sheet_has_a_box_round_it() {
    let (low, high) = slope().bounds();
    assert_eq!(low, Vec3::new(0.0, 0.0, -8.0));
    assert_eq!(high, Vec3::new(20.0, 20.0, 10.0));
}

#[test]
fn a_sheet_comes_to_triangles_that_cover_it() {
    let corners = slope().triangles();
    assert_eq!(corners.len(), 4 * 2 * 3);
    assert!(corners.contains(&Vec3::new(20.0, 20.0, 10.0)));
    assert!(corners.contains(&Vec3::new(10.0, 0.0, 5.0)));
}

#[test]
fn open_air_goes_to_what_a_sheet_says_and_nothing_else_does() {
    let map = Map {
        water_sheets: vec![WaterSheet {
            contents: Contents::Slime,
            ..slope()
        }],
        ..Map::default()
    };
    let inside = Vec3::new(15.0, 5.0, 7.0);
    assert_eq!(map.liquid_or(inside, Contents::Empty), Contents::Slime);
    assert_eq!(map.liquid_or(inside, Contents::Sky), Contents::Slime);
    assert_eq!(map.liquid_or(inside, Contents::Solid), Contents::Solid);
    assert_eq!(map.liquid_or(inside, Contents::Lava), Contents::Lava);
    let outside = Vec3::new(15.0, 5.0, 20.0);
    assert_eq!(map.liquid_or(outside, Contents::Empty), Contents::Empty);
}

#[test]
fn the_look_round_the_eye_is_the_sheets_it_is_in() {
    let mut look = PoolLook::of(Contents::Water).expect("a liquid");
    look.absorption = 0.9;
    let map = Map {
        water_sheets: vec![WaterSheet { look, ..slope() }],
        ..Map::default()
    };
    let found = map.look_at(Vec3::new(15.0, 5.0, 7.0), Contents::Water);
    assert_eq!(found.map(|look| look.absorption), Some(0.9));
    let usual = map.look_at(Vec3::new(50.0, 5.0, 7.0), Contents::Water);
    assert_ne!(usual.map(|look| look.absorption), Some(0.9));
    assert_eq!(
        map.look_at(Vec3::new(50.0, 5.0, 7.0), Contents::Empty),
        None
    );
}
