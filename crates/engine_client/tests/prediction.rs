//! Commands, prediction and the render tick, with a server played by the test.

mod common;

use std::rc::Rc;

use common::room;
use engine_client::{Buttons, Client, StepSmoother};
use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_movement::{MoveVars, player_move};
use engine_net::{ClientMessage, Loopback, ServerMessage, Snapshot, Transport, loopback};
use engine_physics::CollisionWorld;

/// One tick.
const TICK: f32 = 1.0 / 72.0;

/// Where the player starts.
const START: Vec3 = Vec3::new(0.0, 0.0, -255.0);

/// A client, its server's end, and the world.
fn setup() -> (Client, Loopback, Rc<CollisionWorld>) {
    let world = Rc::new(CollisionWorld::new(room()));
    let (client_end, server_end) = loopback();
    let client = Client::new(Box::new(client_end), Rc::clone(&world), START);
    (client, server_end, world)
}

/// Running forward.
fn forward() -> UserCommand {
    let mut buttons = Buttons::default();
    assert!(buttons.set("forward", true));
    buttons.command(0.0, 0.0)
}

/// Every command waiting at the server.
fn received(server: &mut Loopback) -> Vec<UserCommand> {
    std::iter::from_fn(|| server.receive())
        .filter_map(|bytes| match ClientMessage::decode(&bytes) {
            Some(ClientMessage::Move(command)) => Some(command),
            _ => None,
        })
        .collect()
}

/// Sends a snapshot of `player`, with `acknowledged` run.
fn answer(server: &mut Loopback, acknowledged: u32, player: PlayerState) {
    let snapshot = Snapshot {
        tick: acknowledged,
        acknowledged,
        player,
    };
    server.send(ServerMessage::Snapshot(snapshot).encode());
}

#[test]
fn commands_are_numbered_in_order_and_carry_their_time() {
    let (mut client, mut server, _world) = setup();
    client.send_command(forward(), TICK);
    client.send_command(forward(), TICK);
    let commands = received(&mut server);
    assert_eq!(
        commands.iter().map(|c| c.sequence).collect::<Vec<_>>(),
        [1, 2]
    );
    assert!((commands[0].seconds - TICK).abs() < f32::EPSILON);
}

#[test]
fn the_client_predicts_without_waiting_for_the_server() {
    let (mut client, _server, _world) = setup();
    for _ in 0..10 {
        client.send_command(forward(), TICK);
    }
    assert!(client.predicted().origin.x > 1.0);
    assert_eq!(client.pending_count(), 10);
}

/// `cl_pred.c`: start from the server's state and replay what it hasn't run.
#[test]
fn a_late_snapshot_replays_to_the_same_place() {
    let (mut client, mut server, world) = setup();
    for _ in 0..20 {
        client.send_command(forward(), TICK);
    }
    let predicted = *client.predicted();
    let commands = received(&mut server);
    let mut truth = PlayerState {
        origin: START,
        ..PlayerState::default()
    };
    for command in &commands[..5] {
        player_move(&mut truth, command, &MoveVars::default(), &world);
    }
    answer(&mut server, 5, truth);
    client.receive();
    assert_eq!(client.pending_count(), 15);
    assert_eq!(*client.predicted(), predicted);
}

#[test]
fn the_server_wins_when_it_disagrees() {
    let (mut client, mut server, _world) = setup();
    client.send_command(forward(), TICK);
    let moved = PlayerState {
        origin: Vec3::new(100.0, 100.0, -255.0),
        ..PlayerState::default()
    };
    answer(&mut server, 1, moved);
    client.receive();
    assert_eq!(client.predicted().origin, moved.origin);
    assert_eq!(client.pending_count(), 0);
}

#[test]
fn bad_bytes_from_the_server_are_ignored() {
    let (mut client, mut server, _world) = setup();
    server.send(vec![1, 2, 3]);
    client.receive();
    assert_eq!(client.predicted().origin, START);
}

#[test]
fn the_render_tick_moves_a_copy_and_leaves_the_state_alone() {
    let (mut client, _server, _world) = setup();
    for _ in 0..20 {
        client.send_command(forward(), TICK);
    }
    let before = *client.predicted();
    let drawn = client.render_state(&forward(), TICK * 0.5);
    assert!(drawn.origin.x > before.origin.x);
    assert_eq!(*client.predicted(), before);
    assert_eq!(client.render_state(&forward(), 0.0), before);
}

#[test]
fn going_up_a_stair_is_smoothed_and_catches_up() {
    let mut step = StepSmoother::default();
    assert!(step.update(0.0, true, 0.01).abs() < f32::EPSILON);
    let lag = step.update(16.0, true, 0.01);
    assert!((lag + 12.0).abs() < 0.001, "at most 12 behind: {lag}");
    let later = step.update(16.0, true, 0.05);
    assert!(
        (later + 8.0).abs() < 0.001,
        "catches up at 80 a second: {later}"
    );
    assert!(step.update(16.0, true, 1.0).abs() < f32::EPSILON);
}

#[test]
fn going_down_or_in_the_air_is_not_smoothed() {
    let mut step = StepSmoother::default();
    assert!(step.update(100.0, false, 0.01).abs() < f32::EPSILON);
    assert!(step.update(50.0, true, 0.01).abs() < f32::EPSILON);
}

#[test]
fn held_buttons_become_run_speed_and_speed_walks() {
    let mut buttons = Buttons::default();
    assert!(buttons.set("forward", true));
    assert!(buttons.set("moveleft", true));
    let command = buttons.command(10.0, 20.0);
    assert!((command.forward - 400.0).abs() < f32::EPSILON);
    assert!((command.side + 400.0).abs() < f32::EPSILON);
    assert!((command.yaw - 10.0).abs() < f32::EPSILON);
    assert!(buttons.set("speed", true));
    assert!((buttons.command(0.0, 0.0).forward - 200.0).abs() < f32::EPSILON);
}

#[test]
fn unknown_buttons_are_reported() {
    assert!(!Buttons::default().set("attack", true));
}
