//! Packed containers.

use std::fs;
use std::path::{Path, PathBuf};

use engine_assets::{
    Archive, AssetError, DirectoryArchive, FileSystem, PackArchive, write_pack, write_pack_of,
};

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
fn packed(root: &Path) -> PathBuf {
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

#[test]
fn files_that_are_the_same_are_held_once() {
    let root = folder("shared");
    let bytes: Vec<u8> = (0..4096_u32).flat_map(u32::to_le_bytes).collect();
    fs::write(root.join("one.bin"), &bytes).expect("write");
    fs::write(root.join("two.bin"), &bytes).expect("write");
    let mut alone = Vec::new();
    let archive = DirectoryArchive::new(&root);
    let one = vec!["one.bin".to_owned()];
    write_pack_of(&archive, one, &mut alone).expect("packs");
    let two = vec!["one.bin".to_owned(), "two.bin".to_owned()];
    let mut both = Vec::new();
    let summary = write_pack_of(&archive, two, &mut both).expect("packs");
    assert_eq!(summary.shared, 1);
    assert!(both.len() < alone.len() + 64, "only an index entry more");
    let path = root.with_extension("epak");
    fs::write(&path, both).expect("write");
    let pack = PackArchive::open(&path).expect("opens");
    assert_eq!(pack.read("one.bin").expect("read"), bytes);
    assert_eq!(pack.read("two.bin").expect("read"), bytes);
    assert!(!pack.contains("readme.txt"), "only what was asked for");
    fs::remove_dir_all(&root).expect("clean up");
    fs::remove_file(&path).expect("clean up");
}

#[test]
fn what_squeezes_is_squeezed_and_what_doesnt_isnt() {
    let root = folder("squeeze");
    let plain = vec![7_u8; 100_000];
    let mut state = 1_u32;
    let noise: Vec<u8> = (0..100_000)
        .map(|_| {
            state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
            state.to_le_bytes()[3]
        })
        .collect();
    fs::write(root.join("plain.bin"), &plain).expect("write");
    fs::write(root.join("noise.bin"), &noise).expect("write");
    let path = root.with_extension("epak");
    let mut bytes = Vec::new();
    let summary = write_pack(&DirectoryArchive::new(&root), &mut bytes).expect("packs");
    assert_eq!(summary.bytes, 200_016);
    assert!(summary.stored < 101_000, "{} stored", summary.stored);
    fs::write(&path, bytes).expect("write");
    let pack = PackArchive::open(&path).expect("opens");
    assert_eq!(pack.read("plain.bin").expect("read"), plain);
    assert_eq!(pack.read("noise.bin").expect("read"), noise);
    fs::remove_dir_all(&root).expect("clean up");
    fs::remove_file(&path).expect("clean up");
}

#[test]
fn a_squeezed_file_that_wont_open_is_refused() {
    let root = folder("spoilt");
    fs::remove_file(root.join("materials").join("stone.emat")).expect("remove");
    fs::write(root.join("readme.txt"), vec![9_u8; 10_000]).expect("write");
    let path = packed(&root);
    let mut bytes = fs::read(&path).expect("read");
    let end = bytes.len();
    bytes[end - 8..].fill(0xff);
    let at = bytes.len() / 2;
    bytes.truncate(at.max(end - 2));
    bytes.extend_from_slice(&[0xff; 2]);
    fs::write(&path, bytes).expect("write");
    let pack = PackArchive::open(&path).expect("the index is fine");
    assert!(matches!(
        pack.read("readme.txt"),
        Err(AssetError::Corrupt { .. })
    ));
    fs::remove_dir_all(&root).expect("clean up");
    fs::remove_file(&path).expect("clean up");
}

#[test]
fn a_file_said_to_grow_in_a_pack_is_refused() {
    let root = folder("grows");
    fs::remove_file(root.join("materials").join("stone.emat")).expect("remove");
    let path = packed(&root);
    let mut bytes = fs::read(&path).expect("read");
    // The file's own size is the last number in the only index entry.
    let at = 12 + 2 + "readme.txt".len() + 16;
    bytes[at] = 1;
    fs::write(&path, bytes).expect("write");
    assert!(matches!(
        PackArchive::open(&path),
        Err(AssetError::Corrupt { .. })
    ));
    fs::remove_dir_all(&root).expect("clean up");
    fs::remove_file(&path).expect("clean up");
}
