//! Movement in a plain room, against the numbers Quake gives at 72 Hz.

mod common;

use common::{room, room_with_step, room_with_water};
use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_movement::{MoveVars, player_move};
use engine_physics::CollisionWorld;

/// One tick.
const TICK: f32 = 1.0 / 72.0;

/// The floor, where a player's centre rests.
const FLOOR: f32 = -256.0;

/// A player standing on the floor.
fn standing() -> PlayerState {
    PlayerState {
        origin: Vec3::new(0.0, 0.0, FLOOR + 1.0),
        ..PlayerState::default()
    }
}

/// A command of one tick.
fn command() -> UserCommand {
    UserCommand {
        seconds: TICK,
        ..UserCommand::default()
    }
}

/// Runs `command` for `ticks` ticks.
fn run(state: &mut PlayerState, command: UserCommand, ticks: u32, world: &CollisionWorld) {
    let vars = MoveVars::default();
    for _ in 0..ticks {
        player_move(state, &command, &vars, world);
    }
}

#[test]
fn a_player_falls_and_lands() {
    let world = CollisionWorld::new(room());
    let mut state = PlayerState::default();
    run(&mut state, command(), 72 * 2, &world);
    assert!(state.on_ground);
    assert!(state.velocity.z.abs() < f32::EPSILON);
    assert!((state.origin.z - FLOOR).abs() < 0.1, "{}", state.origin.z);
}

#[test]
fn running_reaches_max_speed_and_no_more() {
    let world = CollisionWorld::new(room());
    let mut state = standing();
    let mut forward = command();
    forward.forward = 400.0;
    forward.yaw = 0.0;
    state.origin.x = -250.0;
    run(&mut state, forward, 72, &world);
    assert!(
        (state.velocity.x - 320.0).abs() < 0.5,
        "{}",
        state.velocity.x
    );
}

/// `docs/pre-research/stack.md`: about 63.8 units from 320 at 72 Hz.
#[test]
fn stopping_from_full_speed_takes_quakes_distance() {
    let world = CollisionWorld::new(room());
    let mut state = standing();
    run(&mut state, command(), 2, &world);
    state.origin.x = -200.0;
    state.velocity.x = 320.0;
    let start = state.origin.x;
    run(&mut state, command(), 144, &world);
    assert!(state.velocity.x.abs() < f32::EPSILON);
    let distance = state.origin.x - start;
    assert!((distance - 63.8).abs() < 1.5, "{distance}");
}

/// `docs/pre-research/stack.md`: about 43.7 units at 72 Hz.
#[test]
fn a_jump_rises_quakes_height() {
    let world = CollisionWorld::new(room());
    let mut state = standing();
    run(&mut state, command(), 4, &world);
    let floor = state.origin.z;
    let mut jump = command();
    jump.jump = true;
    let vars = MoveVars::default();
    let mut highest = floor;
    for _ in 0..72 {
        player_move(&mut state, &jump, &vars, &world);
        highest = highest.max(state.origin.z);
    }
    let height = highest - floor;
    assert!((height - 43.7).abs() < 1.0, "{height}");
}

#[test]
fn holding_jump_does_not_repeat() {
    let world = CollisionWorld::new(room());
    let mut state = standing();
    run(&mut state, command(), 4, &world);
    let mut jump = command();
    jump.jump = true;
    run(&mut state, jump, 200, &world);
    assert!(state.on_ground, "it lands and stays down");
    assert!(state.jump_held);
}

#[test]
fn a_wall_stops_the_player_and_keeps_the_slide() {
    let world = CollisionWorld::new(room());
    let mut state = standing();
    run(&mut state, command(), 2, &world);
    state.origin.x = 250.0;
    let mut diagonal = command();
    diagonal.forward = 400.0;
    diagonal.side = -400.0;
    run(&mut state, diagonal, 36, &world);
    assert!(state.origin.x <= 256.0);
    assert!(!world.in_solid(state.origin));
    assert!(
        state.origin.y > 20.0,
        "it slid along the wall: {}",
        state.origin.y
    );
}

/// A player running along x from the low floor.
fn run_at_a_step(height: f32) -> PlayerState {
    let world = CollisionWorld::new(room_with_step(height));
    let mut state = standing();
    state.origin.x = -100.0;
    run(&mut state, command(), 4, &world);
    let mut forward = command();
    forward.forward = 400.0;
    run(&mut state, forward, 72, &world);
    state
}

#[test]
fn a_stair_is_walked_up() {
    let state = run_at_a_step(16.0);
    assert!(state.origin.x > 50.0, "{}", state.origin.x);
    assert!(
        (state.origin.z - (FLOOR + 16.0)).abs() < 0.1,
        "{}",
        state.origin.z
    );
    assert!(state.on_ground);
}

/// `pmove.c`: `STEPSIZE` is 18.
#[test]
fn a_step_too_high_is_a_wall() {
    let state = run_at_a_step(24.0);
    assert!(state.origin.x <= 0.0, "{}", state.origin.x);
    assert!((state.origin.z - FLOOR).abs() < 0.1, "{}", state.origin.z);
}

/// A player under water.
fn swimming() -> PlayerState {
    PlayerState {
        origin: Vec3::new(0.0, 0.0, -200.0),
        ..PlayerState::default()
    }
}

#[test]
fn water_is_measured_from_the_feet_up() {
    let world = CollisionWorld::new(room_with_water());
    let vars = MoveVars::default();
    let level = |z: f32| {
        let mut state = PlayerState {
            origin: Vec3::new(0.0, 0.0, z),
            ..PlayerState::default()
        };
        player_move(&mut state, &command(), &vars, &world);
        state.water_level
    };
    assert_eq!(level(-90.0), 0, "the feet are above the surface");
    assert_eq!(level(-110.0), 1, "the feet are in");
    assert_eq!(level(-140.0), 2, "the waist is in");
    assert_eq!(level(-200.0), 3, "the eyes are in");
}

#[test]
fn a_player_sinks_slowly_in_water() {
    let world = CollisionWorld::new(room_with_water());
    let mut state = swimming();
    run(&mut state, command(), 72, &world);
    assert_eq!(state.liquid, engine_core::player::Liquid::Water);
    assert!(state.origin.z < -200.0);
    assert!(state.origin.z > -260.0, "{}", state.origin.z);
    assert!(state.velocity.z > -60.0, "{}", state.velocity.z);
}

#[test]
fn a_player_swims_where_they_look() {
    let world = CollisionWorld::new(room_with_water());
    let mut state = swimming();
    let mut up = command();
    up.forward = 400.0;
    up.pitch = -80.0;
    run(&mut state, up, 36, &world);
    assert!(state.origin.z > -180.0, "{}", state.origin.z);
}

#[test]
fn jump_swims_upwards() {
    let world = CollisionWorld::new(room_with_water());
    let mut state = swimming();
    let mut jump = command();
    jump.jump = true;
    run(&mut state, jump, 36, &world);
    assert!(state.origin.z > -190.0, "{}", state.origin.z);
}

#[test]
fn air_control_is_limited() {
    let world = CollisionWorld::new(room());
    let mut state = PlayerState {
        velocity: Vec3::new(0.0, 0.0, 200.0),
        ..PlayerState::default()
    };
    let mut forward = command();
    forward.forward = 400.0;
    run(&mut state, forward, 10, &world);
    assert!(state.velocity.x <= 30.5, "{}", state.velocity.x);
    assert!(state.velocity.x > 25.0, "{}", state.velocity.x);
}

#[test]
fn noclip_passes_through_walls() {
    let world = CollisionWorld::new(room());
    let mut state = PlayerState {
        noclip: true,
        ..PlayerState::default()
    };
    let mut forward = command();
    forward.forward = 400.0;
    run(&mut state, forward, 72 * 2, &world);
    assert!((state.origin.x - 800.0).abs() < 1.0, "{}", state.origin.x);
}

#[test]
fn a_player_stuck_a_little_in_a_wall_is_nudged_out() {
    let world = CollisionWorld::new(room());
    let mut state = standing();
    state.origin.x = 256.05;
    run(&mut state, command(), 1, &world);
    assert!(!world.in_solid(state.origin), "{}", state.origin.x);
}

#[test]
fn the_same_input_gives_the_same_result() {
    let world = CollisionWorld::new(room());
    let mut forward = command();
    forward.forward = 400.0;
    forward.yaw = 33.0;
    let (mut first, mut second) = (standing(), standing());
    run(&mut first, forward, 300, &world);
    run(&mut second, forward, 300, &world);
    assert_eq!(first, second);
}
