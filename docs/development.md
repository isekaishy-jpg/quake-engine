# Development

How to build and check the engine. The rules behind these commands are in [DESIGN.md](../DESIGN.md) and the [style guide](style.md).

## Toolchain

`rust-toolchain.toml` pins Rust 1.98.1 with clippy and rustfmt, and rustup installs it on first use. The baseline platform is Windows x64.

## Commands

| Command | What it does |
| --- | --- |
| `cargo run` | Builds and runs the engine. It exits at once for now |
| `cargo xtask check` | The required checks, in order: `cargo fmt -- --check`, clippy with `-D warnings`, and all workspace tests |
| `cargo xtask build-number` | Shows the tracked build number and build identity |
| `cargo xtask reserve-build-number` | Reserves the next package build number |

Run `cargo fmt` without `--all`, which would reach `vendor/`.

## Adding a crate

A new crate needs a proposal and a rule in `xtask/src/crate_graph/rules.rs`. The checks refuse a crate with no rule.

## Game data

The engine reads your own copy of Quake. Game data is never committed.
