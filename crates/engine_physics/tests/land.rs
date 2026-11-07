//! Moving over land, straight from its heights (proposal 0144).

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_map::{CHUNK_CELLS, Map, TerrainData, TerrainSheet};
use engine_physics::CollisionWorld;

/// A player's size.
const MINS: Vec3 = Vec3::new(-16.0, -16.0, -24.0);
/// A player's size.
const MAXS: Vec3 = Vec3::new(16.0, 16.0, 32.0);

/// Land of one chunk from the origin, a cell every 32, its heights from `height`.
fn sheet(height: impl Fn(u32, u32) -> f32) -> TerrainSheet {
    let mut data = TerrainData::flat([1, 1], 32.0);
    let across = data.across();
    for y in 0..=CHUNK_CELLS {
        for x in 0..=CHUNK_CELLS {
            data.heights[(y * across + x) as usize] = height(x, y);
        }
    }
    TerrainSheet {
        data,
        ..TerrainSheet::default()
    }
}

/// A world of nothing but `sheets`.
fn world(sheets: Vec<TerrainSheet>) -> CollisionWorld {
    let map = Map {
        terrain: sheets,
        ..Map::default()
    };
    CollisionWorld::new(Arc::new(map))
}

/// A box falling straight down at `x`, `y`, from high up.
fn fall(world: &CollisionWorld, x: f32, y: f32) -> f32 {
    let (from, to) = (Vec3::new(x, y, 500.0), Vec3::new(x, y, -500.0));
    world.with(&[]).move_box(from, MINS, MAXS, to).end.z
}

#[test]
fn a_box_lands_on_flat_ground_with_its_feet_on_it() {
    let ground = world(vec![sheet(|_, _| 40.0)]);
    let stood = fall(&ground, 500.0, 500.0);
    assert!((stood - 64.0).abs() < 0.1, "{stood}");
    assert!(stood >= 64.0, "and not in it: {stood}");
}

#[test]
fn a_box_rests_on_the_highest_land_under_it() {
    // A step up at x = 320: a box over the edge stands on the high side.
    let ground = world(vec![sheet(|x, _| if x >= 10 { 64.0 } else { 0.0 })]);
    let on_edge = fall(&ground, 310.0, 500.0);
    assert!((on_edge - 88.0).abs() < 0.1, "{on_edge}");
}

#[test]
fn nothing_stops_a_box_off_the_edge_of_land() {
    let ground = world(vec![sheet(|_, _| 0.0)]);
    let stood = fall(&ground, -500.0, 500.0);
    assert!((stood + 500.0).abs() < 0.1, "{stood}");
}

#[test]
fn a_slope_pushes_back_along_its_face() {
    // Rising 1 in 2 along X.
    let ground = world(vec![sheet(|x, _| x as f32 * 16.0)]);
    let (from, to) = (
        Vec3::new(500.0, 500.0, 900.0),
        Vec3::new(500.0, 500.0, -500.0),
    );
    let trace = ground.with(&[]).move_box(from, MINS, MAXS, to);
    assert!(trace.fraction < 1.0);
    let slope = Vec3::new(-0.447, 0.0, 0.894);
    assert!((trace.normal - slope).length() < 0.01, "{:?}", trace.normal);
}

#[test]
fn a_line_stops_on_the_land_it_meets() {
    let ground = world(vec![sheet(|x, _| if x > 30 { 200.0 } else { 0.0 })]);
    let (from, to) = (
        Vec3::new(100.0, 100.0, 100.0),
        Vec3::new(1900.0, 100.0, 100.0),
    );
    let trace = ground.with(&[]).move_box(from, Vec3::ZERO, Vec3::ZERO, to);
    assert!(trace.fraction < 1.0, "{trace:?}");
    assert!((960.0..=992.0).contains(&trace.end.x), "{:?}", trace.end);
}

#[test]
fn a_long_line_finds_what_is_near_its_far_end() {
    let ground = world(vec![sheet(
        |x, y| if x == 60 && y == 60 { 400.0 } else { 0.0 },
    )]);
    let (from, to) = (
        Vec3::new(10.0, 10.0, 100.0),
        Vec3::new(2040.0, 2040.0, 100.0),
    );
    let trace = ground.with(&[]).move_box(from, Vec3::ZERO, Vec3::ZERO, to);
    assert!(trace.fraction < 1.0, "it goes by the peak: {trace:?}");
    assert!(trace.end.x > 1800.0, "{:?}", trace.end);
}

#[test]
fn a_box_falls_through_a_hole() {
    let mut land = sheet(|_, _| 0.0);
    for y in 10..20 {
        for x in 10..20 {
            land.data.set_hole(x, y, true);
        }
    }
    let ground = world(vec![land]);
    let middle = fall(&ground, 480.0, 480.0);
    assert!((middle + 500.0).abs() < 0.1, "{middle}");
    let beside = fall(&ground, 1000.0, 480.0);
    assert!((beside - 24.0).abs() < 0.1, "{beside}");
}

#[test]
fn land_is_only_solid_from_the_side_it_faces() {
    let ground = world(vec![sheet(|_, _| 100.0)]);
    let (from, to) = (Vec3::new(500.0, 500.0, 0.0), Vec3::new(500.0, 500.0, 300.0));
    let rising = ground.with(&[]).move_box(from, MINS, MAXS, to);
    assert!((rising.fraction - 1.0).abs() < 0.001, "{rising:?}");
}

#[test]
fn a_sheet_facing_down_is_a_roof() {
    let roof = TerrainSheet {
        facing_down: true,
        ..sheet(|_, _| 200.0)
    };
    let ground = world(vec![roof]);
    let (from, to) = (Vec3::new(500.0, 500.0, 0.0), Vec3::new(500.0, 500.0, 300.0));
    let head = ground.with(&[]).move_box(from, MINS, MAXS, to);
    assert!((head.end.z - 168.0).abs() < 0.1, "{:?}", head.end);
    assert!(head.normal.z < -0.9, "{:?}", head.normal);
}

#[test]
fn what_is_under_land_is_solid_for_its_thickness() {
    let thin = TerrainSheet {
        thickness: 100.0,
        ..sheet(|_, _| 0.0)
    };
    let ground = world(vec![thin]);
    assert!(ground.in_solid(Vec3::new(500.0, 500.0, -50.0)));
    assert!(!ground.in_solid(Vec3::new(500.0, 500.0, -150.0)));
    assert!(!ground.in_solid(Vec3::new(500.0, 500.0, 50.0)));
}

#[test]
fn walking_a_slope_never_ends_up_in_it() {
    let ground = world(vec![sheet(|x, y| {
        (x as f32 * 0.4).sin() * 30.0 + (y as f32 * 0.3).cos() * 30.0 + 40.0
    })]);
    let collision = ground.with(&[]);
    let mut at = Vec3::new(300.0, 300.0, 200.0);
    let down = collision.move_box(at, MINS, MAXS, at - Vec3::new(0.0, 0.0, 400.0));
    at = down.end;
    for step in 0..2000 {
        let way = Vec3::new(5.0, 3.0, 0.0);
        let up = collision
            .move_box(at, MINS, MAXS, at + Vec3::new(0.0, 0.0, 18.0))
            .end;
        let across = collision.move_box(up, MINS, MAXS, up + way).end;
        at = collision
            .move_box(across, MINS, MAXS, across - Vec3::new(0.0, 0.0, 40.0))
            .end;
        let feet = at + Vec3::new(0.0, 0.0, MINS.z);
        let land = ground.map().terrain[0].height_at(at.x, at.y);
        if let Some(land) = land {
            assert!(feet.z >= land - 0.5, "step {step}: {feet:?} under {land}");
        }
        if at.x > 1900.0 || at.y > 1900.0 {
            return;
        }
    }
    panic!("it never got there: {at:?}");
}
