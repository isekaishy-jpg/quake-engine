//! More than one player on a server, through real scripts (proposals 0045 and
//! 0046).

mod common;

use std::rc::Rc;
use std::sync::Arc;

use common::map;
use engine_core::entity::ModelInstance;
use engine_core::math::Vec3;
use engine_core::player::{PlayerState, UserCommand};
use engine_map::Map;
use engine_net::{
    ClientMessage, Loopback, PlayerInfo, ServerMessage, Snapshot, Transport, apply_delta,
    delta_baseline, delta_from, loopback,
};
use engine_physics::CollisionWorld;
use engine_server::{Level, Seat, SeatInfo, Server};
use fixture_quake::SCRIPTS;

/// A tick.
const TICK: f64 = 1.0 / 72.0;

/// An open room with a world.
fn room() -> Arc<Map> {
    map(&[], &[&[("classname", "worldspawn")]])
}

/// One end of a game, for a test to play.
struct Player {
    /// The connection.
    end: Loopback,
    /// The next command's number.
    sequence: u32,
    /// The latest snapshot.
    latest: Option<Snapshot>,
    /// What's been printed to them.
    prints: Vec<String>,
    /// Which level they've been told to load.
    level: Option<String>,
    /// The last snapshot they could read, for deltas.
    seen: u32,
    /// Whether to acknowledge snapshots, which brings deltas.
    acknowledge: bool,
}

impl Player {
    /// A player on `end`.
    fn new(end: Loopback) -> Self {
        Self {
            end,
            sequence: 0,
            latest: None,
            prints: Vec::new(),
            level: None,
            seen: 0,
            acknowledge: false,
        }
    }

    /// Sends a command with `change` applied.
    fn send(&mut self, change: impl FnOnce(&mut UserCommand)) {
        self.sequence += 1;
        let mut command = UserCommand {
            sequence: self.sequence,
            #[expect(clippy::cast_possible_truncation, reason = "a tick fits f32")]
            seconds: TICK as f32,
            ..UserCommand::default()
        };
        change(&mut command);
        let seen = if self.acknowledge { self.seen } else { 0 };
        self.end
            .send(ClientMessage::Move(vec![command], seen).encode());
    }

    /// Reads everything waiting.
    fn read(&mut self) {
        while let Some(bytes) = self.end.receive() {
            match ServerMessage::decode(&bytes).expect("valid") {
                ServerMessage::Snapshot(snapshot) => {
                    self.seen = snapshot.tick;
                    self.latest = Some(snapshot);
                }
                ServerMessage::SnapshotDelta(delta) => {
                    let base = self.latest.as_ref().expect("a baseline");
                    assert_eq!(delta_baseline(&delta), Some(base.tick));
                    let snapshot = apply_delta(base, &delta).expect("it applies");
                    self.seen = snapshot.tick;
                    self.latest = Some(snapshot);
                }
                ServerMessage::Print(text) => {
                    assert!(!text.contains("script error"), "{text}");
                    self.prints.push(text);
                }
                ServerMessage::Level(name) => self.level = Some(name),
                _ => {}
            }
        }
    }

    /// Where they are.
    fn state(&self) -> PlayerState {
        self.latest.as_ref().expect("a snapshot").player
    }
}

/// A game with a host and one guest, who has loaded the level.
fn two_players() -> (Server, Player, Player) {
    let map = room();
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let (host, server_end) = loopback();
    let mut server = Server::new(
        Box::new(server_end),
        world,
        &map,
        Vec3::new(0.0, 0.0, 0.0),
        SCRIPTS,
        TICK,
        Level {
            name: "room".to_owned(),
            settings: vec![("skill".to_owned(), 1.0), ("deathmatch".to_owned(), 1.0)],
            ..Level::default()
        },
    )
    .expect("the scripts load");
    let (guest, guest_end) = loopback();
    let info = SeatInfo {
        name: "guest".to_owned(),
        colors: 0x4d,
    };
    server.add_seat(Seat::new(Box::new(guest_end), info));
    let mut guest = Player::new(guest);
    guest.read();
    assert_eq!(guest.level.as_deref(), Some("room"));
    guest.end.send(ClientMessage::Ready.encode());
    let mut host = Player::new(host);
    server.tick();
    host.read();
    guest.read();
    (server, host, guest)
}

#[test]
fn a_guest_gets_a_player_only_once_ready() {
    let map = room();
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let (_host, server_end) = loopback();
    let mut server = Server::new(
        Box::new(server_end),
        world,
        &map,
        Vec3::ZERO,
        SCRIPTS,
        TICK,
        Level::default(),
    )
    .expect("the scripts load");
    let (guest, guest_end) = loopback();
    server.add_seat(Seat::new(Box::new(guest_end), SeatInfo::default()));
    let mut guest = Player::new(guest);
    server.tick();
    guest.read();
    assert!(guest.latest.is_none());
    guest.end.send(ClientMessage::Ready.encode());
    server.tick();
    guest.read();
    assert!(guest.latest.is_some());
}

#[test]
fn each_player_has_their_own_entity_and_stats() {
    let (_server, host, guest) = two_players();
    let (one, two) = (
        host.latest.as_ref().expect("a snapshot"),
        guest.latest.as_ref().expect("a snapshot"),
    );
    assert_ne!(one.player_entity, two.player_entity);
    assert_eq!(two.stats[0], 100, "the guest's health");
}

#[test]
fn a_print_to_one_player_reaches_only_them() {
    let (mut server, mut host, mut guest) = two_players();
    server
        .exec(
            r#"
            local players = find_all("classname", "player")
            sprint(players[2], "for the guest")
            bprint("for everyone")
            "#,
        )
        .expect("it runs");
    server.tick();
    host.read();
    guest.read();
    assert_eq!(host.prints, ["for everyone"]);
    assert!(guest.prints.contains(&"for the guest".to_owned()));
    assert!(guest.prints.contains(&"for everyone".to_owned()));
}

#[test]
fn one_players_commands_dont_move_the_other() {
    let (mut server, mut host, mut guest) = two_players();
    server
        .exec(
            r#"
            local players = find_all("classname", "player")
            setorigin(players[1], vector.create(0, 0, 100))
            setorigin(players[2], vector.create(200, 0, 100))
            "#,
        )
        .expect("it runs");
    server.tick();
    guest.read();
    let start = guest.state().origin;
    for _ in 0..36 {
        host.send(|command| command.forward = 400.0);
        guest.send(|_| {});
        server.tick();
        host.read();
        guest.read();
    }
    assert!(host.state().origin.x > 10.0, "{:?}", host.state().origin);
    assert!((guest.state().origin.x - start.x).abs() < 0.01);
}

#[test]
fn only_the_host_can_cheat() {
    let (mut server, mut host, mut guest) = two_players();
    guest
        .end
        .send(ClientMessage::Console("noclip".to_owned()).encode());
    host.end
        .send(ClientMessage::Console("noclip".to_owned()).encode());
    server.tick();
    host.read();
    guest.read();
    assert!(host.state().noclip);
    assert!(!guest.state().noclip);
}

#[test]
fn a_client_cant_move_faster_by_sending_more() {
    let (mut server, _host, mut guest) = two_players();
    server
        .exec(r#"setorigin(find_all("classname", "player")[2], vector.create(0, 0, 100))"#)
        .expect("it runs");
    for _ in 0..200 {
        guest.send(|command| command.forward = 400.0);
    }
    server.tick();
    guest.read();
    let acknowledged = guest.latest.as_ref().expect("a snapshot").acknowledged;
    assert!(acknowledged < 30, "{acknowledged} commands ran at once");
}

#[test]
fn a_player_who_leaves_is_gone() {
    let (mut server, _host, guest) = two_players();
    assert_eq!(server.seat_count(), 2);
    drop(guest);
    // A loopback can't tell it's been dropped, so this is the timeout.
    for _ in 0..(72 * 11) {
        server.tick();
    }
    assert_eq!(server.seat_count(), 1);
    server
        .exec(r#"assert(#find_all("classname", "player") == 1)"#)
        .expect("one player is left");
}

#[test]
fn malformed_messages_are_ignored() {
    let (mut server, mut host, mut guest) = two_players();
    for bytes in [
        vec![],
        vec![255],
        vec![1, 0, 0, 0, 0, 200],
        vec![22, 1],
        vec![2, 0xFF, 0xFE],
    ] {
        guest.end.send(bytes);
    }
    guest.end.send(vec![22; 4000]);
    server.tick();
    host.read();
    assert!(host.latest.is_some());
}

/// A snapshot with a few things in it.
fn snapshot(tick: u32) -> Snapshot {
    Snapshot {
        tick,
        acknowledged: 3,
        player: PlayerState::default(),
        instances: vec![ModelInstance {
            model: 1,
            origin: Vec3::new(1.0, 2.0, 3.0),
            visible: true,
            solid: true,
            frame: 0,
        }],
        entities: (0..50)
            .map(|index| engine_core::entity::EntityState {
                id: index,
                model: 2,
                frame: 1,
                skin: 0,
                effects: 0,
                origin: Vec3::new(1.0, 2.0, 3.0),
                angles: Vec3::ZERO,
            })
            .collect(),
        boxes: vec![(Vec3::ZERO, Vec3::new(1.0, 1.0, 1.0))],
        weapon_model: 1,
        weapon_frame: 0,
        player_entity: 9,
        player_effects: 0,
        punch: Vec3::ZERO,
        time: 1.5,
        stats: vec![100, 0, 25],
    }
}

#[test]
fn a_delta_rebuilds_the_snapshot_exactly() {
    let base = snapshot(1);
    let mut next = snapshot(2);
    next.entities[4].origin = Vec3::new(9.0, 9.0, 9.0);
    next.entities[7].frame = 5;
    let _gone = next.entities.remove(20);
    next.entities.push(engine_core::entity::EntityState {
        id: 99,
        model: 3,
        ..engine_core::entity::EntityState::default()
    });
    next.instances[0].origin = Vec3::new(4.0, 5.0, 6.0);
    next.boxes[0].1 = Vec3::new(2.0, 2.0, 2.0);
    next.boxes.push((Vec3::ZERO, Vec3::ZERO));
    next.stats[0] = 50;
    next.time = 2.0;
    let bytes = delta_from(Some(&base), &next);
    assert_eq!(bytes[0], 18);
    assert_eq!(apply_delta(&base, &bytes[1..]).as_ref(), Some(&next));
}

#[test]
fn a_delta_of_nothing_is_small() {
    let (base, next) = (snapshot(1), snapshot(2));
    let delta = delta_from(Some(&base), &next);
    let full = delta_from(None, &next);
    assert!(
        delta.len() * 10 < full.len(),
        "{} of {}",
        delta.len(),
        full.len()
    );
}

#[test]
fn a_delta_against_the_wrong_snapshot_is_refused() {
    let (base, next) = (snapshot(1), snapshot(2));
    let delta = delta_from(Some(&base), &next);
    assert_eq!(apply_delta(&snapshot(7), &delta[1..]), None);
}

#[test]
fn a_cut_short_delta_is_refused() {
    let (base, next) = (snapshot(1), snapshot(2));
    let delta = delta_from(Some(&base), &next);
    for length in 1..delta.len() {
        assert_eq!(apply_delta(&base, &delta[1..length]), None);
    }
}

#[test]
fn acknowledged_snapshots_become_deltas() {
    let (mut server, mut host, mut guest) = two_players();
    guest.acknowledge = true;
    let mut sent = 0;
    for _ in 0..10 {
        guest.send(|_| {});
        server.tick();
        host.read();
        guest.read();
        sent += 1;
    }
    assert_eq!(sent, 10);
    assert_eq!(
        guest.latest.as_ref().expect("a snapshot").stats[0],
        100,
        "deltas keep the numbers"
    );
}

#[test]
fn names_and_colours_can_change() {
    let (mut server, _host, mut guest) = two_players();
    let info = PlayerInfo {
        name: "renamed".to_owned(),
        colors: 0x12,
    };
    guest.end.send(ClientMessage::Info(info).encode());
    server.tick();
    let mut scores = None;
    while let Some(bytes) = guest.end.receive() {
        if let Some(ServerMessage::Scores(list)) = ServerMessage::decode(&bytes) {
            scores = Some(list);
        }
    }
    let scores = scores.expect("the scores changed");
    assert_eq!(scores.len(), 2);
    assert_eq!(scores[1].name, "renamed");
    assert_eq!(scores[1].colors, 0x12);
}

#[test]
fn a_sound_is_each_listeners_own_only_if_it_comes_from_them() {
    let (mut server, mut host, mut guest) = two_players();
    server
        .exec(r#"sound(find_all("classname", "player")[2], CHAN_BODY, "x.wav", 1, ATTN_NORM)"#)
        .expect("it runs");
    server.tick();
    let own = |player: &mut Player| {
        std::iter::from_fn(|| player.end.receive())
            .find_map(|bytes| match ServerMessage::decode(&bytes) {
                Some(ServerMessage::Sound(sound)) => Some(sound.own),
                _ => None,
            })
            .expect("a sound")
    };
    assert!(!own(&mut host));
    assert!(own(&mut guest));
}
