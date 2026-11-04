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
    map(
        &[],
        &[
            &[("classname", "worldspawn")],
            &[("classname", "info_player_start"), ("origin", "0 0 0")],
            &[
                ("classname", "info_player_deathmatch"),
                ("origin", "-100 0 -200"),
            ],
            &[
                ("classname", "info_player_deathmatch"),
                ("origin", "100 0 -200"),
            ],
        ],
    )
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
                    if !text.ends_with("entered the game") {
                        self.prints.push(text);
                    }
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

/// Shoots `target` dead as `killer`.
const FIRST_KILLS_SECOND: &str = r#"
    local players = find_all("classname", "player")
    players[1].weapon = IT_ROCKET_LAUNCHER
    T_Damage(players[2], players[1], players[1], 500)
"#;

#[test]
fn a_kill_scores_a_frag_and_is_announced() {
    let (mut server, mut host, mut guest) = two_players();
    server.exec(FIRST_KILLS_SECOND).expect("it runs");
    server.tick();
    host.read();
    guest.read();
    assert_eq!(host.latest.as_ref().expect("a snapshot").stats[1], 1);
    assert!(
        host.prints
            .iter()
            .any(|text| text.contains("guest was gibbed by player")),
        "{:?}",
        host.prints
    );
}

#[test]
fn the_dead_respawn_with_a_new_start_and_leave_a_body() {
    let (mut server, mut host, mut guest) = two_players();
    server.exec(FIRST_KILLS_SECOND).expect("it runs");
    for _ in 0..144 {
        host.send(|_| {});
        guest.send(|_| {});
        server.tick();
        host.read();
        guest.read();
    }
    assert!(guest.state().dead);
    for held in [false, true, false] {
        for _ in 0..8 {
            host.send(|_| {});
            guest.send(|command| command.attack = held);
            server.tick();
            host.read();
            guest.read();
        }
    }
    assert!(!guest.state().dead, "they're back");
    assert_eq!(guest.latest.as_ref().expect("a snapshot").stats[0], 100);
    assert!(server.take_next_level().is_none(), "the level goes on");
}

#[test]
fn a_fraglimit_ends_the_level() {
    let map = room();
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let (host, server_end) = loopback();
    let mut server = Server::new(
        Box::new(server_end),
        world,
        &map,
        Vec3::ZERO,
        SCRIPTS,
        TICK,
        Level {
            name: "room".to_owned(),
            settings: vec![
                ("deathmatch".to_owned(), 1.0),
                ("fraglimit".to_owned(), 2.0),
            ],
            ..Level::default()
        },
    )
    .expect("the scripts load");
    let mut host = Player::new(host);
    server
        .exec(r#"find_all("classname", "player")[1].frags = 2"#)
        .expect("it runs");
    for _ in 0..30 {
        host.send(|_| {});
        server.tick();
        host.read();
    }
    server
        .exec("assert(intermission_running == 1)")
        .expect("the level ended");
}

#[test]
fn items_come_back_in_deathmatch() {
    let map = map(
        &[],
        &[
            &[("classname", "worldspawn")],
            &[
                ("classname", "info_player_deathmatch"),
                ("origin", "0 0 -200"),
            ],
            &[("classname", "item_shells"), ("origin", "0 0 -240")],
        ],
    );
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let (host, server_end) = loopback();
    let mut server = Server::new(
        Box::new(server_end),
        world,
        &map,
        Vec3::ZERO,
        SCRIPTS,
        TICK,
        Level {
            settings: vec![("deathmatch".to_owned(), 1.0)],
            ..Level::default()
        },
    )
    .expect("the scripts load");
    let mut host = Player::new(host);
    let visible = |host: &Player| {
        let snapshot = host.latest.as_ref().expect("a snapshot");
        snapshot
            .entities
            .iter()
            .filter(|entity| entity.id != snapshot.player_entity)
            .count()
    };
    for _ in 0..72 {
        host.send(|_| {});
        server.tick();
        host.read();
    }
    assert_eq!(visible(&host), 0, "it's been taken");
    server
        .exec(r#"setorigin(find_all("classname", "player")[1], vector.create(200, 200, -200))"#)
        .expect("it runs");
    for _ in 0..(72 * 31) {
        host.send(|_| {});
        server.tick();
        host.read();
    }
    assert!(visible(&host) >= 1, "it's back");
}

#[test]
fn only_the_host_saves_nothing_goes_wrong_with_a_second_player_in_the_list() {
    let (mut server, mut host, mut guest) = two_players();
    for _ in 0..30 {
        host.send(|_| {});
        guest.send(|_| {});
        server.tick();
        host.read();
        guest.read();
    }
    assert!(server.save().is_ok());
}

/// Fires a line along x from the first player, as they saw the world
/// `seen_ago` seconds back, at a second player who left that line
/// `left_ticks_ago`, and says whether it hit.
fn shot_lands(left_ticks_ago: u32, seen_ago: f64) -> bool {
    let (mut server, mut host, mut guest) = two_players();
    server
        .exec(
            r#"
            local players = find_all("classname", "player")
            setorigin(players[1], vector.create(-200, 0, -255))
            setorigin(players[2], vector.create(200, 0, -255))
            shooter, target = players[1], players[2]
            "#,
        )
        .expect("it runs");
    // A while with the target in the shooter's line, then gone.
    for _ in 0..72 {
        host.send(|_| {});
        guest.send(|_| {});
        server.tick();
        host.read();
        guest.read();
    }
    server
        .exec(r#"setorigin(target, vector.create(200, 200, -255))"#)
        .expect("it runs");
    for _ in 0..left_ticks_ago {
        host.send(|_| {});
        guest.send(|_| {});
        server.tick();
        host.read();
        guest.read();
    }
    let now = host.latest.as_ref().expect("a snapshot").time;
    host.send(|command| command.view_time = now - seen_ago);
    guest.send(|_| {});
    server
        .exec(
            r#"
            hit_something = false
            local real = target.origin
            "#,
        )
        .expect("it runs");
    server.tick();
    server
        .exec(
            r#"
            local id = rawget(shooter, "_id")
            engine.rewind(id)
            traceline(shooter.origin, shooter.origin + vector.create(800, 0, 0), false, shooter)
            hit_something = trace_ent == target
            engine.end_rewind()
            assert(vector.magnitude(target.origin - vector.create(200, 200, -255)) < 40)
            "#,
        )
        .expect("it runs");
    server.exec("assert(hit_something)").is_ok()
}

#[test]
fn a_shot_hits_where_the_shooter_saw_the_target() {
    assert!(shot_lands(3, 0.1));
}

#[test]
fn a_shot_at_now_misses_a_target_thats_moved() {
    assert!(!shot_lands(3, 0.0));
}

#[test]
fn a_view_too_old_is_limited() {
    assert!(shot_lands(14, 0.22), "within the limit");
    assert!(
        !shot_lands(29, 0.6),
        "the view is cut back to a quarter of a second"
    );
}
