//! Repository automation.
//!
//! The `xtask` executable (`cargo xtask <command>`) is a thin command line
//! over this library, so the permanent tests in `xtask/tests` exercise the
//! same code the commands run:
//! - [`checks`] runs the required checks from DESIGN.md;
//! - [`build_number`] reserves package build numbers that are never reused;
//! - [`crate_graph`] states and enforces the crate dependency rules;
//! - [`luau`] keeps Luau formatted;
//! - [`product`] spells the product version and build identity.

pub mod build_number;
pub mod checks;
pub mod crate_graph;
pub mod luau;
pub mod product;

use std::path::PathBuf;

/// Returns the workspace root, the parent of the `xtask` package directory.
#[must_use]
pub fn workspace_root() -> PathBuf {
    PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/.."))
}
