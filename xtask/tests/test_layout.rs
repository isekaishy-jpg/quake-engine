//! docs/style.md §12: tests are not collocated with production code. No file under a
//! `src/` tree of the workspace holds a test, and only the crates that came from Sol
//! hold `#[cfg(test)]` hooks (proposal 0125).

use std::path::{Path, PathBuf};
use xtask::crate_graph::TEST_SEAMS;

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

/// Every source file of the workspace.
fn all_sources() -> Vec<(String, PathBuf)> {
    let root = workspace_root();
    let mut sources = Vec::new();
    let mut found = Vec::new();
    rust_files(&root.join("xtask").join("src"), &mut found);
    sources.extend(found.drain(..).map(|path| ("xtask".to_owned(), path)));
    let crates = std::fs::read_dir(root.join("crates")).expect("crates");
    for member in crates.flatten() {
        let name = member.file_name().to_string_lossy().into_owned();
        rust_files(&member.path().join("src"), &mut found);
        sources.extend(found.drain(..).map(|path| (name.clone(), path)));
    }
    sources
}

/// The files of `sources` holding `marker`.
fn holding(sources: &[(String, PathBuf)], marker: &str) -> Vec<String> {
    sources
        .iter()
        .filter(|(_, path)| std::fs::read_to_string(path).is_ok_and(|text| text.contains(marker)))
        .map(|(_, path)| path.display().to_string())
        .collect()
}

/// The permanent test: no `src/` file of any member holds a test.
#[test]
fn no_test_lives_under_src() {
    let sources = all_sources();
    assert!(sources.len() > 15, "found {} source files", sources.len());
    let offenders = holding(&sources, concat!("#[te", "st]"));
    assert!(offenders.is_empty(), "tests under src: {offenders:?}");
}

/// No `src/` file holds test code at all, except that [`TEST_SEAMS`] reach tests by
/// `#[path]` and keep hooks for them.
#[test]
fn no_test_code_lives_under_src() {
    let mut sources = all_sources();
    sources.retain(|(name, _)| !TEST_SEAMS.contains(&name.as_str()));
    assert!(sources.len() > 15, "found {} source files", sources.len());
    let offenders = holding(&sources, concat!("#[cfg", "(test)]"));
    assert!(offenders.is_empty(), "test code under src: {offenders:?}");
}
