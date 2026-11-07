# Proposal 0133: precompiled shaders

Status: accepted and implemented, 2026-11-06, the [work list](../research/work-list.md), items 29 and 30.

## Research

[Proposal 0107](0107-shaders.md) measured compiling at each launch at about 16 ms and left it. The human's word: shaders should be precompiled. Compiling at runtime might do while testing, and has no place in a finished product. So a measurement doesn't settle this. The note also said the engine's own shaders live in the container, and 0107 kept the core in the program so that a broken one could still show green.

- The compiler was reached through `vulkane::safe::naga`, at nine places in the renderer. Tools can't use the renderer, so what compiles has to be in `engine_shaders`.
- `naga` is a large thing for a finished game to carry, with every WGSL source as text.
- A renderer makes pipelines as it starts, before any level, so the store has to be there first.
- What's compiled can go stale when the engine's shaders change, so it needs to say what it was made from.
- File ids shouldn't move when that happens, or `fids.txt` grows without end.

## Design

- **`Module`** names what's compiled: canvas, models, sprites, shadows, edge smoothing, the plain surface, a surface with its code, and the final pass with its code, with or without samples. The variants live here, so that what's asked for and what's compiled can't differ.
- **`compile_shaders`** writes every module a game could want to `compiled/shaders`: what's built in, and all of `shaders/*.wgsl`. Packing runs it. A shader that doesn't compile stops the pack, so a broken one can't ship, and neither can a broken core shader.
- **`spirv`** reads what's compiled. With the `compiler` feature, what's missing or stale is compiled when it's asked for. Without it, a game says it needs its `common.pkt`.
- **A compiled file starts with the number of the shader code it came from**, from `build.rs`, and one that doesn't match isn't used. Paths don't carry it, so file ids hold still.
- **The feature.** `engine_shaders/compiler` is off unless asked for. `engine_app` has `shader_compiler` by default, for working on things. `cargo build --release -p engine_app --no-default-features` is a finished game: no `naga`, and no WGSL in it.
- **Green** is one more compiled module in the container, so it needs nothing kept in the program. That answers why 0107 kept the core there.
- `engine_compile`'s packing is a feature too, on by default, and the workspace takes it off for what depends on the crate, so a game doesn't pull a compiler in through it.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a finished game can't start with no `common.pkt`, and says so. Editing a `.wgsl` file under a finished game does nothing, as it should.
- **Acceptance criteria:** a game with no compiler plays a level with shaders from a container. A broken shader stops a pack. Stale compiled shaders aren't used. Working from a folder still compiles what's needed.
- **Future extension:** none planned.

## Review

A build with `--no-default-features` has no `naga` in its tree and no shader text in the program. In an empty folder with `common.pkt` it played the showcase, which uses shaders, with nothing about them in its console. Without the container it stops and says why. The first version put the code's number in each path, which would have grown `fids.txt` at every shader change, so it moved inside the file.
