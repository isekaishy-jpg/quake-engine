//! Something really going off edges, over gaps, through water and up lifts.

use std::rc::Rc;
use std::sync::Arc;

use engine_compile::{
    BakeInput, CompileError, bake_light, compile, placeholder_texture, read_map_text,
};
use engine_core::math::Vec3;
use engine_map::{Map, MipLevel};
use engine_net::{Loopback, ServerMessage, Transport, loopback};
use engine_physics::CollisionWorld;
use engine_server::{Level, Server};
use fixture_quake::{SCRIPTS, spawn_point};

/// A thing that walks to its goal, as a monster or NPC would.
const WALKER: &str = r#"
function walker_think(self)
    engine.walk_to(self, self.goal, 12)
    self.nextthink = time() + 0.05
end

function walker(self)
    self.solid = SOLID_SLIDEBOX
    self.health = 100
    self.movetype = MOVETYPE_STEP
    setsize(self, vector.create(-16, -16, -24), vector.create(16, 16, 40))
    self.goal = engine.parse_vector(self.goal)
    self.think = walker_think
    self.nextthink = time() + 0.5
end
"#;

/// A map from `text`, baked.
fn baked(text: &str) -> Arc<Map> {
    let working = read_map_text(text).expect("reads");
    let mut source = |name: &str| placeholder_texture(name);
    let mut map = compile(&working, &mut source).expect("compiles");
    let mut images =
        |_: &str| -> Result<Arc<Vec<MipLevel>>, CompileError> { Ok(Arc::new(Vec::new())) };
    let mut input = BakeInput {
        working: &working,
        images: &mut images,
    };
    bake_light(&mut map, &mut input).expect("bakes");
    Arc::new(map)
}

/// A closed room from -1024 to 1024 with `extra` in it, and a walker from `from` going to
/// `to`. The player is out of the way, on a ledge of their own.
fn room(extra: &str, from: [i32; 3], to: [i32; 3]) -> Arc<Map> {
    let [fx, fy, fz] = from;
    let [tx, ty, tz] = to;
    baked(&format!(
        r#"format 1 map
entity world {{ type = worldspawn light_spacing = 64 }}
brush {{ material = floor cuboid = -1056 -320 -320 1056 320 -288 }}
brush {{ material = roof cuboid = -1056 -320 512 1056 320 544 }}
brush {{ material = wall cuboid = -1056 -320 -288 -1024 320 512 }}
brush {{ material = wall cuboid = 1024 -320 -288 1056 320 512 }}
brush {{ material = wall cuboid = -1024 -320 -288 1024 -288 512 }}
brush {{ material = wall cuboid = -1024 288 -288 1024 320 512 }}
entity start {{ type = info_player_start origin = -960 240 400 }}
brush {{ material = perch cuboid = -1000 200 360 -920 280 376 }}
entity {{ type = walker origin = {fx} {fy} {fz} goal = "{tx} {ty} {tz}" }}
{extra}"#
    ))
}

/// Where the walker is after `seconds`, and the furthest it fell behind.
fn walk(map: &Arc<Map>, seconds: u32) -> Vec3 {
    let world = Rc::new(CollisionWorld::new(Arc::clone(map)));
    let (mut client, server_end): (Loopback, _) = loopback();
    let mut server = Server::new(
        Box::new(server_end),
        world,
        spawn_point(map).origin,
        SCRIPTS,
        1.0 / 72.0,
        Level {
            settings: vec![("skill".to_owned(), 1.0), ("registered".to_owned(), 1.0)],
            scripts: vec![("walker".to_owned(), WALKER.to_owned())],
            ..Level::default()
        },
    )
    .expect("the scripts load");
    let mut told = Vec::new();
    for tick in 0..seconds * 72 {
        server.tick();
        if tick + 1 == seconds * 72 {
            server
                .exec(
                    r#"local o = find_all("classname", "walker")[1].origin
                    engine.print("AT " .. o.x .. " " .. o.y .. " " .. o.z)"#,
                )
                .expect("asks");
            server.tick();
        }
        while let Some(bytes) = client.receive() {
            if let Some(ServerMessage::Print(text)) = ServerMessage::decode(&bytes) {
                assert!(!text.contains("script error"), "{text}");
                told.push(text);
            }
        }
    }
    let line = told
        .iter()
        .find(|line| line.starts_with("AT "))
        .expect("told");
    let parts: Vec<f32> = line[3..]
        .split(' ')
        .filter_map(|word| word.trim().parse().ok())
        .collect();
    Vec3::new(parts[0], parts[1], parts[2])
}

/// Whether `at` is about at `goal`.
fn arrived(at: Vec3, goal: [f32; 3]) -> bool {
    let [x, y, z] = goal;
    (at - Vec3::new(x, y, z)).length() < 64.0
}

#[test]
fn it_goes_through_a_door_with_a_name() {
    let map = room(
        "brush { material = floor cuboid = -1024 -288 -288 1024 288 0 }
         brush { material = wall cuboid = -16 -288 0 16 -64 512 }
         brush { material = wall cuboid = -16 64 0 16 288 512 }
         brush { material = wall cuboid = -16 -64 128 16 64 512 }
         entity hall_door { type = func_door angle = -1 speed = 200 wait = 3
             brush { material = door cuboid = -8 -64 0 8 64 128 }
         }",
        [-300, 0, 26],
        [300, 0, 24],
    );
    let at = walk(&map, 8);
    assert!(arrived(at, [300.0, 0.0, 24.0]), "{at:?}");
}

#[test]
fn it_goes_off_an_edge() {
    let map = room(
        "brush { material = shelf cuboid = -1024 -288 -288 0 288 96 }
         brush { material = floor cuboid = 0 -288 -288 1024 288 0 }",
        [-300, 0, 122],
        [300, 0, 24],
    );
    let at = walk(&map, 10);
    assert!(arrived(at, [300.0, 0.0, 24.0]), "{at:?}");
}

#[test]
fn it_jumps_a_gap() {
    let map = room(
        "brush { material = floor cuboid = -1024 -288 -288 -64 288 0 }
         brush { material = floor cuboid = 64 -288 -288 1024 288 0 }",
        [-300, 0, 26],
        [300, 0, 24],
    );
    let at = walk(&map, 10);
    assert!(arrived(at, [300.0, 0.0, 24.0]), "{at:?}");
}

#[test]
fn it_swims_a_pool() {
    let map = room(
        "brush { material = floor cuboid = -1024 -288 -288 -128 288 0 }
         brush { material = floor cuboid = 128 -288 -288 1024 288 0 }
         brush { material = floor cuboid = -128 -288 -288 128 288 -144 }
         brush { material = water contents = water cuboid = -128 -288 -144 128 288 -16 }",
        [-300, 0, 26],
        [300, 0, 24],
    );
    let at = walk(&map, 15);
    assert!(arrived(at, [300.0, 0.0, 24.0]), "{at:?}");
}

#[test]
fn it_rides_a_lift() {
    let map = room(
        "brush { material = floor cuboid = -1024 -288 -288 1024 288 0 }
         brush { material = shelf cuboid = -1024 -288 0 -64 288 96 }
         entity { type = func_plat height = 88
             brush { material = lift cuboid = -64 -64 88 64 64 96 }
         }",
        [300, 0, 26],
        [-300, 0, 120],
    );
    let at = walk(&map, 20);
    assert!(arrived(at, [-300.0, 0.0, 120.0]), "{at:?}");
}
