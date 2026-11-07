# Development

How to build and check the engine. The rules behind these commands are in [DESIGN.md](../DESIGN.md) and the [style guide](style.md).

## Toolchain

`rust-toolchain.toml` pins Rust 1.98.1 with clippy and rustfmt, and rustup installs it on first use. The baseline platform is Windows x64.

## Commands

| Command | What it does |
| --- | --- |
| `cargo run` | Builds and runs the game |
| `cargo xtask check` | The required checks, in order: `cargo fmt -- --check`, clippy with `-D warnings`, and all workspace tests |
| `cargo xtask build-number` | Shows the tracked build number and build identity |
| `cargo xtask reserve-build-number` | Reserves the next package build number |

Run `cargo fmt` without `--all`, which would reach `vendor/`.

## Timing the GPU

`gpu_times` on the console starts a clock on the GPU, and after that says what each part of a frame took, on average, since it was last asked. `gpu_times off` stops it. A clock on the CPU can't tell a frame's parts apart. A GPU's own clocks change speed, so one scene can read half as much again from one run to the next: only compare several runs, taken in turn.

## Adding a crate

A new crate needs a proposal and a rule in `xtask/src/crate_graph/rules.rs`. The checks refuse a crate with no rule.

## Game data

The engine reads your own copy of Quake. Game data is never committed.

## Bakes

`engine-compile content <map>` bakes a level's light the usual way. `--bake` says how much work to do, before the map's name:

| Bake | What it does |
|---|---|
| `quick` | Coarse, with nothing bounced, to see a level soon |
| `usual` | What a build does |
| `final` | Fine, for a level that's done. It goes in `maps/<map>.efinal`, and a build leaves that alone |
| `none` | No light, and a level plays flat. It takes away a build's bake |

The game uses a final bake if it's of this version of the level, and a build's if it isn't, and the compiler says when a final bake is out of date. What only a final bake shows has to be checked before a level's done, so one can be run on part of a level, with two corners:

```bash
target/release/engine-compile content --bake final --area -300 -300 0 300 300 300 yard
```

That goes by whole faces, so a floor that runs through the box counts all of it. The `compile` tool takes `bake`, `low` and `high`.
