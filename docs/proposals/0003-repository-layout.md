# Proposal 0003: repository and workspace layout

Status: accepted and implemented, 2026-11-03, increment 0.

## Research

- DESIGN.md allows only README.md and DESIGN.md as root documentation. [Large Rust Workspaces](https://matklad.github.io/2021/08/22/large-rust-workspaces.html) recommends a flat `crates/` directory with a virtual root manifest.
- populous-reborn uses `crates/<name>/`, `xtask/`, and `docs/` split by record type, with `[workspace.package]` inheritance and dependencies declared once.
- Cargo treats a path dependency inside the workspace as a member, and `exclude` doesn't override that for a path under a member directory. That left patched `winit` and `micropool` as members, which failed zero-warnings checks. Moving them outside member directories fixes it.

| Option for patched third-party code | For | Against |
|---|---|---|
| Root `vendor/` | Clearly not ours, excluded cleanly | Sol manifests need `../../vendor` paths |
| Inside each Sol crate | Unchanged from upstream | Becomes a workspace member and fails the checks |

## Design

```
Cargo.toml  Cargo.lock  rust-toolchain.toml  clippy.toml  .gitattributes  BUILD_NUMBER
.cargo/config.toml   the cargo xtask alias
crates/              engine crates, the fixture and the Sol crates
vendor/              patched third-party sources: winit, micropool
xtask/               repository automation
reference/           id Software's releases, as submodules
docs/                index, style, development, research, proposals, reviews, pre-research
```

Resolver 3. `default-members` is `engine_app`, so `cargo run` starts the engine. Every package inherits version, edition, `rust-version` and `publish`. Crate names use `engine_<system>`, because the product name isn't settled and the engine isn't tied to one game.

## Proposal

- **Selected approach:** the layout above.
- **Tradeoffs:** empty crates exist before they have code, so boundaries hold from the first commit.
- **Acceptance criteria:** `cargo metadata` lists exactly the ruled packages, `cargo run` builds and runs the executable, and no other documentation sits at the root.
- **Future extension:** a new crate needs a proposal and a rule.

## Review

`cargo run` builds and exits at once, which is correct for an empty composition root. `cargo fmt` is used without `--all`, because `--all` would reach `vendor/`.
