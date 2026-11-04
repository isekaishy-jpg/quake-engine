# Proposal 0008: crate boundaries and their permanent test

Status: accepted and implemented, 2026-11-03, increment 0.

## Research

- [Proposal 0001](0001-architecture.md) sets the encapsulation rules.
- Cargo refuses dependency cycles among normal dependencies but allows a development dependency to point upwards, which still couples crates.
- `cargo metadata --no-deps` reports each package's declared dependencies. Cargo doesn't report lints, so those are read from the manifest.
- populous-reborn enforces rules like these with a table and a check on a small workspace model, so tests can state violating workspaces directly.

## Design

The rules are data in `xtask/src/crate_graph/rules.rs`:

| Crate | May depend on |
|---|---|
| `engine_core` | Nothing |
| `engine_console` | `core` |
| `engine_platform` | `core`, `solapp` |
| `engine_assets` | `core` |
| `engine_map` | `core` |
| `engine_physics` | `core`, `map` |
| `engine_movement` | `core`, `physics` |
| `engine_sim` | `core`, `map`, `physics`, `movement` |
| `engine_script` | `core`, `map`, `sim` |
| `engine_net` | `core` |
| `engine_server` | `core`, `console`, `map`, `physics`, `sim`, `script`, `net` |
| `engine_client` | `core`, `console`, `map`, `physics`, `movement`, `net` |
| `engine_render` | `core`, `map` |
| `engine_audio` | `core` |
| `engine_ui` | `core`, `console`, `render` |
| `fixture_quake` | `core`, `assets`, `map` |
| `engine_app` | Every engine crate and the fixture |
| `xtask` | Nothing |

Private backends, each usable only by its owner: `solapp` (`engine_platform`), `vulkane` (`engine_render`), `box3d-rust` (`engine_physics`), `mlua` (`engine_script`), `kira` (`engine_audio`), `assets_manager` and `planus` (`engine_assets`), `renet`, `renet_netcode` and `renet_steam` (`engine_net`), `qbsp` (`fixture_quake`).

The check reports every violation: a package with no rule, a forbidden dependency of any kind, a backend outside its owner, and a package not inheriting the shared fields or lints.

## Proposal

- **Selected approach:** the table and check above, enforced by `xtask/tests/crate_graph.rs`.
- **Tradeoffs:** a dependency on a backend from the wrong crate fails the checks, even in a test.
- **Acceptance criteria:** the permanent test passes on the workspace, and tests show each kind of violation being reported.
- **Future extension:** changing a rule is an architecture decision with its own proposal. The check can't see a backend's type leaking through a public API, so review has to catch that for now.

## Review

- **Finding:** rules alone can't stop a foreign type appearing in a public signature. Listed above as a limit, and added to the style guide's review checklist.
- The tests that covered populous-reborn's UI libraries, TOML rule and testing features were dropped, because those rules don't exist here.
