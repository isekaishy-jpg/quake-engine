//! The crate dependency rules of proposal 0008, as data.
//!
//! Changing a rule is an architecture decision: it needs a proposal, and the
//! permanent test over `cargo metadata` enforces whatever this table says.

/// One workspace crate and the workspace crates it may depend on.
#[derive(Clone, Copy, Debug)]
pub struct CrateRule {
    /// The package name.
    pub name: &'static str,
    /// The workspace packages it may depend on, in any dependency kind.
    pub may_depend_on: &'static [&'static str],
}

/// Every workspace package and its permitted workspace dependencies.
///
/// - `engine_core` is the shared foundation and depends on nothing.
/// - Dependencies point towards foundations and never upwards.
/// - No engine crate knows the fixture: only `fixture_quake` understands Quake
///   formats, and only `engine_app` knows it exists.
/// - The server side never sees rendering, audio, UI or the platform.
/// - `engine_app` is the composition root, the only crate that knows every
///   system, and nothing depends on it.
/// - `xtask` is repository tooling, and nothing depends on it.
pub const CRATE_RULES: &[CrateRule] = &[
    CrateRule {
        name: "engine_core",
        may_depend_on: &[],
    },
    CrateRule {
        name: "engine_format",
        may_depend_on: &[],
    },
    CrateRule {
        name: "engine_console",
        may_depend_on: &["engine_core"],
    },
    CrateRule {
        name: "engine_platform",
        may_depend_on: &["engine_core", "engine_host"],
    },
    CrateRule {
        name: "engine_assets",
        may_depend_on: &["engine_core"],
    },
    CrateRule {
        name: "engine_map",
        may_depend_on: &["engine_core"],
    },
    CrateRule {
        name: "engine_compile",
        may_depend_on: &[
            "engine_assets",
            "engine_core",
            "engine_format",
            "engine_map",
            "engine_physics",
        ],
    },
    CrateRule {
        name: "engine_tools",
        may_depend_on: &[
            "engine_assets",
            "engine_compile",
            "engine_format",
            "engine_map",
            "engine_script",
            "engine_shaders",
        ],
    },
    CrateRule {
        name: "engine_shaders",
        may_depend_on: &[],
    },
    CrateRule {
        name: "engine_editor",
        may_depend_on: &[
            "engine_assets",
            "engine_compile",
            "engine_core",
            "engine_format",
            "engine_map",
            "engine_platform",
            "engine_render",
            "engine_tools",
        ],
    },
    CrateRule {
        name: "engine_test_maps",
        may_depend_on: &["engine_core", "engine_map"],
    },
    CrateRule {
        name: "engine_physics",
        may_depend_on: &["engine_core", "engine_map", "engine_test_maps"],
    },
    CrateRule {
        name: "engine_movement",
        may_depend_on: &["engine_core", "engine_physics"],
    },
    CrateRule {
        name: "engine_sim",
        may_depend_on: &[
            "engine_core",
            "engine_map",
            "engine_physics",
            "engine_movement",
            "engine_test_maps",
        ],
    },
    CrateRule {
        name: "engine_script",
        may_depend_on: &["engine_core", "engine_map", "engine_sim"],
    },
    CrateRule {
        name: "engine_net",
        may_depend_on: &["engine_core"],
    },
    CrateRule {
        name: "engine_server",
        may_depend_on: &[
            "engine_core",
            "engine_console",
            "engine_map",
            "engine_physics",
            "engine_sim",
            "engine_script",
            "engine_net",
            "engine_test_maps",
        ],
    },
    CrateRule {
        name: "engine_client",
        may_depend_on: &[
            "engine_core",
            "engine_console",
            "engine_map",
            "engine_physics",
            "engine_movement",
            "engine_net",
            "engine_test_maps",
        ],
    },
    CrateRule {
        name: "engine_render",
        may_depend_on: &["engine_core", "engine_map", "engine_shaders"],
    },
    CrateRule {
        name: "engine_audio",
        may_depend_on: &["engine_core"],
    },
    CrateRule {
        name: "engine_ui",
        may_depend_on: &["engine_core", "engine_console", "engine_render"],
    },
    CrateRule {
        name: "fixture_quake",
        may_depend_on: &["engine_core", "engine_assets", "engine_map"],
    },
    CrateRule {
        name: "engine_app",
        may_depend_on: &[
            "engine_assets",
            "engine_audio",
            "engine_client",
            "engine_compile",
            "engine_console",
            "engine_core",
            "engine_map",
            "engine_movement",
            "engine_net",
            "engine_physics",
            "engine_platform",
            "engine_render",
            "engine_script",
            "engine_shaders",
            "engine_server",
            "engine_sim",
            "engine_ui",
            "fixture_quake",
        ],
    },
    CrateRule {
        name: "engine_host",
        may_depend_on: &["engine_worker", "engine_cache"],
    },
    CrateRule {
        name: "engine_worker",
        may_depend_on: &[],
    },
    CrateRule {
        name: "engine_cache",
        may_depend_on: &[],
    },
    CrateRule {
        name: "engine_cache_worker_example",
        may_depend_on: &["engine_cache", "engine_worker"],
    },
    CrateRule {
        name: "xtask",
        may_depend_on: &[],
    },
];

/// Packages whose tests still sit beside their code. They reach private seams,
/// so moving them needs those seams settled first (proposal 0096).
pub const INLINE_TESTS: &[&str] = &["engine_host", "engine_worker", "engine_cache"];

/// An external dependency that is a swappable backend and therefore private
/// to one owning crate.
#[derive(Clone, Copy, Debug)]
pub struct BackendRule {
    /// The external package name.
    pub dependency: &'static str,
    /// The only workspace crate that may depend on it.
    pub owner: &'static str,
}

/// The backends. Swapping one stays local to its owner.
pub const BACKEND_RULES: &[BackendRule] = &[
    BackendRule {
        dependency: "engine_host",
        owner: "engine_platform",
    },
    BackendRule {
        dependency: "vulkane",
        owner: "engine_render",
    },
    BackendRule {
        dependency: "naga",
        owner: "engine_shaders",
    },
    BackendRule {
        dependency: "gltf",
        owner: "engine_compile",
    },
    BackendRule {
        dependency: "fontdue",
        owner: "engine_ui",
    },
    BackendRule {
        dependency: "box3d-rust",
        owner: "engine_physics",
    },
    BackendRule {
        dependency: "mlua",
        owner: "engine_script",
    },
    BackendRule {
        dependency: "kira",
        owner: "engine_audio",
    },
    BackendRule {
        dependency: "assets_manager",
        owner: "engine_assets",
    },
    BackendRule {
        dependency: "planus",
        owner: "engine_assets",
    },
    BackendRule {
        dependency: "renet",
        owner: "engine_net",
    },
    BackendRule {
        dependency: "renet_netcode",
        owner: "engine_net",
    },
    BackendRule {
        dependency: "renet_steam",
        owner: "engine_net",
    },
    BackendRule {
        dependency: "renetcode",
        owner: "engine_net",
    },
    BackendRule {
        dependency: "spake2",
        owner: "engine_net",
    },
    BackendRule {
        dependency: "chacha20poly1305",
        owner: "engine_net",
    },
    BackendRule {
        dependency: "ed25519-dalek",
        owner: "engine_net",
    },
    BackendRule {
        dependency: "x25519-dalek",
        owner: "engine_net",
    },
    BackendRule {
        dependency: "sha2",
        owner: "engine_net",
    },
    BackendRule {
        dependency: "qbsp",
        owner: "fixture_quake",
    },
];

/// A file-format library and the crates that may use it.
#[derive(Clone, Copy, Debug)]
pub struct FormatRule {
    /// The external package name.
    pub dependency: &'static str,
    /// The workspace crates that may depend on it.
    pub users: &'static [&'static str],
}

/// The format libraries.
pub const FORMAT_RULES: &[FormatRule] = &[FormatRule {
    dependency: "serde_json",
    users: &["engine_app", "engine_editor", "engine_tools"],
}];

/// The edition every workspace package uses.
pub const REQUIRED_EDITION: &str = "2024";
/// The minimum Rust version every workspace package declares.
pub const REQUIRED_RUST_VERSION: &str = "1.98";
