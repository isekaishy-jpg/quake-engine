//! Compiling brushes into something the engine collides with and draws.

use std::sync::Arc;

use engine_compile::{compile_text, placeholder_texture};
use engine_core::math::Vec3;
use engine_map::{Contents, Map, NodeChild, PoolLook};
use engine_physics::CollisionWorld;

/// A closed room from -256 to 256 with 32 unit walls, and a pillar.
const ROOM: &str = r#"
format 1 map
brush { material = floor cuboid = -288 -288 -288 288 288 -256 }
brush { material = ceiling cuboid = -288 -288 256 288 288 288 }
brush { material = wall cuboid = -288 -288 -256 -256 288 256 }
brush { material = wall cuboid = 256 -288 -256 288 288 256 }
brush { material = wall cuboid = -256 -288 -256 256 -256 256 }
brush { material = wall cuboid = -256 256 -256 256 288 256 }
brush { material = pillar cuboid = -32 -32 -256 32 32 256 }
brush { contents = water material = "*water" cuboid = 96 96 -256 224 224 -128 }
entity start { type = info_player_start origin = 128 0 -232 }
entity door_1 {
    type = func_door
    speed = 100
    brush { material = door cuboid = 128 -64 -256 144 64 -128 }
}
"#;

/// Compiles the room.
fn room() -> Arc<Map> {
    let mut source = |name: &str| placeholder_texture(name);
    Arc::new(compile_text(ROOM, &mut source).expect("compiles"))
}

/// The contents at `point`, walking the world's point tree.
fn contents_at(map: &Map, point: Vec3) -> Contents {
    let mut child = map.models[0].root.expect("a tree");
    loop {
        match child {
            NodeChild::Leaf(leaf) => return map.tree.leaves[leaf as usize].contents,
            NodeChild::Node(node) => {
                let node = map.tree.nodes[node as usize];
                let plane = map.tree.planes[node.plane as usize];
                let side = plane.normal.dot(point) - plane.distance;
                child = node.children[usize::from(side < 0.0)];
            }
        }
    }
}

#[test]
fn the_point_tree_knows_what_fills_space() {
    let map = room();
    assert_eq!(
        contents_at(&map, Vec3::new(100.0, -100.0, 0.0)),
        Contents::Empty
    );
    assert_eq!(
        contents_at(&map, Vec3::new(0.0, 0.0, 0.0)),
        Contents::Solid,
        "pillar"
    );
    assert_eq!(
        contents_at(&map, Vec3::new(-270.0, 0.0, 0.0)),
        Contents::Solid,
        "wall"
    );
    assert_eq!(
        contents_at(&map, Vec3::new(160.0, 160.0, -200.0)),
        Contents::Water
    );
    assert_eq!(
        contents_at(&map, Vec3::new(160.0, 160.0, -100.0)),
        Contents::Empty
    );
}

#[test]
fn a_player_stops_where_their_box_touches_a_wall() {
    let world = CollisionWorld::new(room());
    let start = Vec3::new(-100.0, -100.0, 0.0);
    let trace = world.trace(start, Vec3::new(-400.0, -100.0, 0.0));
    assert!(trace.fraction < 1.0, "it hit");
    assert!(
        (trace.end.x - -240.0).abs() < 0.1,
        "stops 16 from the wall: {}",
        trace.end.x
    );
    let down = world.trace(start, Vec3::new(-100.0, -100.0, -400.0));
    assert!(
        (down.end.z - -232.0).abs() < 0.1,
        "feet on the floor: {}",
        down.end.z
    );
    assert!(!world.in_solid(start), "the middle is open");
    assert!(world.in_solid(Vec3::new(0.0, 0.0, 0.0)), "the pillar isn't");
    assert!(world.in_solid(Vec3::new(40.0, 0.0, 0.0)), "nor its edge");
    assert!(
        !world.in_solid(Vec3::new(50.0, 0.0, 0.0)),
        "past its edge is"
    );
}

#[test]
fn hidden_faces_are_removed() {
    let map = room();
    let world_faces = map.models[0].face_count as usize;
    for (index, face) in map.faces.iter().take(world_faces).enumerate() {
        let first = face.first_point as usize;
        let points = &map.points[first..first + face.point_count as usize];
        let share = 1.0 / points.len() as f32;
        let middle = points.iter().fold(Vec3::ZERO, |sum, p| sum + *p) * share;
        let normal = (points[1] - points[0])
            .cross(points[2] - points[1])
            .normalized();
        let front = middle + normal * 0.5;
        assert_ne!(
            contents_at(&map, front),
            Contents::Solid,
            "face {index} at {middle:?} is buried"
        );
    }
    let pillar = map
        .textures
        .iter()
        .position(|texture| texture.name == "pillar")
        .and_then(|index| u32::try_from(index).ok())
        .expect("a pillar");
    let sides = map.faces.iter().filter(|f| f.texture == pillar).count();
    assert_eq!(sides, 4, "top and bottom meet the ceiling and floor");
}

#[test]
fn a_brush_entity_is_its_own_model() {
    let map = room();
    assert_eq!(map.models.len(), 2);
    let door = map.entity_of_class("func_door").expect("a door");
    assert_eq!(door.get("model"), Some("*1"));
    assert_eq!(door.get("targetname"), Some("door_1"));
    assert_eq!(door.get("speed"), Some("100"));
    let model = &map.models[1];
    assert!(model.face_count >= 6);
    assert_eq!(model.mins.x, 128.0);
    assert_eq!(model.maxs.z, -128.0);
    assert_eq!(map.entities[0].get("classname"), Some("worldspawn"));
}

#[test]
fn a_sloped_brush_collides_where_it_should() {
    let text = "format 1 map\nbrush {\n material = ramp\n face { normal = 1 0 1 distance = 0 }\n face { normal = -1 0 0 distance = 128 }\n face { normal = 0 0 -1 distance = 128 }\n face { normal = 0 1 0 distance = 128 }\n face { normal = 0 -1 0 distance = 128 }\n}\n";
    let mut source = |name: &str| placeholder_texture(name);
    let map = Arc::new(compile_text(text, &mut source).expect("compiles"));
    let world = CollisionWorld::new(map);
    let trace = world.trace(Vec3::new(-64.0, 0.0, 200.0), Vec3::new(-64.0, 0.0, -100.0));
    assert!(trace.fraction < 1.0, "it hit the slope");
    assert!(
        trace.normal.z > 0.5 && trace.normal.x > 0.5,
        "{:?}",
        trace.normal
    );
}

#[test]
fn bad_maps_say_what_is_wrong() {
    let mut source = |name: &str| placeholder_texture(name);
    let cases = [
        "format 1 material\n",
        "format 1 map\nbrush { cuboid = 1 2 3 }\n",
        "format 1 map\nbrush { }\n",
        "format 1 map\nbrush { contents = soup cuboid = 0 0 0 1 1 1 }\n",
        "format 1 map\nentity x { }\n",
    ];
    for text in cases {
        assert!(compile_text(text, &mut source).is_err(), "{text}");
    }
}

/// Liquid brushes are pools, and solid ones aren't.
#[test]
fn liquid_brushes_become_pools() {
    let map = room();
    assert_eq!(map.pools.len(), 1);
    let pool = map.pools[0];
    assert_eq!(pool.low, Vec3::new(96.0, 96.0, -256.0));
    assert_eq!(pool.high, Vec3::new(224.0, 224.0, -128.0));
    assert_eq!(Some(pool.look), PoolLook::of(Contents::Water));
}

/// A map can change how its liquids look.
#[test]
fn a_water_block_changes_the_look() {
    let text = format!(
        "{ROOM}
water {{ caustics = 0.25 absorption = 0.5 }}
"
    );
    let mut source = |name: &str| placeholder_texture(name);
    let map = compile_text(&text, &mut source).expect("compiles");
    let usual = PoolLook::of(Contents::Water).expect("a liquid");
    let look = map.pools[0].look;
    assert!((look.caustics - 0.25).abs() < 0.001);
    assert!((look.absorption - 0.5).abs() < 0.001);
    assert_eq!(look.color, usual.color, "what's unset stays usual");
}
