//! Encoding, and refusing bad input.

use engine_core::math::Vec3;
use engine_core::player::{Liquid, PlayerState, UserCommand};
use engine_net::{ClientMessage, ServerMessage, Snapshot, Transport, loopback};

/// A command with every field set.
fn command() -> UserCommand {
    UserCommand {
        sequence: 7,
        seconds: 0.25,
        yaw: 90.5,
        pitch: -12.25,
        forward: 400.0,
        side: -200.0,
        up: 100.0,
        jump: true,
    }
}

/// A snapshot with every field set.
fn snapshot() -> Snapshot {
    Snapshot {
        tick: 9,
        acknowledged: 7,
        player: PlayerState {
            origin: Vec3::new(1.0, 2.0, 3.0),
            velocity: Vec3::new(4.0, 5.0, 6.0),
            on_ground: true,
            water_level: 2,
            liquid: Liquid::Slime,
            water_jump_time: 1.5,
            jump_held: true,
            noclip: true,
        },
    }
}

#[test]
fn messages_survive_the_round_trip() {
    let moving = ClientMessage::Move(command());
    assert_eq!(ClientMessage::decode(&moving.encode()), Some(moving));
    let console = ClientMessage::Console("noclip".to_owned());
    assert_eq!(ClientMessage::decode(&console.encode()), Some(console));
    let state = ServerMessage::Snapshot(snapshot());
    assert_eq!(ServerMessage::decode(&state.encode()), Some(state));
}

#[test]
fn short_and_long_messages_are_refused() {
    let bytes = ClientMessage::Move(command()).encode();
    for length in 0..bytes.len() {
        assert_eq!(ClientMessage::decode(&bytes[..length]), None, "{length}");
    }
    let mut long = bytes;
    long.push(0);
    assert_eq!(ClientMessage::decode(&long), None);
    let bytes = ServerMessage::Snapshot(snapshot()).encode();
    for length in 0..bytes.len() {
        assert_eq!(ServerMessage::decode(&bytes[..length]), None, "{length}");
    }
}

#[test]
fn numbers_that_are_not_numbers_are_refused() {
    let mut nan = command();
    nan.yaw = f32::NAN;
    assert_eq!(
        ClientMessage::decode(&ClientMessage::Move(nan).encode()),
        None
    );
    let mut infinite = command();
    infinite.seconds = f32::INFINITY;
    assert_eq!(
        ClientMessage::decode(&ClientMessage::Move(infinite).encode()),
        None
    );
}

#[test]
fn unknown_tags_and_bad_flags_are_refused() {
    assert_eq!(ClientMessage::decode(&[99]), None);
    assert_eq!(ServerMessage::decode(&[99]), None);
    let mut bytes = ClientMessage::Move(command()).encode();
    *bytes.last_mut().expect("bytes") = 7;
    assert_eq!(ClientMessage::decode(&bytes), None);
}

#[test]
fn oversized_console_text_is_refused() {
    let text = "x".repeat(257);
    assert_eq!(
        ClientMessage::decode(&ClientMessage::Console(text).encode()),
        None
    );
}

#[test]
fn text_that_is_not_utf8_is_refused() {
    assert_eq!(ClientMessage::decode(&[2, 0xFF, 0xFE]), None);
}

#[test]
fn loopback_carries_messages_both_ways_in_order() {
    let (mut first, mut second) = loopback();
    first.send(vec![1]);
    first.send(vec![2]);
    second.send(vec![3]);
    assert_eq!(second.receive(), Some(vec![1]));
    assert_eq!(second.receive(), Some(vec![2]));
    assert_eq!(second.receive(), None);
    assert_eq!(first.receive(), Some(vec![3]));
}
