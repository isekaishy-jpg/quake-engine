//! Encoding, and refusing bad input.

use engine_core::entity::{Effect, EntityState, ModelInstance};
use engine_core::math::Vec3;
use engine_core::player::{Liquid, PlayerState, UserCommand};
use engine_core::sound::{SoundStart, StaticSound};
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
        attack: true,
        impulse: 7,
        view_time: 0.0,
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
            no_backing: 0.25,
            jump_held: true,
            noclip: true,
            dead: true,
            frozen: false,
            ground_model: Some(4),
        },
        instances: vec![ModelInstance {
            model: 3,
            origin: Vec3::new(7.0, 8.0, 9.0),
            visible: true,
            frame: 3,
            solid: false,
        }],
        entities: vec![EntityState {
            id: u64::MAX,
            model: 7,
            frame: 300,
            skin: 2,
            effects: 8,
            origin: Vec3::new(1.0, 2.0, 3.0),
            angles: Vec3::new(4.0, 5.0, 6.0),
            clip: 3,
            clip_start: 12.5,
            morphs: [
                [1, 200],
                [3, 7],
                [0; 2],
                [0; 2],
                [0; 2],
                [0; 2],
                [0; 2],
                [0; 2],
            ],
        }],
        boxes: vec![(Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 5.0, 6.0))],
        weapon_model: 3,
        weapon_frame: 5,
        player_entity: 7,
        player_effects: 2,
        punch: Vec3::new(-2.0, 0.0, 0.0),
        time: 0.0,
        stats: Vec::new(),
    }
}

#[test]
fn messages_survive_the_round_trip() {
    let moving = ClientMessage::Move(vec![command()], 0);
    assert_eq!(ClientMessage::decode(&moving.encode()), Some(moving));
    let console = ClientMessage::Console("noclip".to_owned());
    assert_eq!(ClientMessage::decode(&console.encode()), Some(console));
    for message in [
        ServerMessage::Snapshot(snapshot()),
        ServerMessage::LightStyle {
            index: 5,
            pattern: "abc".to_owned(),
        },
        ServerMessage::Print("hello".to_owned()),
        ServerMessage::Sound(SoundStart {
            entity: u64::MAX,
            channel: 7,
            origin: Vec3::new(1.0, 2.0, 3.0),
            volume: 0.5,
            attenuation: 4.0,
            own: true,
            name: "misc/talk.wav".to_owned(),
        }),
        ServerMessage::StaticSound(StaticSound {
            origin: Vec3::new(4.0, 5.0, 6.0),
            volume: 1.0,
            attenuation: 3.0,
            name: "ambience/fire1.wav".to_owned(),
        }),
        ServerMessage::Music(6),
        ServerMessage::Gravity(100.0),
        ServerMessage::Day {
            hour: 17.5,
            at: 1234.25,
        },
        ServerMessage::Effect(Effect::Temp {
            kind: 3,
            origin: Vec3::new(1.0, 2.0, 3.0),
            end: Vec3::new(4.0, 5.0, 6.0),
            entity: 9,
        }),
        ServerMessage::Effect(Effect::Particles {
            origin: Vec3::new(1.0, 2.0, 3.0),
            direction: Vec3::new(0.0, 0.0, 100.0),
            color: 73,
            count: 40,
        }),
        ServerMessage::ModelName {
            number: 12,
            name: "progs/flame.mdl".to_owned(),
        },
        ServerMessage::SetView {
            yaw: 90.0,
            pitch: -10.0,
        },
    ] {
        assert_eq!(ServerMessage::decode(&message.encode()), Some(message));
    }
}

#[test]
fn a_snapshot_claiming_too_many_models_is_refused() {
    let mut bytes = ServerMessage::Snapshot(snapshot()).encode();
    let count_at = bytes.len() - 4 - 18;
    bytes[count_at..count_at + 4].copy_from_slice(&u32::MAX.to_le_bytes());
    assert_eq!(ServerMessage::decode(&bytes), None);
}

#[test]
fn short_and_long_messages_are_refused() {
    let bytes = ClientMessage::Move(vec![command()], 0).encode();
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
        ClientMessage::decode(&ClientMessage::Move(vec![nan], 0).encode()),
        None
    );
    let mut infinite = command();
    infinite.seconds = f32::INFINITY;
    assert_eq!(
        ClientMessage::decode(&ClientMessage::Move(vec![infinite], 0).encode()),
        None
    );
}

#[test]
fn unknown_tags_and_bad_flags_are_refused() {
    assert_eq!(ClientMessage::decode(&[99]), None);
    assert_eq!(ServerMessage::decode(&[99]), None);
    let mut bytes = ClientMessage::Move(vec![command()], 0).encode();
    let jump = bytes.len() - 11;
    bytes[jump] = 7;
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

/// Bytes of a sound with these values.
fn sound_bytes(channel: u8, volume: f32, attenuation: f32, name: &str) -> Vec<u8> {
    let mut bytes = ServerMessage::Sound(SoundStart {
        entity: 1,
        channel: 0,
        origin: Vec3::ZERO,
        volume: 1.0,
        attenuation: 1.0,
        own: false,
        name: "a".to_owned(),
    })
    .encode();
    bytes.truncate(bytes.len() - 1);
    let channel_at = 1 + size_of::<u64>();
    bytes[channel_at] = channel;
    let volume_at = bytes.len() - 8;
    bytes[volume_at..volume_at + 4].copy_from_slice(&volume.to_le_bytes());
    bytes[volume_at + 4..].copy_from_slice(&attenuation.to_le_bytes());
    bytes.extend_from_slice(name.as_bytes());
    bytes
}

#[test]
fn sounds_out_of_range_are_refused() {
    assert!(ServerMessage::decode(&sound_bytes(0, 1.0, 1.0, "a")).is_some());
    assert!(ServerMessage::decode(&sound_bytes(8, 1.0, 1.0, "a")).is_none());
    assert!(ServerMessage::decode(&sound_bytes(0, 1.5, 1.0, "a")).is_none());
    assert!(ServerMessage::decode(&sound_bytes(0, -0.5, 1.0, "a")).is_none());
    assert!(ServerMessage::decode(&sound_bytes(0, 1.0, 5.0, "a")).is_none());
    assert!(ServerMessage::decode(&sound_bytes(0, f32::NAN, 1.0, "a")).is_none());
    assert!(ServerMessage::decode(&sound_bytes(0, 1.0, 1.0, "")).is_none());
    let long = "a".repeat(65);
    assert!(ServerMessage::decode(&sound_bytes(0, 1.0, 1.0, &long)).is_none());
}

#[test]
fn a_music_message_is_one_track() {
    assert_eq!(
        ServerMessage::decode(&[9, 6]),
        Some(ServerMessage::Music(6))
    );
    assert_eq!(ServerMessage::decode(&[9]), None);
    assert_eq!(ServerMessage::decode(&[9, 6, 7]), None);
}

#[test]
fn a_named_effect_comes_through() {
    let message = ServerMessage::Effect(Effect::Named {
        effect: 1234,
        origin: Vec3::new(1.0, 2.0, 3.0),
        entity: 9,
        handle: 77,
        settings: [0.25, 0.5, 0.75, 2.0, 3.0],
    });
    assert_eq!(ServerMessage::decode(&message.encode()), Some(message));
    let stopping = ServerMessage::Effect(Effect::NamedStops {
        handle: 77,
        seconds: 1.5,
    });
    assert_eq!(ServerMessage::decode(&stopping.encode()), Some(stopping));
}
