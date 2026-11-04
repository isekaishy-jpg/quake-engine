//! The search path and path rules.

use engine_assets::{Archive, AssetError, FileSystem, normalize_path};

/// An archive of fixed files.
struct Memory {
    /// Its name.
    label: &'static str,
    /// Its files.
    files: Vec<(&'static str, &'static str)>,
}

impl Archive for Memory {
    fn label(&self) -> &str {
        self.label
    }

    fn paths(&self) -> Vec<String> {
        self.files
            .iter()
            .map(|(path, _)| (*path).to_owned())
            .collect()
    }

    fn contains(&self, path: &str) -> bool {
        self.files.iter().any(|(name, _)| *name == path)
    }

    fn read(&self, path: &str) -> Result<Vec<u8>, AssetError> {
        self.files
            .iter()
            .find(|(name, _)| *name == path)
            .map(|(_, text)| text.as_bytes().to_vec())
            .ok_or_else(|| AssetError::NotFound(path.to_owned()))
    }
}

/// A search path with a base archive and an override mounted on top.
fn filesystem() -> FileSystem {
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(Memory {
        label: "base",
        files: vec![("maps/a.bsp", "base a"), ("maps/b.bsp", "base b")],
    }));
    filesystem.mount(Box::new(Memory {
        label: "mod",
        files: vec![("maps/a.bsp", "mod a"), ("sound/x.wav", "x")],
    }));
    filesystem
}

#[test]
fn the_most_recently_mounted_archive_wins() {
    let filesystem = filesystem();
    assert_eq!(filesystem.read("maps/a.bsp").expect("read"), b"mod a");
    assert_eq!(filesystem.read("maps/b.bsp").expect("read"), b"base b");
    assert_eq!(filesystem.location("maps/a.bsp"), Some("mod"));
    assert_eq!(filesystem.search_path(), ["mod", "base"]);
}

#[test]
fn paths_are_normalized_before_lookup() {
    assert_eq!(filesystem().read("MAPS\\B.BSP").expect("read"), b"base b");
}

#[test]
fn a_missing_file_is_reported() {
    assert!(matches!(
        filesystem().read("nope"),
        Err(AssetError::NotFound(_))
    ));
    assert!(!filesystem().exists("nope"));
    assert!(filesystem().exists("sound/x.wav"));
}

#[test]
fn listing_is_sorted_and_unique() {
    assert_eq!(filesystem().list("maps/"), ["maps/a.bsp", "maps/b.bsp"]);
}

#[test]
fn paths_cannot_leave_the_game_data() {
    for bad in [
        "",
        "../secret",
        "a/../../b",
        "/etc/passwd",
        "c:/windows",
        "\\\\share",
    ] {
        assert!(
            matches!(normalize_path(bad), Err(AssetError::InvalidPath(_))),
            "{bad}"
        );
    }
    assert_eq!(
        normalize_path("Gfx\\Palette.LMP").expect("valid"),
        "gfx/palette.lmp"
    );
}
