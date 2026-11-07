//! A player on land, with the real movement code (proposal 0144).

use std::sync::Arc;

use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_map::{CHUNK_CELLS, Map, TerrainData, TerrainSheet};
use engine_movement::{MoveVars, player_move};
use engine_physics::CollisionWorld;

/// A sheet of one chunk from the origin, its heights from `height`.
fn land(height: impl Fn(u32, u32) -> f32) -> CollisionWorld {
    let mut data = TerrainData::flat([1, 1], 32.0);
    let across = data.across();
    for y in 0..=CHUNK_CELLS {
        for x in 0..=CHUNK_CELLS {
            data.heights[(y * across + x) as usize] = height(x, y);
        }
    }
    let map = Map {
        terrain: vec![TerrainSheet {
            data,
            ..TerrainSheet::default()
        }],
        ..Map::default()
    };
    CollisionWorld::new(Arc::new(map))
}

/// Where someone who starts at `from` and walks along X for `seconds` gets to.
fn walk(world: &CollisionWorld, from: Vec3, seconds: f32) -> PlayerState {
    let mut state = PlayerState {
        origin: from,
        ..PlayerState::default()
    };
    let vars = MoveVars::default();
    let tick = 1.0 / 72.0;
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        reason = "a test's length"
    )]
    let ticks = (seconds / tick) as u32;
    for at in 0..ticks {
        let command = UserCommand {
            sequence: at,
            seconds: tick,
            forward: 320.0,
            ..UserCommand::default()
        };
        let mut touched = Vec::new();
        player_move(&mut state, &command, &vars, world.with(&[]), &mut touched);
    }
    state
}

/// The land's height under `state`.
fn land_under(world: &CollisionWorld, state: &PlayerState) -> f32 {
    let sheet = &world.map().terrain[0];
    sheet
        .height_at(state.origin.x, state.origin.y)
        .expect("it's on the land")
}

#[test]
fn someone_stands_on_flat_land() {
    let world = land(|_, _| 100.0);
    let state = walk(&world, Vec3::new(300.0, 300.0, 300.0), 2.0);
    assert!(state.on_ground, "{state:?}");
    let above = state.origin.z - 100.0;
    assert!((23.5..25.0).contains(&above), "{above}");
}

#[test]
fn someone_walks_over_a_gentle_slope_and_keeps_going() {
    // One in four.
    let world = land(|x, _| x as f32 * 8.0);
    let state = walk(&world, Vec3::new(100.0, 300.0, 200.0), 3.0);
    assert!(state.on_ground, "{state:?}");
    assert!(state.origin.x > 700.0, "it got along: {:?}", state.origin);
    let above = state.origin.z - land_under(&world, &state);
    assert!((20.0..32.0).contains(&above), "on the ground: {above}");
}

#[test]
fn a_cliff_stops_someone() {
    let world = land(|x, _| if x >= 20 { 300.0 } else { 0.0 });
    let state = walk(&world, Vec3::new(300.0, 300.0, 100.0), 3.0);
    assert!(state.origin.x < 640.0, "{:?}", state.origin);
    assert!(state.origin.x > 560.0, "it reached it: {:?}", state.origin);
}

#[test]
fn walking_over_rough_land_stays_on_it() {
    let world = land(|x, y| (x as f32 * 0.5).sin() * 12.0 + (y as f32 * 0.4).cos() * 12.0);
    let state = walk(&world, Vec3::new(100.0, 300.0, 200.0), 5.0);
    assert!(state.origin.x > 900.0, "it got along: {:?}", state.origin);
    let above = state.origin.z - land_under(&world, &state);
    assert!(above > 20.0, "not in it: {above}");
}

/// A sheet of one chunk at height `z`, with only what `keep` says left of it.
fn sheet_at(z: f32, keep: impl Fn(u32, u32) -> bool) -> TerrainSheet {
    let mut data = TerrainData::flat([1, 1], 32.0);
    for y in 0..CHUNK_CELLS {
        for x in 0..CHUNK_CELLS {
            data.set_hole(x, y, !keep(x, y));
        }
    }
    TerrainSheet {
        data,
        origin: Vec3::new(0.0, 0.0, z),
        ..TerrainSheet::default()
    }
}

/// A world of `sheets`.
fn stacked(sheets: Vec<TerrainSheet>) -> CollisionWorld {
    let map = Map {
        terrain: sheets,
        ..Map::default()
    };
    CollisionWorld::new(Arc::new(map))
}

/// A road along X at height 0, and a bridge across it along Y at height 200 with an underside at
/// 170, in a strip from 800 to 1088 along X.
fn bridge() -> CollisionWorld {
    let strip = |x: u32, _: u32| (25..34).contains(&x);
    let deck = TerrainSheet {
        thickness: 30.0,
        ..sheet_at(200.0, strip)
    };
    let underside = TerrainSheet {
        facing_down: true,
        thickness: 30.0,
        ..sheet_at(170.0, strip)
    };
    stacked(vec![sheet_at(0.0, |_, _| true), deck, underside])
}

#[test]
fn someone_walks_along_a_road_under_a_bridge() {
    let world = bridge();
    let state = walk(&world, Vec3::new(300.0, 1000.0, 40.0), 4.0);
    assert!(
        state.origin.x > 1300.0,
        "under it and out: {:?}",
        state.origin
    );
    assert!(state.origin.z < 40.0, "on the road: {:?}", state.origin);
}

#[test]
fn someone_stands_on_a_bridge_over_a_road() {
    let world = bridge();
    let state = walk(&world, Vec3::new(850.0, 1000.0, 260.0), 0.5);
    assert!(
        (state.origin.z - 224.0).abs() < 2.0,
        "on the deck: {:?}",
        state.origin
    );
}

/// Ground with a hole in it, 40 thick, a cave's roof under that, and its floor far below.
fn cave() -> CollisionWorld {
    let open = |x: u32, y: u32| !(28..36).contains(&x) || !(28..36).contains(&y);
    let ground = TerrainSheet {
        thickness: 40.0,
        ..sheet_at(0.0, open)
    };
    let roof = TerrainSheet {
        facing_down: true,
        thickness: 40.0,
        ..sheet_at(-40.0, open)
    };
    stacked(vec![ground, roof, sheet_at(-300.0, |_, _| true)])
}

#[test]
fn someone_falls_through_a_hole_into_a_cave_and_walks_under_the_ground() {
    let world = cave();
    let state = walk(&world, Vec3::new(800.0, 1024.0, 60.0), 5.0);
    assert!(state.origin.z < -250.0, "down in it: {:?}", state.origin);
    assert!(
        state.origin.x > 1400.0,
        "and along under the land: {:?}",
        state.origin
    );
    assert!(
        !world.in_solid(state.origin),
        "and not stuck: {:?}",
        state.origin
    );
}

#[test]
fn a_cave_has_a_roof() {
    let world = cave();
    let (from, to) = (
        Vec3::new(500.0, 500.0, -270.0),
        Vec3::new(500.0, 500.0, 200.0),
    );
    let trace = world.trace(from, to);
    assert!(trace.fraction < 1.0, "{trace:?}");
    assert!(trace.end.z < -60.0, "{:?}", trace.end);
}
