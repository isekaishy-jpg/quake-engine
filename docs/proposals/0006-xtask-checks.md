# Proposal 0006: `xtask` and the required checks

Status: accepted and implemented, 2026-11-03, increment 0.

## Research

- DESIGN.md requires checks to pass with zero warnings. Automation in a Rust `xtask` crate is [the recommended pattern](https://matklad.github.io/2021/08/22/large-rust-workspaces.html), and it runs wherever cargo runs.
- populous-reborn's `xtask` already implements this with tests. Its checks, build numbers and crate-graph code are reusable, and it's the user's own code. Its UI pack and pack-reader generation aren't relevant here.
- On Windows a running executable can't be replaced, so `xtask` builds into its own target directory, or `cargo xtask check` would fail rebuilding itself.

## Design

`xtask` is a library with a thin binary. `cargo xtask check` runs, in order and stopping at the first failure:

```text
cargo fmt -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
```

Nested commands use the `CARGO` variable, so they stay on the pinned toolchain.

## Proposal

- **Selected approach:** adapt populous-reborn's `xtask`, without its pack tooling.
- **Tradeoffs:** a second copy of this code exists across repositories. They serve different products, so sharing isn't worth a dependency.
- **Acceptance criteria:** `cargo xtask check` passes on the workspace and fails on a violation.
- **Future extension:** packaging and asset generation become commands here.

## Review

`cargo xtask check` passes. Tests were carried over and adapted, and they cover build numbers, product versions, crate rules and test layout.
