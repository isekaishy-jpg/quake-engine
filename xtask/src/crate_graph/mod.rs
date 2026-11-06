//! The crate dependency rules and their check.
//!
//! The check works on a small model of the workspace ([`WorkspacePackage`])
//! rather than on `cargo metadata` types, so tests can state violating
//! workspaces directly. [`load_workspace`] builds the model from Cargo.

mod check;
mod load;
mod manifest;
mod rules;

pub use check::{Violation, check_workspace};
pub use load::{LoadError, load_workspace};
pub use manifest::inherits_workspace_lints;
pub use rules::{
    BACKEND_RULES, BackendRule, CRATE_RULES, CrateRule, FORMAT_RULES, FormatRule, INLINE_TESTS,
    REQUIRED_EDITION, REQUIRED_RUST_VERSION,
};

/// The kind of a dependency declaration.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// `[dependencies]`.
    Normal,
    /// `[dev-dependencies]`.
    Development,
    /// `[build-dependencies]`.
    Build,
}

/// One dependency declared by a workspace package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Dependency {
    /// The depended-on package name (not a local rename).
    pub name: String,
    /// The section that declares it.
    pub kind: Kind,
    /// The features it enables.
    pub features: Vec<String>,
}

/// The facts the rules need about one workspace package.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspacePackage {
    /// The package name.
    pub name: String,
    /// The Rust edition.
    pub edition: String,
    /// The declared minimum Rust version, if any.
    pub rust_version: Option<String>,
    /// Whether the package may be published to a registry.
    pub publishable: bool,
    /// Whether the manifest inherits the workspace lints. Cargo does not
    /// report lints, so this comes from the manifest text.
    pub inherits_workspace_lints: bool,
    /// Every declared dependency, workspace and external.
    pub dependencies: Vec<Dependency>,
    /// What the package's own features enable, such as
    /// `rui_runtime/testing`.
    pub feature_enables: Vec<String>,
}
