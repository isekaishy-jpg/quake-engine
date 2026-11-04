//! The server, spoken to through its transport.

mod common;

use std::rc::Rc;
use std::sync::Arc;

use common::room;
use engine_core::math::Vec3;
use engine_core::player::UserCommand;
use engine_net::{ClientMessage, Loopback, ServerMessage, Snapshot, Transport, loopback};
use engine_physics::CollisionWorld;
use engine_server::{Level, Server};

/// A server, and the client's end.
fn setup() -> (Server, Loopback) {
    let map = room();
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let (client_end, server_end) = loopback();
    let origin = Vec3::new(0.0, 0.0, -255.0);
    let server = Server::new(
        Box::new(server_end),
        world,
        &map,
        origin,
        &[],
        1.0 / 72.0,
        Level::default(),
    )
    .expect("server");
    (server, client_end)
}

/// A forward command numbered `sequence`.
fn forward(sequence: u32) -> ClientMessage {
    let command = UserCommand {
        sequence,
        seconds: 1.0 / 72.0,
        forward: 400.0,
        ..UserCommand::default()
    };
    ClientMessage::Move(vec![command], 0)
}

/// The newest snapshot.
fn snapshot(client: &mut Loopback) -> Snapshot {
    let mut last = None;
    while let Some(bytes) = client.receive() {
        if let Some(ServerMessage::Snapshot(snapshot)) = ServerMessage::decode(&bytes) {
            last = Some(snapshot);
        }
    }
    last.expect("a snapshot")
}

#[test]
fn every_tick_sends_a_numbered_snapshot() {
    let (mut server, mut client) = setup();
    server.tick();
    assert_eq!(snapshot(&mut client).tick, 1);
    server.tick();
    assert_eq!(snapshot(&mut client).tick, 2);
}

#[test]
fn commands_run_and_are_acknowledged() {
    let (mut server, mut client) = setup();
    for sequence in 1..=30 {
        client.send(forward(sequence).encode());
        server.tick();
    }
    let state = snapshot(&mut client);
    assert_eq!(state.acknowledged, 30);
    assert!(state.player.origin.x > 1.0);
}

#[test]
fn old_commands_do_not_run_twice() {
    let (mut server, mut client) = setup();
    client.send(forward(5).encode());
    server.tick();
    let first = snapshot(&mut client);
    client.send(forward(5).encode());
    client.send(forward(3).encode());
    server.tick();
    assert_eq!(snapshot(&mut client).player, first.player);
}

#[test]
fn a_command_cannot_claim_a_long_time() {
    let (mut server, mut client) = setup();
    let command = UserCommand {
        sequence: 1,
        seconds: 1000.0,
        forward: 400.0,
        ..UserCommand::default()
    };
    let cheat = ClientMessage::Move(vec![command], 0);
    client.send(cheat.encode());
    server.tick();
    let state = snapshot(&mut client);
    assert!(state.player.origin.x < 100.0, "{}", state.player.origin.x);
}

#[test]
fn bad_bytes_are_ignored() {
    let (mut server, mut client) = setup();
    client.send(vec![255, 1, 2]);
    client.send(Vec::new());
    server.tick();
    assert_eq!(snapshot(&mut client).acknowledged, 0);
}

#[test]
fn console_commands_change_the_player() {
    let (mut server, mut client) = setup();
    client.send(ClientMessage::Console("noclip".to_owned()).encode());
    client.send(ClientMessage::Console("setpos 10 20 30".to_owned()).encode());
    server.tick();
    let state = snapshot(&mut client);
    assert!(state.player.noclip);
    assert_eq!(state.player.origin, Vec3::new(10.0, 20.0, 30.0));
}
