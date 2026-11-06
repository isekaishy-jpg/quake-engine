//! Compiling brushes into something the engine collides with and draws.

use std::sync::Arc;

use engine_compile::{
    LoadedMesh, compile, compile_text, occluders_of, placeholder_texture, read_map_text, read_mesh,
    read_occlusion, resolve_meshes, write_occlusion,
};
use engine_core::math::Vec3;
use engine_format::from_text;
use engine_map::{Contents, Face, Map, Material, MipLevel, NodeChild, PoolLook, Texture};
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
    assert_eq!(door.get("name"), Some("door_1"));
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

/// A square of two triangles, with its UVs across a whole image.
const SQUARE: &str = r#"
format 1 mesh
part top {
    material = stone
    points = 0 0 0  64 0 0  64 64 0  0 64 0
    uvs = 0 0  1 0  1 1  0 1
    triangles = 0 1 2  0 2 3
}
hull { cuboid = 0 0 -8 64 64 0 }
"#;

/// The last of the world's faces, which a mesh in the world ends with.
fn last_world_face(map: &Map) -> &Face {
    let world = &map.models[0];
    &map.faces[(world.first_face + world.face_count - 1) as usize]
}

/// The room, with `SQUARE` placed by `mesh`.
fn room_with_mesh(mesh: &str) -> Map {
    let mut working = read_map_text(&format!("{ROOM}\n{mesh}\n")).expect("reads");
    let square = read_mesh(&from_text(SQUARE).expect("text")).expect("a mesh");
    let mut read = |_: &str| {
        Ok(LoadedMesh {
            mesh: Arc::new(square.clone()),
            occluders: occluders_of(&square),
        })
    };
    resolve_meshes(&mut working, &mut read).expect("placed");
    let mut source = |name: &str| placeholder_texture(name);
    compile(&working, &mut source).expect("compiles")
}

/// Each triangle is a face, in place, and its UVs reach its corners.
#[test]
fn a_placed_mesh_is_faces_with_its_uvs() {
    let plain = room();
    let map = room_with_mesh("mesh { source = square origin = -128 -128 -200 }");
    assert_eq!(map.faces.len(), plain.faces.len() + 2);
    let face = last_world_face(&map);
    let first = face.first_point as usize;
    let corners = &map.points[first..first + 3];
    assert_eq!(corners[0], Vec3::new(-128.0, -128.0, -200.0));
    let width = 64.0;
    let across: Vec<f32> = corners.iter().map(|p| face.axes[0].project(*p)).collect();
    assert!((across[0] - 0.0).abs() < 0.01, "{across:?}");
    assert!((across[1] - width).abs() < 0.01, "{across:?}");
}

/// A placement turns and stretches.
#[test]
fn a_placement_turns_and_scales() {
    let map =
        room_with_mesh("mesh { source = square origin = 0 0 -200 angles = 0 90 0 scale = 2 }");
    let face = last_world_face(&map);
    let first = face.first_point as usize;
    let far = map.points[first + 1];
    assert!((far.x + 128.0).abs() < 0.01, "{far:?}");
    assert!((far.y - 128.0).abs() < 0.01, "{far:?}");
}

/// A hull blocks movement, and cuts nothing out of what's around it.
#[test]
fn a_hull_collides_without_cutting_faces() {
    let plain = room();
    let map = Arc::new(room_with_mesh(
        "mesh { source = square origin = -128 -128 -256 }",
    ));
    assert_eq!(map.faces.len(), plain.faces.len() + 2, "the floor is whole");
    let world = CollisionWorld::new(Arc::clone(&map));
    assert!(world.in_solid(Vec3::new(-100.0, -100.0, -258.0)));
    let middle = Vec3::new(-96.0, -96.0, -259.0);
    assert_eq!(contents_at(&map, middle), Contents::Solid);
}

/// A mesh that's only seen is drawn, and there's nothing to bump into.
#[test]
fn a_mesh_only_to_be_seen_has_no_hull() {
    let plain = room();
    let map = Arc::new(room_with_mesh(
        "mesh { source = square origin = -128 -128 -200 only_seen = 1 }",
    ));
    assert_eq!(map.faces.len(), plain.faces.len() + 2, "it's drawn");
    let world = CollisionWorld::new(Arc::clone(&map));
    assert!(!world.in_solid(Vec3::new(-100.0, -100.0, -204.0)));
}

/// An entity's mesh is its model, and its sidecar goes with it.
#[test]
fn a_mesh_entity_has_a_model_and_a_shade() {
    let map =
        room_with_mesh("entity lift_1 { type = func_door mesh = square origin = -128 -128 -200 }");
    let found = map
        .entities
        .iter()
        .find(|e| e.fields.iter().any(|(k, v)| k == "name" && v == "lift_1"))
        .expect("the entity");
    let model = found
        .fields
        .iter()
        .find(|(k, _)| k == "model")
        .expect("model");
    let number: u32 = model.1.trim_start_matches('*').parse().expect("a number");
    assert!(
        !found.fields.iter().any(|(k, _)| k == "origin"),
        "its place is its model's"
    );
    let occluder = map.occluders.first().expect("a shade");
    assert_eq!(occluder.model, number);
    let middle = occluder.spheres[0].center;
    assert!((middle[0] + 96.0).abs() < 0.01, "{middle:?}");
}

/// A sidecar reads back as what was written.
#[test]
fn a_sidecar_round_trips() {
    let square = read_mesh(&from_text(SQUARE).expect("text")).expect("a mesh");
    let spheres = occluders_of(&square);
    assert_eq!(spheres.len(), 1, "one part has only the whole");
    let text = write_occlusion(&spheres);
    assert_eq!(read_occlusion(&from_text(&text).expect("text")), spheres);
}

/// A triangle that names a corner the part lacks says so.
#[test]
fn a_bad_mesh_says_what_is_wrong() {
    let text = "format 1 mesh\npart p { material = m points = 0 0 0 uvs = 0 0 triangles = 0 1 2 }";
    let error = read_mesh(&from_text(text).expect("text")).expect_err("no such corner");
    assert!(error.to_string().contains("corner"), "{error}");
}

/// A flame is crossed faces seen from both sides, with no collision, and
/// something that flickers and sparks.
#[test]
fn a_flame_has_faces_and_an_emitter() {
    let plain = room();
    let text = format!("{ROOM}\nentity f {{ type = flame origin = 0 100 -200 size = 32 }}\n");
    let mut source = |name: &str| placeholder_texture(name);
    let map = Arc::new(compile_text(&text, &mut source).expect("compiles"));
    assert_eq!(map.faces.len(), plain.faces.len() + 8);
    let emitter = map.emitters[0];
    assert_eq!(emitter.origin, Vec3::new(0.0, 100.0, -184.0));
    assert!((emitter.light_radius - 192.0).abs() < 0.01);
    let world = CollisionWorld::new(Arc::clone(&map));
    assert!(!world.in_solid(emitter.origin), "nothing to walk into");
}

/// A decal is pieces of the faces under it, just off them, and it stops at
/// its edges.
#[test]
fn a_decal_clips_to_the_faces_under_it() {
    let plain = room();
    let text = format!(
        "{ROOM}\ndecal {{ material = mark origin = -128 -128 -256 normal = 0 0 1 size = 64 }}\n"
    );
    let mut source = |name: &str| placeholder_texture(name);
    let map = compile_text(&text, &mut source).expect("compiles");
    assert!(map.faces.len() > plain.faces.len(), "the decal has faces");
    let world = &map.models[0];
    let face = &map.faces[(world.first_face + world.face_count - 1) as usize];
    let first = face.first_point as usize;
    for point in &map.points[first..first + face.point_count as usize] {
        assert!(
            (point.z + 255.8).abs() < 0.01,
            "just off the floor: {point:?}"
        );
        assert!(point.x >= -160.01 && point.x <= -95.99, "{point:?}");
    }
}

/// A decal turned away from a face leaves it alone.
#[test]
fn a_decal_only_lands_on_what_it_faces() {
    let plain = room();
    let text = format!(
        "{ROOM}\ndecal {{ material = mark origin = -128 -128 -256 normal = 0 0 -1 size = 64 }}\n"
    );
    let mut source = |name: &str| placeholder_texture(name);
    let map = compile_text(&text, &mut source).expect("compiles");
    assert_eq!(map.faces.len(), plain.faces.len());
}

/// A face fits itself to the hotspot nearest its shape.
#[test]
fn a_face_fits_the_hotspot_of_its_shape() {
    let text = "format 1 map\nbrush { material = sheet cuboid = 0 0 0 256 64 16 }\n";
    let mut source = |name: &str| {
        let mut texture = placeholder_texture(name);
        texture.material = Some(Arc::new(Material {
            hotspots: vec![[0.0, 0.0, 1.0, 0.25], [0.0, 0.25, 0.5, 1.0]],
            ..Material::default()
        }));
        texture
    };
    let map = compile_text(text, &mut source).expect("compiles");
    let top = map
        .faces
        .iter()
        .find(|face| {
            let first = face.first_point as usize;
            map.points[first..first + face.point_count as usize]
                .iter()
                .all(|point| (point.z - 16.0).abs() < 0.01)
        })
        .expect("the top");
    let width = map.textures[top.texture as usize].mips[0].width as f32;
    let height = map.textures[top.texture as usize].mips[0].height as f32;
    let first = top.first_point as usize;
    let corners = &map.points[first..first + top.point_count as usize];
    let along: Vec<f32> = corners
        .iter()
        .map(|p| top.axes[0].project(*p) / width)
        .collect();
    let down: Vec<f32> = corners
        .iter()
        .map(|p| top.axes[1].project(*p) / height)
        .collect();
    let range = |values: &[f32]| {
        let low = values.iter().copied().fold(f32::MAX, f32::min);
        let high = values.iter().copied().fold(f32::MIN, f32::max);
        (low, high)
    };
    let (low, high) = range(&along);
    assert!(low.abs() < 0.01 && (high - 1.0).abs() < 0.01, "{along:?}");
    let (low, high) = range(&down);
    assert!(low.abs() < 0.01 && (high - 0.25).abs() < 0.01, "{down:?}");
}

/// Pieces of a face that another brush has cut still fit as one, by the whole
/// of it.
#[test]
fn a_cut_face_fits_a_hotspot_as_a_whole() {
    let text = "format 1 map
        brush { material = sheet cuboid = 0 0 0 256 64 16 }
        brush { material = plain cuboid = 96 16 16 160 48 48 }
";
    let mut source = |name: &str| {
        let mut texture = placeholder_texture(name);
        if name == "sheet" {
            texture.material = Some(Arc::new(Material {
                hotspots: vec![[0.0, 0.0, 1.0, 0.25], [0.0, 0.25, 0.5, 1.0]],
                ..Material::default()
            }));
        }
        texture
    };
    let map = compile_text(text, &mut source).expect("compiles");
    let tops: Vec<&Face> = map
        .faces
        .iter()
        .filter(|face| {
            let first = face.first_point as usize;
            map.textures[face.texture as usize].name == "sheet"
                && map.points[first..first + face.point_count as usize]
                    .iter()
                    .all(|point| (point.z - 16.0).abs() < 0.01)
        })
        .collect();
    assert!(tops.len() > 1, "the top is cut into pieces");
    for piece in &tops {
        assert_eq!(piece.axes, tops[0].axes, "one fit for all of them");
    }
}

/// A stencil image for stamps: black in the middle, mid grey round it.
fn stencil(name: &str, inner: [u8; 4]) -> Texture {
    let mut texture = placeholder_texture(name);
    let mut rgba = Vec::new();
    for y in 0..16 {
        for x in 0..16 {
            let middle = (4..12).contains(&x) && (4..12).contains(&y);
            rgba.extend_from_slice(&if middle { inner } else { [128, 128, 128, 255] });
        }
    }
    texture.mips = vec![MipLevel {
        width: 16,
        height: 16,
        rgba,
        form: engine_map::PixelForm::Rgba,
    }];
    texture
}

/// A floor with `stamp` pressed into it, in the room.
fn stamped_room(inner: [u8; 4]) -> (Map, usize) {
    let text = format!(
        "{ROOM}\nstamp {{ image = cut inner = carved origin = -128 -128 -256 normal = 0 0 1 \
         size = 64 depth = 16 detail = 8 }}\n"
    );
    let mut source = |name: &str| {
        if name == "cut" {
            stencil(name, inner)
        } else {
            placeholder_texture(name)
        }
    };
    let map = compile_text(&text, &mut source).expect("compiles");
    let carved = map.textures.iter().position(|t| t.name == "carved");
    (map, carved.unwrap_or(usize::MAX))
}

/// The heights of faces of a texture, over or under the floor.
fn floor_levels(map: &Map) -> Vec<f32> {
    let mut levels: Vec<f32> = Vec::new();
    for face in &map.faces {
        let first = face.first_point as usize;
        let points = &map.points[first..first + face.point_count as usize];
        let z = points[0].z;
        let flat = points.iter().all(|p| (p.z - z).abs() < 0.01);
        let inside = points
            .iter()
            .all(|p| (-160.01..=-95.99).contains(&p.x) && (-160.01..=-95.99).contains(&p.y));
        if flat && inside && !levels.iter().any(|l| (*l - z).abs() < 0.01) {
            levels.push(z);
        }
    }
    levels
}

/// Black cuts into a surface, and leaves walls of the inner material.
#[test]
fn a_dark_stamp_carves_with_walls() {
    let (map, carved) = stamped_room([0, 0, 0, 255]);
    let levels = floor_levels(&map);
    assert!(
        levels.iter().any(|z| (*z + 272.0).abs() < 0.01),
        "{levels:?}"
    );
    assert!(
        levels.iter().any(|z| (*z + 256.0).abs() < 0.01),
        "{levels:?}"
    );
    let walls = map
        .faces
        .iter()
        .filter(|f| f.texture as usize == carved)
        .count();
    assert!(walls >= 4, "a wall on each side: {walls}");
}

/// White stands out from a surface.
#[test]
fn a_bright_stamp_raises() {
    let (map, _) = stamped_room([255, 255, 255, 255]);
    let levels = floor_levels(&map);
    assert!(
        levels.iter().any(|z| (*z + 240.0).abs() < 0.01),
        "{levels:?}"
    );
}

/// Stamped faces say where to take their light from, past the surface.
#[test]
fn a_stamp_lifts_its_light() {
    let (map, _) = stamped_room([0, 0, 0, 255]);
    assert!(!map.light_lifts.is_empty());
    let deepest = map
        .light_lifts
        .iter()
        .map(|(_, lift)| lift.z)
        .fold(0.0, f32::max);
    assert!(deepest >= 16.0, "past the surface: {deepest}");
}

/// The floor stays whole round a stamp, and collision doesn't change.
#[test]
fn a_stamp_leaves_collision_and_the_rest_alone() {
    let (map, _) = stamped_room([0, 0, 0, 255]);
    let map = Arc::new(map);
    let world = CollisionWorld::new(Arc::clone(&map));
    assert!(
        world.in_solid(Vec3::new(-128.0, -128.0, -258.0)),
        "still solid where cut"
    );
    let covers = |x: f32, y: f32| {
        map.faces.iter().any(|face| {
            let first = face.first_point as usize;
            let points = &map.points[first..first + face.point_count as usize];
            points.iter().all(|p| (p.z + 256.0).abs() < 0.01) && {
                let low_x = points.iter().map(|p| p.x).fold(f32::MAX, f32::min);
                let high_x = points.iter().map(|p| p.x).fold(f32::MIN, f32::max);
                let low_y = points.iter().map(|p| p.y).fold(f32::MAX, f32::min);
                let high_y = points.iter().map(|p| p.y).fold(f32::MIN, f32::max);
                (low_x..=high_x).contains(&x) && (low_y..=high_y).contains(&y)
            }
        })
    };
    assert!(covers(-200.0, -200.0), "the floor beyond it");
    assert!(!covers(-128.0, -128.0), "no flat floor over the cut");
}

/// A map small enough for one cell still finds its leaves from node 0, which is
/// where lookups start.
#[test]
fn a_tiny_map_has_a_tree_that_works() {
    let text = "format 1 map
brush { material = floor cuboid = -64 -64 -16 64 64 0 }
brush { material = roof cuboid = -64 -64 96 64 64 112 }
brush { material = wall cuboid = -80 -64 -16 -64 64 112 }
brush { material = wall cuboid = 64 -64 -16 80 64 112 }
brush { material = wall cuboid = -64 -80 -16 64 -64 112 }
brush { material = wall cuboid = -64 64 -16 64 80 112 }
";
    let mut source = |name: &str| placeholder_texture(name);
    let map = compile_text(text, &mut source).expect("compiles");
    let inside = map.tree.leaf_at(Vec3::new(0.0, 0.0, 48.0));
    let leaf = map.tree.leaf(inside).expect("a leaf");
    assert_eq!(leaf.contents, Contents::Empty);
    assert!(leaf.face_count > 0, "it lists faces to draw");
    assert_eq!(
        map.tree.leaf_at(Vec3::new(0.0, 0.0, -8.0)),
        0,
        "the floor is solid"
    );
}

/// A brush only cuts the faces it touches. Its planes run on for ever, and they
/// used to slice every face they crossed, right across a map.
#[test]
fn a_brush_leaves_faces_it_does_not_touch_whole() {
    let faces = |extra: &str| {
        let text = format!(
            "format 1 map\n\
             brush {{ material = wall cuboid = -1024 -1024 -32 1024 1024 0 }}\n{extra}"
        );
        let working = read_map_text(&text).expect("reads");
        let mut source = |name: &str| placeholder_texture(name);
        compile(&working, &mut source)
            .expect("compiles")
            .faces
            .len()
    };
    let floor = faces("");
    // A box standing on the floor cuts one hole in it.
    let standing = faces("brush { material = wall cuboid = 0 0 0 64 64 64 }\n");
    // Up in the air, with planes crossing the floor's sides, it cuts nothing.
    let floating = faces("brush { material = wall cuboid = 0 0 200 64 64 264 }\n");
    assert_eq!(floating, floor + 6, "only its own faces");
    assert!(standing <= floor + 5 + 8, "{standing}");
}

#[test]
fn a_level_says_how_its_water_looks() {
    let text = "format 1 map
water { reflect = 0.2 waves = 0 refraction = 0.5 }
brush { material = stone cuboid = -128 -128 -64 128 128 -32 }
brush { material = stone contents = water cuboid = -64 -64 -32 64 64 0 }
";
    let working = read_map_text(text).expect("reads");
    let mut source = |name: &str| placeholder_texture(name);
    let map = compile(&working, &mut source).expect("compiles");
    let look = map.pools[0].look;
    assert_eq!(look.reflect, 0.2);
    assert_eq!(look.waves, 0.0);
    assert_eq!(look.refraction, 0.5);
}

#[test]
fn a_zone_is_a_shape_and_nothing_to_see_or_touch() {
    let text = "format 1 map
brush { material = stone cuboid = -128 -128 -64 128 128 -32 }
entity market {
    type = zone
    title = \"The Market\"
    parent = town
    music = 4
    light = 0.5
    brush { material = hidden cuboid = 0 0 0 64 64 64 }
}
";
    let working = read_map_text(text).expect("reads");
    let mut source = |name: &str| placeholder_texture(name);
    let map = compile(&working, &mut source).expect("compiles");
    assert_eq!(map.models.len(), 1, "it's no model");
    assert_eq!(map.zones.len(), 1);
    let zone = &map.zones[0];
    assert_eq!(zone.name, "market");
    assert_eq!(zone.title.as_deref(), Some("The Market"));
    assert_eq!(zone.parent.as_deref(), Some("town"));
    assert_eq!(zone.music.as_deref(), Some("4"));
    assert_eq!(zone.light, Some([0.5; 3]));
    assert!(zone.holds(Vec3::new(32.0, 32.0, 32.0)));
    assert!(!zone.holds(Vec3::new(100.0, 32.0, 32.0)));
    assert_eq!(zone.size, 64.0 * 64.0 * 64.0);
}
