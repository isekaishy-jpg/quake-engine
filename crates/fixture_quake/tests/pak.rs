//! Pak reading and game directories, using small generated paks.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use engine_assets::{Archive, AssetError, FileSystem};
use fixture_quake::{PakArchive, mount_game};

/// Numbers temporary directories within this test run.
static NEXT_DIRECTORY: AtomicU32 = AtomicU32::new(0);

/// A temporary directory, removed when dropped.
struct TempDirectory(PathBuf);

impl TempDirectory {
    /// Creates an empty directory.
    fn new() -> Self {
        let number = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let name = format!("fixture-quake-{}-{number}", std::process::id());
        let path = std::env::temp_dir().join(name);
        fs::create_dir_all(&path).expect("temporary directory");
        Self(path)
    }
}

impl Drop for TempDirectory {
    fn drop(&mut self) {
        let _ignored = fs::remove_dir_all(&self.0);
    }
}

/// The bytes of a pak holding `files`.
fn pak(files: &[(&str, &[u8])]) -> Vec<u8> {
    let mut bytes = vec![0; 12];
    let mut directory = Vec::new();
    for (name, contents) in files {
        let offset = u32::try_from(bytes.len()).expect("small test file");
        let length = u32::try_from(contents.len()).expect("small test file");
        bytes.extend_from_slice(contents);
        let mut field = [0; 56];
        field[..name.len()].copy_from_slice(name.as_bytes());
        directory.extend_from_slice(&field);
        directory.extend_from_slice(&offset.to_le_bytes());
        directory.extend_from_slice(&length.to_le_bytes());
    }
    let directory_offset = u32::try_from(bytes.len()).expect("small test file");
    let directory_length = u32::try_from(directory.len()).expect("small test file");
    bytes.extend_from_slice(&directory);
    bytes[..4].copy_from_slice(b"PACK");
    bytes[4..8].copy_from_slice(&directory_offset.to_le_bytes());
    bytes[8..12].copy_from_slice(&directory_length.to_le_bytes());
    bytes
}

/// Writes `bytes` to `name` in `directory`.
fn write(directory: &Path, name: &str, bytes: &[u8]) -> PathBuf {
    let path = directory.join(name);
    fs::write(&path, bytes).expect("write test file");
    path
}

#[test]
fn files_are_listed_and_read() {
    let directory = TempDirectory::new();
    let bytes = pak(&[
        ("maps/start.bsp", b"map"),
        ("Quake.RC", b"exec default.cfg"),
    ]);
    let path = write(&directory.0, "pak0.pak", &bytes);
    let archive = PakArchive::open(&path).expect("open");
    assert_eq!(archive.paths(), ["maps/start.bsp", "quake.rc"]);
    assert!(archive.contains("quake.rc"));
    assert_eq!(archive.read("maps/start.bsp").expect("read"), b"map");
    assert!(matches!(archive.read("nope"), Err(AssetError::NotFound(_))));
}

#[test]
fn bad_files_are_refused() {
    let directory = TempDirectory::new();
    let cases: [(&str, &[u8]); 2] = [("short", b"PACK"), ("wrong", b"NOPE\0\0\0\0\0\0\0\0")];
    for (name, bytes) in cases {
        let path = write(&directory.0, name, bytes);
        assert!(
            matches!(PakArchive::open(&path), Err(AssetError::Corrupt { .. })),
            "{name}"
        );
    }
}

#[test]
fn a_directory_past_the_end_is_refused() {
    let directory = TempDirectory::new();
    let mut bytes = pak(&[("a", b"x")]);
    bytes[8..12].copy_from_slice(&6400_u32.to_le_bytes());
    let path = write(&directory.0, "pak0.pak", &bytes);
    assert!(matches!(
        PakArchive::open(&path),
        Err(AssetError::Corrupt { .. })
    ));
}

#[test]
fn an_entry_past_the_end_is_refused() {
    let directory = TempDirectory::new();
    let mut bytes = pak(&[("a", b"x")]);
    let length_field = bytes.len() - 4;
    bytes[length_field..].copy_from_slice(&999_u32.to_le_bytes());
    let path = write(&directory.0, "pak0.pak", &bytes);
    assert!(matches!(
        PakArchive::open(&path),
        Err(AssetError::Corrupt { .. })
    ));
}

#[test]
fn a_game_directory_mounts_paks_over_loose_files_and_later_paks_win() {
    let directory = TempDirectory::new();
    fs::write(directory.0.join("both.txt"), "loose").expect("write");
    fs::write(directory.0.join("loose.txt"), "only loose").expect("write");
    let _pak0 = write(
        &directory.0,
        "PAK0.PAK",
        &pak(&[("both.txt", b"pak0"), ("a", b"a0")]),
    );
    let _pak1 = write(&directory.0, "pak1.pak", &pak(&[("a", b"a1")]));
    let mut filesystem = FileSystem::new();
    mount_game(&mut filesystem, &directory.0).expect("mount");
    assert_eq!(filesystem.read("both.txt").expect("read"), b"pak0");
    assert_eq!(filesystem.read("a").expect("read"), b"a1");
    assert_eq!(filesystem.read("loose.txt").expect("read"), b"only loose");
    assert_eq!(filesystem.search_path().len(), 3);
}

#[test]
fn a_missing_pak_ends_the_sequence() {
    let directory = TempDirectory::new();
    let _pak0 = write(&directory.0, "pak0.pak", &pak(&[("a", b"0")]));
    let _pak2 = write(&directory.0, "pak2.pak", &pak(&[("a", b"2")]));
    let mut filesystem = FileSystem::new();
    mount_game(&mut filesystem, &directory.0).expect("mount");
    assert_eq!(filesystem.read("a").expect("read"), b"0");
}

#[test]
fn a_missing_game_directory_is_reported() {
    let mut filesystem = FileSystem::new();
    let missing = Path::new("no-such-directory-here");
    assert!(matches!(
        mount_game(&mut filesystem, missing),
        Err(AssetError::NotFound(_))
    ));
}
