//! Loose files.

use std::fs;

use engine_assets::{Archive, AssetError, DirectoryArchive};

/// Upper-case files on disk answer to lower-case archive paths.
#[test]
fn files_are_found_whatever_their_case_on_disk() {
    let root = std::env::temp_dir().join(format!("engine-assets-{}", std::process::id()));
    fs::create_dir_all(root.join("MAPS")).expect("directory");
    fs::write(root.join("MAPS").join("START.BSP"), "map").expect("write");
    let archive = DirectoryArchive::new(&root);
    assert!(archive.contains("maps/start.bsp"));
    assert_eq!(archive.read("maps/start.bsp").expect("read"), b"map");
    assert_eq!(archive.paths(), ["maps/start.bsp"]);
    assert!(!archive.contains("maps"), "a directory isn't a file");
    assert!(matches!(archive.read("nope"), Err(AssetError::NotFound(_))));
    fs::remove_dir_all(&root).expect("clean up");
}
