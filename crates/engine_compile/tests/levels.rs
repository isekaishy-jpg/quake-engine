//! The real levels play from start to end. Each test walks a level's walkable
//! grid between its beats, in the order a player goes. Doors and lifts aren't in
//! the grid, so a door is walked through, and a lift starts a new leg.
//!
//! These need the levels baked: `engine-compile content ember_keep vesper_deep`.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use engine_assets::{DirectoryArchive, FileSystem};
use engine_compile::{MaterialLibrary, apply_bake, attach_side_files, compile, map_hash, read_map};
use engine_core::math::Vec3;
use engine_format::from_text;
use engine_map::{CRISP_STYLES, Face, FaceLight, Map, SideFiles, TileLight};

/// How far from a beat a path can end, for a beat that's only near the grid.
const NEAR: f32 = 96.0;

/// The width of a player.
const PLAYER: f32 = 32.0;

/// Where content is.
fn content() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../content")
}

/// A level, with its bake.
fn level(name: &str) -> Map {
    let root = content();
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(DirectoryArchive::new(&root)));
    let path = root.join("maps").join(format!("{name}.emap"));
    let text = std::fs::read_to_string(path).expect("the map");
    let document = from_text(&text).expect("parses");
    let mut working = read_map(&document).expect("reads");
    let mut library = MaterialLibrary::new(&filesystem);
    let used = library.prepare(&mut working).expect("prepares");
    let mut source = |material: &str| library.load(material).expect("loads");
    let mut map = compile(&working, &mut source).expect("compiles");
    let bake = root.join("maps").join(format!("{name}.ebake"));
    let bytes = std::fs::read(bake).expect("baked");
    let hash = map_hash(&document, &used);
    assert!(apply_bake(&mut map, &bytes, hash).expect("reads"), "stale");
    let folder = root.clone();
    let files: SideFiles = Arc::new(move |file| std::fs::read(folder.join(file)).ok());
    attach_side_files(&mut map, name, &files);
    map
}

/// Walks `beats` in order, saying which step has no way.
fn walk(map: &Map, beats: &[(&str, [f32; 3])]) {
    for pair in beats.windows(2) {
        let ((from_name, from), (to_name, to)) = (pair[0], pair[1]);
        let (from, to) = (
            Vec3::new(from[0], from[1], from[2]),
            Vec3::new(to[0], to[1], to[2]),
        );
        let path = map.find_path(PLAYER, from, to);
        let path = path.unwrap_or_else(|| panic!("no way from {from_name} to {to_name}"));
        let last = *path.last().expect("a point");
        let missed = (last - to).length();
        assert!(missed < NEAR, "{to_name} is {missed} from the grid");
    }
}

/// Whether there's a way between two points.
fn joined(map: &Map, from: [f32; 3], to: [f32; 3]) -> bool {
    let (from, to) = (
        Vec3::new(from[0], from[1], from[2]),
        Vec3::new(to[0], to[1], to[2]),
    );
    map.find_path(PLAYER, from, to).is_some()
}

/// How far down an enemy may start from its floor.
const MOST_DROP: f32 = 64.0;

/// Those that need the larger clipping hull.
const LARGE: [&str; 4] = [
    "monster_ogre",
    "monster_demon1",
    "monster_shalrath",
    "monster_shambler",
];

/// What's wrong with where each enemy of `map` starts.
fn misplaced_enemies(map: &Map) -> Vec<String> {
    let mut found = Vec::new();
    let world = map.models.first().expect("a world");
    for entity in &map.entities {
        let kind = entity.get("classname").unwrap_or_default();
        if !kind.starts_with("monster_") {
            continue;
        }
        let numbers: Vec<f32> = entity
            .get("origin")
            .unwrap_or_default()
            .split_whitespace()
            .filter_map(|number| number.parse().ok())
            .collect();
        let [x, y, z] = numbers[..] else {
            found.push(format!("{kind} has no origin"));
            continue;
        };
        let hull = world.hulls[usize::from(LARGE.contains(&kind))];
        let at = Vec3::new(x, y, z);
        if map.tree.clip_solid(hull, at) {
            found.push(format!("{kind} at {x} {y} {z} is in solid"));
            continue;
        }
        // These fly.
        if kind == "monster_wizard" {
            continue;
        }
        let drop = (1..=512)
            .map(|step| step as f32)
            .find(|down| map.tree.clip_solid(hull, at - Vec3::new(0.0, 0.0, *down)));
        match drop {
            Some(drop) if drop <= MOST_DROP => {}
            Some(drop) => found.push(format!("{kind} at {x} {y} {z} is {drop} over its floor")),
            None => found.push(format!("{kind} at {x} {y} {z} has no floor")),
        }
    }
    found
}

/// Every enemy starts clear of the walls, and over a floor.
#[test]
#[ignore = "needs the levels baked"]
fn every_enemy_starts_somewhere_it_can_stand() {
    let mut found = Vec::new();
    for name in ["ember_keep", "vesper_deep"] {
        let misplaced = misplaced_enemies(&level(name));
        found.extend(misplaced.iter().map(|line| format!("{name}: {line}")));
    }
    assert!(
        found.is_empty(),
        "{}",
        found.join(
            "
"
        )
    );
}

/// How much the light can change across a join, out of 255, before it shows.
const MOST_STEP: f32 = 24.0;

/// How many of a level's places can still step that far. A few are left where
/// faces meet at a corner that's in the way of their samples.
const MOST_STEPS: usize = 40;

/// How far past an edge a point is looked for on the next face.
const ACROSS: f32 = 0.25;

/// How far apart points along a join are.
const ALONG: f32 = 4.0;

/// The way a flat `face` looks, if it has any area.
fn normal_of(corners: &[Vec3]) -> Option<Vec3> {
    let first = *corners.first()?;
    let mut sum = Vec3::ZERO;
    for pair in corners[1..].windows(2) {
        sum = sum + (pair[0] - first).cross(pair[1] - first);
    }
    let length = sum.length();
    (length > 1e-3).then(|| sum * (1.0 / length))
}

/// Whether `point`, on a face's plane, is on the face.
fn covers(corners: &[Vec3], normal: Vec3, point: Vec3) -> bool {
    let next = corners.iter().cycle().skip(1);
    corners
        .iter()
        .zip(next)
        .all(|(a, b)| normal.cross(*b - *a).dot(point - *a) >= -0.05)
}

/// One sample as a number, from what it's found by.
type Pick<'a> = &'a dyn Fn(usize) -> f32;

/// A layer of `face` blended between samples at `point`, as it's drawn.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "places on a face's own grid"
)]
fn blended(face: &Face, light: &FaceLight, layer: usize, point: Vec3, pick: Pick<'_>) -> f32 {
    let step = light.texels_per_sample as f32;
    let at =
        |axis: usize| (face.axes[axis].project(point) - light.texel_origin[axis] as f32) / step;
    let (x, y) = (at(0), at(1));
    let (low_x, low_y) = (x.floor(), y.floor());
    let (width, height) = (light.width as usize, light.height as usize);
    let first = light.first_sample as usize + layer * width * height;
    let read = |column: f32, row: f32| {
        let column = (column.max(0.0) as usize).min(width - 1);
        let row = (row.max(0.0) as usize).min(height - 1);
        pick(first + row * width + column)
    };
    let (right, down) = (x - low_x, y - low_y);
    let top = read(low_x, low_y) * (1.0 - right) + read(low_x + 1.0, low_y) * right;
    let bottom = read(low_x, low_y + 1.0) * (1.0 - right) + read(low_x + 1.0, low_y + 1.0) * right;
    top * (1.0 - down) + bottom * down
}

/// The red light `face` shows at `point`.
fn shown(map: &Map, tiles: &HashMap<u32, TileLight>, face: &Face, point: Vec3) -> f32 {
    let Some(light) = face.light else {
        return 0.0;
    };
    let held = tiles.get(&light.tile);
    let samples = held.map_or(&map.light, |tile| &tile.light);
    let edges = held.map_or(&map.light_edges, |tile| &tile.edges);
    let mut total = 0.0;
    for (layer, style) in light.styles.iter().enumerate() {
        if *style == Face::NO_STYLE {
            break;
        }
        let red = blended(face, &light, layer, point, &|at| f32::from(samples[at][0]));
        total += if CRISP_STYLES.contains(style) {
            let edge = blended(face, &light, layer, point, &|at| f32::from(edges[at]));
            if edge < 128.0 { 0.0 } else { red }
        } else {
            red
        };
    }
    total
}

/// A lit face, its corners, and the way it looks.
type LitFace<'a> = (&'a Face, Vec<Vec3>, Vec3);

/// How many places along the joins of `map` the light steps.
#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "rounding, and counts of steps along a face"
)]
fn steps_at_joins(map: &Map) -> usize {
    let mut tiles = HashMap::new();
    if let Some(known) = &map.light_tiles {
        for tile in (0_u32..).take(known.cells().len()) {
            if let Some(light) = known.read(tile) {
                tiles.insert(tile, light);
            }
        }
    }
    let mut planes: HashMap<[i64; 4], Vec<LitFace<'_>>> = HashMap::new();
    // A decal lies over a surface and doesn't join it.
    let is_decal = |face: &Face| {
        let texture = &map.textures[face.texture as usize];
        texture.material.as_ref().is_some_and(|m| m.decal)
    };
    for face in map.faces.iter().filter(|face| face.light.is_some()) {
        if is_decal(face) {
            continue;
        }
        let corners = map.face_points(face).to_vec();
        let Some(normal) = normal_of(&corners) else {
            continue;
        };
        let round = |value: f32| (value * 256.0).round() as i64;
        let height = (normal.dot(corners[0]) * 4.0).round() as i64;
        let key = [round(normal.x), round(normal.y), round(normal.z), height];
        planes.entry(key).or_default().push((face, corners, normal));
    }
    let mut steps = 0;
    for faces in planes.values().filter(|faces| faces.len() > 1) {
        for (number, (face, corners, normal)) in faces.iter().enumerate() {
            let middle = corners.iter().fold(Vec3::ZERO, |sum, corner| sum + *corner)
                * (1.0 / corners.len() as f32);
            let next = corners.iter().cycle().skip(1);
            for (from, to) in corners.iter().zip(next) {
                let side = *to - *from;
                let count = (side.length() / ALONG).ceil() as usize;
                let mut out = normal.cross(side);
                out = out * (1.0 / out.length().max(1e-4));
                if out.dot(middle - *from) > 0.0 {
                    out = out * -1.0;
                }
                for at in 1..count {
                    let point = *from + side * (at as f32 / count as f32);
                    let over = point + out * ACROSS;
                    for (other, (next_face, next_corners, next_normal)) in faces.iter().enumerate()
                    {
                        if other != number && covers(next_corners, *next_normal, over) {
                            let here = shown(map, &tiles, face, point);
                            let there = shown(map, &tiles, next_face, point);
                            steps += usize::from((here - there).abs() > MOST_STEP);
                        }
                    }
                }
            }
        }
    }
    steps
}

/// Faces side by side show the same light where they join, so shadows run on
/// across them. This also finds a bake that didn't come from the code in hand.
#[test]
#[ignore = "needs the levels baked"]
fn light_runs_on_across_joins() {
    for name in ["ember_keep", "vesper_deep"] {
        let steps = steps_at_joins(&level(name));
        assert!(steps <= MOST_STEPS, "{name} steps at {steps} places");
    }
}

/// A level's clipping trees stay in proportion to it. Vesper Deep had 75 million
/// nodes once, from copies of what brushes said where they overlapped.
#[test]
#[ignore = "needs the levels baked"]
fn clip_trees_stay_in_proportion() {
    for name in ["ember_keep", "vesper_deep"] {
        let map = level(name);
        let nodes = map.tree.clip_nodes.len();
        assert!(
            nodes < map.faces.len() * 40,
            "{name} has {nodes} clip nodes"
        );
    }
}

/// The Ember Keep plays through, in its three legs between lifts.
#[test]
#[ignore = "needs the level baked"]
fn the_ember_keep_plays_from_start_to_end() {
    let map = level("ember_keep");
    walk(
        &map,
        &[
            ("the start", [1536.0, -1400.0, 24.0]),
            ("the end of the ledge", [768.0, -220.0, 24.0]),
            ("the bottom of the stairs", [768.0, 320.0, -104.0]),
            ("the pillared hall", [600.0, 832.0, -104.0]),
            ("the Great Hall", [1300.0, 832.0, 24.0]),
            ("the Forge door", [2000.0, 720.0, 24.0]),
            ("the Forge", [2704.0, 720.0, 24.0]),
            ("the foot of the lift", [2704.0, 1040.0, 24.0]),
        ],
    );
    walk(
        &map,
        &[
            ("the top of the lift", [2600.0, 1200.0, 280.0]),
            ("the gantry", [2280.0, 700.0, 280.0]),
            ("the silver key", [3016.0, 264.0, 280.0]),
        ],
    );
    walk(
        &map,
        &[
            ("the gantry", [2280.0, 350.0, 280.0]),
            ("the passage", [2000.0, 350.0, 280.0]),
            ("the tower", [2080.0, 100.0, 280.0]),
        ],
    );
    walk(
        &map,
        &[
            ("the Great Hall", [1536.0, 832.0, 24.0]),
            ("the silver door", [1150.0, 1180.0, 24.0]),
            ("the crypt", [480.0, 1200.0, -168.0]),
            ("the altar", [480.0, 2030.0, -168.0]),
            ("the crypt's lift", [760.0, 2170.0, -168.0]),
        ],
    );
    walk(
        &map,
        &[
            ("the top of the crypt's lift", [1060.0, 2170.0, 280.0]),
            ("the balcony", [1250.0, 1330.0, 296.0]),
        ],
    );
    walk(
        &map,
        &[
            ("the Great Hall", [1536.0, 1300.0, 24.0]),
            ("the gold door", [1536.0, 1480.0, 24.0]),
            ("the Sanctum", [1536.0, 1650.0, 24.0]),
            ("round the lava", [1176.0, 2000.0, 24.0]),
            ("the far side", [1536.0, 2500.0, 24.0]),
            ("the exit", [1536.0, 2750.0, 24.0]),
        ],
    );
}

/// Each locked part really is locked: with its door aside, nothing joins it to
/// what comes before it but the doors a player needs a key for.
#[test]
#[ignore = "needs the level baked"]
fn the_ember_keep_has_no_way_round_its_doors() {
    let map = level("ember_keep");
    // Without the lift, the gantry can't be reached from the floor of the Forge.
    assert!(!joined(&map, [2704.0, 720.0, 24.0], [3016.0, 264.0, 280.0]));
    // The Crypt's upper passage only meets the hall at its one-way door, and the
    // Sanctum isn't reached from it at all.
    assert!(!joined(
        &map,
        [1024.0, 1600.0, 280.0],
        [1536.0, 2500.0, 24.0]
    ));
}

/// Vesper Deep plays through, from the broken gate to the Pumpworks, with the
/// walkways as a branch. Its grids come from beside its bake, a chunk at a time.
#[test]
#[ignore = "needs the level baked"]
fn vesper_deep_plays_from_start_to_end() {
    let map = level("vesper_deep");
    walk(
        &map,
        &[
            ("the gate", [4096.0, 160.0, 1560.0]),
            ("the street", [4096.0, 900.0, 1560.0]),
            ("the square", [3800.0, 1760.0, 1564.0]),
            ("the Minster's door", [4096.0, 2300.0, 1560.0]),
            ("the nave", [4096.0, 2900.0, 1560.0]),
            ("behind the altar", [4096.0, 3200.0, 1560.0]),
            ("under the altar", [4096.0, 3560.0, 1400.0]),
        ],
    );
    walk(
        &map,
        &[
            ("the foot of the shaft", [3930.0, 3700.0, 344.0]),
            ("the arch", [4096.0, 3560.0, 344.0]),
            ("the terrace", [4096.0, 3440.0, 344.0]),
            ("the Grand Stair", [4096.0, 3100.0, 240.0]),
            ("the Market", [3900.0, 2300.0, 92.0]),
            ("the quay", [4400.0, 1450.0, 88.0]),
            ("the bridge", [4160.0, 1200.0, 120.0]),
            ("the south bank", [4160.0, 900.0, 88.0]),
        ],
    );
    walk(
        &map,
        &[
            ("the Market", [4900.0, 2300.0, 92.0]),
            ("the east bridge", [5800.0, 1700.0, 120.0]),
            ("the Foundry door", [6200.0, 1680.0, 92.0]),
            ("the wheel", [7380.0, 1700.0, 92.0]),
        ],
    );
    walk(
        &map,
        &[
            ("the Market", [3300.0, 2300.0, 92.0]),
            ("the west bridge", [2200.0, 2190.0, 120.0]),
            ("the Pumpworks door", [1900.0, 2190.0, 88.0]),
            ("the Pumpworks", [1320.0, 2190.0, 92.0]),
            ("the way on", [480.0, 1880.0, 88.0]),
        ],
    );
    walk(
        &map,
        &[
            ("the terrace's east end", [5300.0, 3880.0, 344.0]),
            ("the walkway", [5900.0, 3880.0, 344.0]),
            ("the first tower", [6300.0, 3900.0, 344.0]),
            ("the next tower", [6300.0, 3100.0, 344.0]),
            ("the corner tower", [7000.0, 3100.0, 344.0]),
        ],
    );
    walk(
        &map,
        &[
            ("the high tower", [7000.0, 3900.0, 536.0]),
            ("the high walk", [6600.0, 3900.0, 536.0]),
            ("the far tower", [6300.0, 4600.0, 536.0]),
        ],
    );
}
