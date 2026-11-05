//! Playing a map straight from its text, with the fixture's scripts.

use std::rc::Rc;
use std::sync::Arc;

use engine_compile::{
    BakeInput, CompileError, bake_light, compile, compile_text, placeholder_texture, read_map_text,
};
use engine_core::math::Vec3;
use engine_core::player::UserCommand;
use engine_map::{Map, MipLevel};
use engine_net::{ClientMessage, Loopback, ServerMessage, Transport, loopback};
use engine_physics::CollisionWorld;
use engine_server::{Level, Server, Sight};
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

/// Two rooms with a wall between them.
const TWO_ROOMS: &str = "format 1 map
brush { material = floor cuboid = -1024 -256 -32 1024 256 0 }
brush { material = roof cuboid = -1024 -256 256 1024 256 288 }
brush { material = wall cuboid = -1056 -288 -32 -1024 288 288 }
brush { material = wall cuboid = 1024 -288 -32 1056 288 288 }
brush { material = wall cuboid = -1024 -288 -32 1024 -256 288 }
brush { material = wall cuboid = -1024 256 -32 1024 288 288 }
brush { material = wall cuboid = -32 -256 0 32 256 256 }
";

/// The two rooms, baked.
fn two_rooms() -> Map {
    let working = read_map_text(TWO_ROOMS).expect("reads");
    let mut source = |name: &str| placeholder_texture(name);
    let mut map = compile(&working, &mut source).expect("compiles");
    let mut images = |path: &str| -> Result<Arc<Vec<MipLevel>>, CompileError> {
        Err(CompileError::Bad(path.to_owned()))
    };
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light(&mut map, &mut input).expect("bakes");
    map
}

/// What's behind a wall can't be seen, what's in the room can, and what's near
/// always can, even through the wall.
#[test]
fn sight_stops_at_walls_but_not_up_close() {
    let map = two_rooms();
    let eye = Vec3::new(-800.0, 0.0, 64.0);
    let sight = Sight::from(&map.tree, eye, eye).expect("the map has visible sets");
    let size = Vec3::new(16.0, 16.0, 24.0);
    let at = |x: f32| {
        let middle = Vec3::new(x, 0.0, 64.0);
        (middle - size, middle + size)
    };
    let (low, high) = at(-500.0);
    assert!(sight.shows(low, high), "in the room");
    let (low, high) = at(800.0);
    assert!(!sight.shows(low, high), "behind the wall");
    let near_wall = Vec3::new(-100.0, 0.0, 64.0);
    let sight = Sight::from(&map.tree, near_wall, near_wall).expect("sets");
    let (low, high) = at(120.0);
    assert!(sight.shows(low, high), "close by, through the wall");
}

/// With no visible sets there's nothing to cull by.
#[test]
fn a_map_with_no_sets_shows_everything() {
    let mut source = |name: &str| placeholder_texture(name);
    let map = compile_text(TWO_ROOMS, &mut source).expect("compiles");
    assert!(Sight::from(&map.tree, Vec3::ZERO, Vec3::ZERO).is_none());
}
