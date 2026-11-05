//! Packed containers.

use std::fs;
use std::path::PathBuf;

use engine_assets::{Archive, AssetError, DirectoryArchive, FileSystem, PackArchive, write_pack};

/// A fresh folder with two files in it.
fn folder(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!("engine-pack-{name}-{}", std::process::id()));
    let _gone = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("materials")).expect("directory");
    fs::write(root.join("materials").join("stone.emat"), "stone").expect("write");
    fs::write(root.join("readme.txt"), "hello there").expect("write");
    root
}

/// `root` written as a pack beside it.
fn packed(root: &PathBuf) -> PathBuf {
    let path = root.with_extension("epak");
    let mut bytes = Vec::new();
    write_pack(&DirectoryArchive::new(root), &mut bytes).expect("packs");
    fs::write(&path, bytes).expect("write");
    path
}

#[test]
fn a_pack_holds_what_its_folder_held() {
    let root = folder("same");
    let path = packed(&root);
    let pack = PackArchive::open(&path).expect("opens");
    assert_eq!(pack.paths(), ["materials/stone.emat", "readme.txt"]);
    assert!(pack.contains("readme.txt"));
    assert_eq!(pack.read("materials/stone.emat").expect("read"), b"stone");
    assert_eq!(pack.read("readme.txt").expect("read"), b"hello there");
    assert!(matches!(pack.read("nope"), Err(AssetError::NotFound(_))));
    fs::remove_dir_all(&root).expect("clean up");
    fs::remove_file(&path).expect("clean up");
}

#[test]
fn a_later_container_overrides_an_earlier_one_by_name() {
    let root = folder("layer");
    let path = packed(&root);
    let over = folder("over");
    fs::write(over.join("readme.txt"), "changed").expect("write");
    fs::remove_file(over.join("materials").join("stone.emat")).expect("remove");
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(PackArchive::open(&path).expect("opens")));
    filesystem.mount(Box::new(DirectoryArchive::new(&over)));
    assert_eq!(filesystem.read("readme.txt").expect("read"), b"changed");
    assert_eq!(
        filesystem.read("materials/stone.emat").expect("read"),
        b"stone"
    );
    fs::remove_dir_all(&root).expect("clean up");
    fs::remove_dir_all(&over).expect("clean up");
    fs::remove_file(&path).expect("clean up");
}

#[test]
fn a_damaged_pack_is_refused() {
    let root = folder("bad");
    let path = packed(&root);
    let bytes = fs::read(&path).expect("read");
    for length in [0, 3, 11, 13, 30] {
        fs::write(&path, &bytes[..length]).expect("write");
        assert!(PackArchive::open(&path).is_err(), "cut to {length}");
    }
    fs::write(&path, b"not a pack at all").expect("write");
    assert!(PackArchive::open(&path).is_err(), "wrong magic");
    let mut lying = bytes.clone();
    lying.truncate(bytes.len() - 4);
    fs::write(&path, &lying).expect("write");
    assert!(PackArchive::open(&path).is_err(), "a file past the end");
    fs::remove_dir_all(&root).expect("clean up");
    fs::remove_file(&path).expect("clean up");
}
