//! Playing a map straight from its text, with the fixture's scripts.

use std::rc::Rc;
use std::sync::Arc;

use engine_compile::{compile_text, placeholder_texture};
use engine_core::math::Vec3;
use engine_core::player::UserCommand;
use engine_map::Map;
use engine_net::{ClientMessage, Loopback, ServerMessage, Transport, loopback};
use engine_physics::CollisionWorld;
use engine_server::{Level, Server};
use fixture_quake::{SCRIPTS, spawn_point};

/// A room with a start, and a door a trigger opens.
const MAP: &str = r#"
format 1 map
brush { material = floor cuboid = -320 -320 -32 320 320 0 }
brush { material = ceiling cuboid = -320 -320 256 320 320 288 }
brush { material = wall cuboid = -320 -320 0 -288 320 256 }
brush { material = wall cuboid = 288 -320 0 320 320 256 }
brush { material = wall cuboid = -288 -320 0 288 -288 256 }
brush { material = wall cuboid = -288 288 0 288 320 256 }
entity start { type = info_player_start origin = 0 0 24 }
entity door_1 {
    type = func_door
    angle = -1
    lip = 8
    speed = 200
    wait = -1
    brush { material = door cuboid = 128 -64 0 144 64 128 }
}
"#;

/// The compiled map.
fn map() -> Arc<Map> {
    let mut source = |name: &str| placeholder_texture(name);
    Arc::new(compile_text(MAP, &mut source).expect("compiles"))
}

/// A server on the map, with its client's end.
fn start() -> (Server, Loopback, Vec3) {
    let map = map();
    let origin = spawn_point(&map).origin;
    let world = Rc::new(CollisionWorld::new(map));
    let (client, server_end) = loopback();
    let server = Server::new(
        Box::new(server_end),
        world,
        origin,
        SCRIPTS,
        1.0 / 72.0,
        Level {
            name: "test".to_owned(),
            settings: vec![("skill".to_owned(), 1.0), ("registered".to_owned(), 1.0)],
            ..Level::default()
        },
    )
    .expect("the scripts load");
    (server, client, origin)
}

/// Runs `ticks` of `forward` speed, and returns the last position.
fn run(server: &mut Server, client: &mut Loopback, ticks: u32, forward: f32) -> Vec3 {
    let mut position = Vec3::ZERO;
    for sequence in 1..=ticks {
        let command = UserCommand {
            sequence,
            seconds: 1.0 / 72.0,
            forward,
            ..UserCommand::default()
        };
        client.send(ClientMessage::Move(vec![command], 0).encode());
        server.tick();
        while let Some(bytes) = client.receive() {
            if let Some(ServerMessage::Snapshot(snapshot)) = ServerMessage::decode(&bytes) {
                position = snapshot.player.origin;
            }
        }
    }
    position
}

#[test]
fn a_player_stands_on_a_compiled_floor() {
    let (mut server, mut client, _start) = start();
    let position = run(&mut server, &mut client, 72, 0.0);
    assert!(
        (position.z - 24.0).abs() < 1.0,
        "feet on the floor at z 0: {position:?}"
    );
}

#[test]
fn a_player_is_stopped_by_a_compiled_wall() {
    let (mut server, mut client, _start) = start();
    let position = run(&mut server, &mut client, 72 * 4, 400.0);
    assert!(position.x > 50.0, "walked forward: {position:?}");
    assert!(
        position.x < 288.0 - 15.0,
        "and stopped short of the wall: {position:?}"
    );
}
