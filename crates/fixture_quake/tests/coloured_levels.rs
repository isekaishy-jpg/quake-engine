//! Levels with coloured light, from the rerelease.

use engine_assets::{Archive, AssetError};
use fixture_quake::ColouredLevels;

/// An archive of empty files.
struct Files(Vec<&'static str>);

impl Archive for Files {
    fn label(&self) -> &str {
        "files"
    }

    fn paths(&self) -> Vec<String> {
        self.0.iter().map(|path| (*path).to_owned()).collect()
    }

    fn contains(&self, path: &str) -> bool {
        self.0.contains(&path)
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, AssetError> {
        if self.contains(path) {
            Ok(path.as_bytes().to_vec())
        } else {
            Err(AssetError::NotFound(path.to_owned()))
        }
    }
}

/// A rerelease with a level, a box that's a level file without light, and the rest.
fn rerelease() -> ColouredLevels<Files> {
    ColouredLevels::new(Files(vec![
        "maps/e1m1.bsp",
        "maps/e1m1.lit",
        "maps/b_bh10.bsp",
        "progs/player.mdl",
        "progs.dat",
        "gfx/palette.lmp",
    ]))
}

#[test]
fn a_level_comes_with_its_light() {
    let levels = rerelease();
    assert!(levels.contains("maps/e1m1.bsp"));
    assert!(levels.contains("maps/e1m1.lit"));
    assert_eq!(
        levels.read("maps/e1m1.lit").expect("reads"),
        b"maps/e1m1.lit"
    );
}

#[test]
fn everything_else_stays_as_the_game_has_it() {
    let levels = rerelease();
    for path in [
        "progs/player.mdl",
        "progs.dat",
        "gfx/palette.lmp",
        "maps/b_bh10.bsp",
    ] {
        assert!(!levels.contains(path), "{path}");
        assert!(levels.read(path).is_err(), "{path}");
    }
    assert_eq!(levels.paths().len(), 2);
}

#[test]
fn a_level_not_there_is_not_there() {
    assert!(!rerelease().contains("maps/e1m2.bsp"));
}
