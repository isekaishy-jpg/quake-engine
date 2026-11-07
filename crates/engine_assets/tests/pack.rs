//! Packed containers.

use std::fs;
use std::path::{Path, PathBuf};

use engine_assets::{
    Archive, AssetError, DirectoryArchive, FileIds, FileSystem, PackArchive, write_pack,
    write_pack_of,
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
    let path = root.with_extension("pkt");
    let mut bytes = Vec::new();
    write_pack(
        &DirectoryArchive::new(root),
        &mut FileIds::default(),
        &mut bytes,
    )
    .expect("packs");
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
    write_pack_of(&archive, one, &mut FileIds::default(), &mut alone).expect("packs");
    let two = vec!["one.bin".to_owned(), "two.bin".to_owned()];
    let mut both = Vec::new();
    let summary = write_pack_of(&archive, two, &mut FileIds::default(), &mut both).expect("packs");
    assert_eq!(summary.shared, 1);
    assert!(both.len() < alone.len() + 64, "only an index entry more");
    let path = root.with_extension("pkt");
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
    let path = root.with_extension("pkt");
    let mut bytes = Vec::new();
    let summary = write_pack(
        &DirectoryArchive::new(&root),
        &mut FileIds::default(),
        &mut bytes,
    )
    .expect("packs");
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
    let bytes = fs::read(&path).expect("read");
    let size = fs::read(root.join("readme.txt")).expect("read").len() as u64;
    let number = size.to_le_bytes();
    // The file's size and how long it's stored are the same number, in the index.
    let places: Vec<usize> = (0..bytes.len() - 8)
        .filter(|at| bytes[*at..*at + 8] == number)
        .collect();
    assert_eq!(places.len(), 2, "{places:?}");
    let refused = places.iter().any(|at| {
        let mut changed = bytes.clone();
        changed[*at..*at + 8].copy_from_slice(&1_u64.to_le_bytes());
        fs::write(&path, changed).expect("write");
        matches!(PackArchive::open(&path), Err(AssetError::Corrupt { .. }))
    });
    assert!(refused);
    fs::remove_dir_all(&root).expect("clean up");
    fs::remove_file(&path).expect("clean up");
}

#[test]
fn a_file_keeps_its_id_and_a_new_one_never_takes_an_old_one() {
    let mut ids = FileIds::default();
    let first = ids.assign("maps/a.ebin");
    let second = ids.assign("maps/b.ebin");
    assert_ne!(first, second);
    assert_eq!(ids.assign("maps/a.ebin"), first);
    let mut read = FileIds::parse(&ids.to_text()).expect("reads back");
    assert_eq!(read, ids);
    assert!(read.assign("maps/c.ebin") > second);
}

#[test]
fn a_table_with_one_id_twice_is_refused() {
    assert!(FileIds::parse("next;3\n1;a\n1;b\n").is_err());
    assert!(FileIds::parse("next;1\n1;a\n").is_err());
}

#[test]
fn a_file_can_be_read_by_its_id() {
    let root = folder("by_id");
    fs::write(root.join("thing.txt"), b"held").expect("write");
    let mut ids = FileIds::default();
    let mut bytes = Vec::new();
    write_pack(&DirectoryArchive::new(&root), &mut ids, &mut bytes).expect("packs");
    let path = root.with_extension("pkt");
    fs::write(&path, bytes).expect("write");
    let mut filesystem = FileSystem::new();
    filesystem.mount(Box::new(PackArchive::open(&path).expect("opens")));
    let id = ids.id_of("thing.txt").expect("has an id");
    assert_eq!(filesystem.read_id(id).expect("reads"), b"held");
    assert!(filesystem.read_id(id + 1).is_err());
}

#[test]
fn verifying_finds_a_changed_byte_that_opening_does_not() {
    let root = folder("verified");
    fs::write(root.join("thing.txt"), b"some bytes to change").expect("write");
    let path = packed(&root);
    PackArchive::open(&path)
        .expect("opens")
        .verify()
        .expect("is whole");
    let mut bytes = fs::read(&path).expect("read");
    let last = bytes.len() - 1;
    bytes[last] ^= 1;
    fs::write(&path, bytes).expect("write");
    let opened = PackArchive::open(&path).expect("still opens");
    assert!(opened.verify().is_err());
}

/// Whatever a damaged index says, opening one gives an answer and doesn't fall over.
#[test]
fn no_changed_byte_in_an_index_breaks_opening() {
    let root = folder("fuzzed");
    let path = packed(&root);
    let bytes = fs::read(&path).expect("read");
    let index = 16
        + usize::try_from(u64::from_le_bytes(
            bytes[8..16].try_into().expect("a length"),
        ))
        .expect("fits");
    for at in 0..index {
        for flip in [0x01, 0x80, 0xff] {
            let mut changed = bytes.clone();
            changed[at] ^= flip;
            fs::write(&path, &changed).expect("write");
            if let Ok(pack) = PackArchive::open(&path) {
                for file in pack.paths() {
                    let _either = pack.read(&file);
                }
            }
        }
    }
    fs::remove_dir_all(&root).expect("clean up");
    fs::remove_file(&path).expect("clean up");
}
