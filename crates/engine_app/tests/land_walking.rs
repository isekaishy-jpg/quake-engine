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
