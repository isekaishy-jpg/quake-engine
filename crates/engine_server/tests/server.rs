//! The server, spoken to through its transport.

mod common;

use std::rc::Rc;
use std::sync::Arc;

use common::room;
use engine_core::math::Vec3;
use engine_core::player::UserCommand;
use engine_map::{Entity, Map, NavGrid, NavPlace, Spline};
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

/// Things that say when they think and wake, one of them never sleeping.
const SLEEPERS: &str = r#"
function ticker(self)
    self.nextthink = 0.1
    self.think = function(me)
        engine.print("tick")
        me.nextthink = engine.time() + 0.1
    end
    self.wake = function(me)
        engine.print("woke " .. tostring(engine.slept > 0))
    end
end

function worker(self)
    self.flags = 32768
    self.nextthink = 0.1
    self.think = function(me)
        engine.print("work")
        me.nextthink = engine.time() + 0.1
    end
end
"#;

/// What's said over `ticks` of a player running forward, from `first`.
fn said(
    server: &mut Server,
    client: &mut Loopback,
    first: u32,
    ticks: u32,
    run: bool,
) -> Vec<String> {
    let mut lines = Vec::new();
    for sequence in first..first + ticks {
        if run {
            client.send(forward(sequence).encode());
        }
        server.tick();
        while let Some(bytes) = client.receive() {
            if let Some(ServerMessage::Print(line)) = ServerMessage::decode(&bytes) {
                lines.push(line);
            }
        }
    }
    lines
}

/// A map with an entity of `kind` a short way along the floor from the start.
fn map_with(kinds: &[(&str, &str)]) -> Arc<Map> {
    let mut map = (*room()).clone();
    map.wake_reach = 100.0;
    for (kind, origin) in kinds {
        map.entities.push(Entity {
            fields: vec![
                ("classname".to_owned(), (*kind).to_owned()),
                ("origin".to_owned(), (*origin).to_owned()),
            ],
        });
    }
    Arc::new(map)
}

/// What's out of reach sleeps, and wakes knowing it slept when a player comes.
/// One told never to sleep carries on all along.
#[test]
fn entities_sleep_out_of_reach_and_wake_when_it_comes_back() {
    let map = map_with(&[("ticker", "180 0 -240"), ("worker", "-180 0 -240")]);
    let world = Rc::new(CollisionWorld::new(Arc::clone(&map)));
    let (mut client, server_end) = loopback();
    let origin = Vec3::new(0.0, 0.0, -255.0);
    let mut server = Server::new(
        Box::new(server_end),
        world,
        origin,
        &[("sleepers", SLEEPERS)],
        1.0 / 72.0,
        Level::default(),
    )
    .expect("server");
    let still = said(&mut server, &mut client, 1, 72, false);
    assert!(!still.iter().any(|line| line.contains("tick")), "{still:?}");
    assert!(still.iter().any(|line| line.contains("work")), "{still:?}");
    let moved = said(&mut server, &mut client, 1, 72, true);
    assert!(
        moved.iter().any(|line| line.contains("woke true")),
        "{moved:?}"
    );
    assert!(moved.iter().any(|line| line.contains("tick")), "{moved:?}");
}

/// A map that isn't tiled has nothing asleep.
#[test]
fn nothing_sleeps_with_no_reach_set() {
    let mut map = (*map_with(&[("ticker", "900 0 -240")])).clone();
    map.wake_reach = 0.0;
    let world = Rc::new(CollisionWorld::new(Arc::new(map)));
    let (mut client, server_end) = loopback();
    let mut server = Server::new(
        Box::new(server_end),
        world,
        Vec3::new(0.0, 0.0, -255.0),
        &[("sleepers", SLEEPERS)],
        1.0 / 72.0,
        Level::default(),
    )
    .expect("server");
    let still = said(&mut server, &mut client, 1, 72, false);
    assert!(still.iter().any(|line| line.contains("tick")), "{still:?}");
}

/// Scripts can ask for a way across a map, and a point on a spline.
#[test]
fn scripts_find_paths_and_follow_splines() {
    let script = r#"
    function asker(self)
        local path = engine.find_path(vector.create(0, 0, 0), vector.create(32, 0, 0))
        engine.print("path " .. #path .. " " .. path[#path].x)
                engine.print("spline " .. engine.spline_point("line", 1).x)
        engine.print("no spline " .. tostring(engine.spline_point("none", 1) == nil))
        engine.print("seeded " .. tostring(engine.seeded(self, 2) == engine.seeded(self, 2)))
        engine.print("walking " .. tostring(select(2, engine.walk_to(self, vector.create(32, 0, 0), 4))))
    end
    "#;
    let mut map = (*map_with(&[("asker", "0 0 -240")])).clone();
    map.wake_reach = 0.0;
    let mut first = NavPlace::at(Vec3::ZERO);
    let mut second = NavPlace::at(Vec3::new(16.0, 0.0, 0.0));
    let third = NavPlace::at(Vec3::new(32.0, 0.0, 0.0));
    first.links[0] = 1;
    second.links[0] = 2;
    map.nav = Some(NavGrid {
        origin: Vec3::ZERO,
        spacing: 16.0,
        size: [3, 1],
        columns: vec![0, 1, 2, 3],
        places: vec![first, second, third],
    });
    map.splines.push(Spline {
        name: "line".to_owned(),
        points: vec![Vec3::ZERO, Vec3::new(40.0, 0.0, 0.0)],
        closed: false,
    });
    let world = Rc::new(CollisionWorld::new(Arc::new(map)));
    let (mut client, server_end) = loopback();
    let mut server = Server::new(
        Box::new(server_end),
        world,
        Vec3::new(0.0, 0.0, -255.0),
        &[("asker", script)],
        1.0 / 72.0,
        Level::default(),
    )
    .expect("server");
    let lines = said(&mut server, &mut client, 1, 2, false).join("\n");
    assert!(lines.contains("path 2 32"), "{lines}");
    assert!(lines.contains("spline 40"), "{lines}");
    assert!(lines.contains("no spline true"), "{lines}");
    assert!(lines.contains("seeded true"), "{lines}");
    assert!(lines.contains("walking true"), "{lines}");
}

/// A brush entity far from the origin wakes by where it stands, not by its
/// origin, which is only how far it's moved.
#[test]
fn a_brush_entity_sleeps_by_its_bounds() {
    let script = r#"
    function mover(self)
        self.mins = vector.create(150, -8, -250)
        self.maxs = vector.create(170, 8, -230)
        self.nextthink = 0.1
        self.think = function(me)
            engine.print("tick")
            me.nextthink = engine.time() + 0.1
        end
    end
    "#;
    let map = map_with(&[("mover", "0 0 0")]);
    let world = Rc::new(CollisionWorld::new(map));
    let (mut client, server_end) = loopback();
    let mut server = Server::new(
        Box::new(server_end),
        world,
        Vec3::new(160.0, 0.0, -255.0),
        &[("mover", script)],
        1.0 / 72.0,
        Level::default(),
    )
    .expect("server");
    let lines = said(&mut server, &mut client, 1, 72, false);
    assert!(lines.iter().any(|line| line.contains("tick")), "{lines:?}");
}
