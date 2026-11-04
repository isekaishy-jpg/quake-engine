//! Every map in a real install. Run with `QUAKE_DIR` set and `--ignored`.

use std::path::{Path, PathBuf};

use engine_assets::FileSystem;
use fixture_quake::{Palette, import_map, mount_game, spawn_point};

/// Imports every map on the search path, and returns how many there were.
fn import_all(filesystem: &FileSystem, palette: &Palette) -> usize {
    let mut count = 0;
    for path in filesystem.list("maps/") {
        if !path.ends_with(".bsp") {
            continue;
        }
        let bsp = filesystem.read(&path).expect("read");
        let lit = filesystem.read(&path.replace(".bsp", ".lit")).ok();
        let map = import_map(&bsp, lit.as_deref(), palette).expect(&path);
        assert!(!map.faces.is_empty(), "{path}");
        assert!(!map.textures.is_empty(), "{path}");
        let _start = spawn_point(&map);
        count += 1;
    }
    count
}

#[test]
#[ignore = "needs game data"]
fn every_map_imports() {
    let base = PathBuf::from(std::env::var("QUAKE_DIR").expect("QUAKE_DIR"));
    let mut filesystem = FileSystem::new();
    mount_game(&mut filesystem, &base.join("id1")).expect("mount");
    let palette = filesystem.read("gfx/palette.lmp").expect("palette");
    let palette = Palette::parse(&palette).expect("palette");
    assert!(import_all(&filesystem, &palette) > 0);
    for extra in ["hipnotic", "rogue"] {
        let path = base.join(extra);
        if Path::is_dir(&path) {
            let mut filesystem = FileSystem::new();
            mount_game(&mut filesystem, &path).expect("mount");
            assert!(import_all(&filesystem, &palette) > 0, "{extra}");
        }
    }
    let rerelease = base.join("rerelease");
    for extra in ["id1", "hipnotic", "rogue", "dopa", "mg1", "mg3"] {
        let path = rerelease.join(extra);
        if Path::is_dir(&path) {
            let mut filesystem = FileSystem::new();
            mount_game(&mut filesystem, &path).expect("mount");
            let _count = import_all(&filesystem, &palette);
        }
    }
}
