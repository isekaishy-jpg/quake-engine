//! docs/style.md §12: tests are not collocated with production code. No
//! file under a `src/` tree of the workspace holds a `#[cfg(test)]` item
//! (proposal 0009 exempts the crates that came from Sol).

use std::path::{Path, PathBuf};

use xtask::crate_graph::LINT_EXEMPT;
use xtask::workspace_root;

/// Every `.rs` file under `dir`, recursively.
fn rust_files(dir: &Path, found: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            rust_files(&path, found);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
}

/// The permanent test: no `src/` file of a member holds test code.
#[test]
fn no_test_code_lives_under_src() {
    let root = workspace_root();
    let mut sources = Vec::new();
    rust_files(&root.join("xtask").join("src"), &mut sources);
    let crates = std::fs::read_dir(root.join("crates")).expect("crates");
    for member in crates.flatten() {
        let name = member.file_name();
        if !LINT_EXEMPT.contains(&name.to_string_lossy().as_ref()) {
            rust_files(&member.path().join("src"), &mut sources);
        }
    }
    assert!(sources.len() > 15, "found {} source files", sources.len());
    let marker = concat!("#[cfg", "(test)]");
    let offenders: Vec<String> = sources
        .iter()
        .filter(|path| std::fs::read_to_string(path).is_ok_and(|text| text.contains(marker)))
        .map(|path| path.display().to_string())
        .collect();
    assert!(
        offenders.is_empty(),
        "test code under src/:\n{}",
        offenders.join("\n")
    );
}
