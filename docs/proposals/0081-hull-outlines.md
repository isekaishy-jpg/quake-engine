# Proposal 0081: hull outlines

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) keeps two outline techniques for testing, an inverted hull and a post-process, with some combination the likely result. The post-process came with [the final pass](0075-the-final-pass.md).
- An inverted hull draws a model again, pushed out along its normals, with its front faces culled, so only a rim shows behind it. It follows a model's own shape and lets depth hide it properly, and it needs normals, so it's for models.

## Design

- `alias.wgsl` follows `frame.wgsl` in one module, so it can read the outline's settings.
- `vs_hull` pushes each vertex out by the outline's width, scaled with distance so a line keeps its width on screen. `fs_hull` is the outline's colour.
- `r_outline_hull` turns it on, apart from `r_outline`, so the two can be tried alone or together. The view's own weapon has no hull.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** each model is drawn twice with it on.
- **Acceptance criteria:** a model shows the outline's colour where its hull shows, and nothing with it off.
- **Future extension:** none planned.

## Review

See the increment 11 review.
