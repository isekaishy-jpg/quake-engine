# Proposal 0016: taking in `vulkane`

Status: accepted, 2026-11-03, increment 2.

## Research

- `vulkane` 0.16.0 is MIT or Apache-2.0, generated from `vk.xml`, and has a safe wrapper over surfaces, swapchains, pipelines, descriptors and images.
- It's proven on `solapp`: the `test` project's renderer creates its surface from a `solapp` window handle, and can also render with no window at all.
- That project's copy adds `Image::new_2d_array` with explicit mip levels and `Sampler::new_mipmapped`, recorded in its `SOLSTICE-CHANGES.md`. Quake's textures ship with four mip levels, so those additions are needed.
- Its `fetch-spec` feature downloads `vk.xml` at build time over a TLS stack with a [known advisory](../pre-research/stack.md). The crate carries `vk.xml`, so the feature isn't needed.
- Its `naga` feature compiles WGSL to SPIR-V at startup, so no shader compiler is needed on the build machine.

## Design

- The patched copy goes in `vendor/vulkane`, excluded from the workspace, as `winit` and `micropool` are.
- `engine_render` is the only crate that depends on it, which the crate-graph test already enforces.
- `fetch-spec` stays off, and `naga` is on.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a vendored copy has to be updated by hand, and it's the price of the mip additions.
- **Acceptance criteria:** the workspace builds with no download of `vk.xml`, and `cargo tree` shows `vulkane` only under `engine_render`.
- **Future extension:** the additions could go upstream, which would remove the copy.

## Review

See [review 0005](../reviews/0005-increment-2.md).
