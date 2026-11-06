//! The real levels play from start to end. Each test walks a level's walkable
//! grid between its beats, in the order a player goes. Doors and lifts aren't in
//! the grid, so a door is walked through, and a lift starts a new leg.
//!
//! These need the levels baked: `engine-compile content ember_keep vesper_deep`.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use engine_assets::{DirectoryArchive, FileSystem};
use engine_compile::{MaterialLibrary, apply_bake, attach_side_files, compile, map_hash, read_map};
use engine_core::math::Vec3;
use engine_format::from_text;
use engine_map::{Map, SideFiles};

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
