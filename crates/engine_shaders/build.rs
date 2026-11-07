//! Gives the engine's shader code a number, so that what's compiled can't go stale unseen.

use std::fs;
use std::path::{Path, PathBuf};

/// Every file under `folder`, in order.
fn files_in(folder: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            files_in(&path, found);
        } else {
            found.push(path);
        }
    }
}

fn main() {
    println!("cargo:rerun-if-changed=src/shaders");
    println!("cargo:rerun-if-changed=src/compiler.rs");
    let mut files = vec![PathBuf::from("src/compiler.rs")];
    files_in(Path::new("src/shaders"), &mut files);
    files.sort();
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for file in files {
        for byte in fs::read(file).unwrap_or_default() {
            if byte != b'\r' {
                hash = (hash ^ u64::from(byte)).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
    }
    println!("cargo:rustc-env=ENGINE_SHADERS_ID={hash:016x}");
}
