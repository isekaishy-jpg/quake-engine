# Proposal 0075: the final pass

Status: accepted, 2026-11-05, increment 11.

## Research

- Outlines, fog, post anti-aliasing and stencil rays all need a finished scene's colour and depth, and the renderer drew straight to the swapchain.
- Post-process outlines read depth, and [forward renderers have no normal buffer](https://mert-kaan.is-a.dev/projects/outline-shader), so normals are rebuilt from depth.
- SMAA proper needs three passes and lookup textures. The human wants cheap and not blurry, so a single edge-aware pass comes first, and it's named for what it is. MSAA stays a separate stage.

## Design

- The scene draws into an off-screen colour and depth target, sized to the output, and a full-screen pass in the present pass reads both, before the canvas.
- `post.wgsl` smooths edges by blending a share of a pixel along them, draws outlines from depth gaps and normal creases, and adds height fog.
- `r_aa`, `r_outline` (0 off, 1 full, 2 semi) with `r_outline_width`, `r_outline_fade` and `r_outline_color` are settings. Semi-outlines fade with distance and lighten creases.
- A map's `fog` block sets colour, density, base height and falloff.
- The frame uniforms gain the inverse view projection and these settings. `frame.wgsl` holds the struct, so shaders share it.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** one extra full-screen pass, always, and one more colour and depth image.
- **Acceptance criteria:** with everything off, a capture matches one from before, each setting changes a capture, and fog thickens with distance.
- **Future extension:** stencil rays join this pass.

## Review

See the increment 11 review.
