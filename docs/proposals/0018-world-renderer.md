# Proposal 0018: the world renderer

Status: accepted, 2026-11-03, increment 2.

## Research

- The pre-research chose static meshes grouped by texture, the PVS, one lightmap atlas with styles blended in the shader, and texture kinds by name.
- `gl_warp.c` gives the original sky and liquid formulas. Sky is two 128-pixel layers scrolling at different speeds, with the view direction flattened by 3 in Z. Liquid moves each coordinate by `8 * sin(other * 0.125 + time)`.
- `r_surf.c` and `d_init.c` give the look. A light value of 128 leaves a texture unchanged, so a lightmap sample doubles before it multiplies. Palette entries from 224 up ignore light.
- Original colours are in display space, so the swapchain needs a UNORM format, or the sums would come out different. `vulkane`'s `pick_surface_format` prefers SRGB, so the engine picks for itself.
- A render pass is compatible with another of the same formats, so pipelines can draw to the window and to an off-screen image.
- `naga`'s SPIR-V writer sets `ADJUST_COORDINATE_SPACE` by default, which flips y for Vulkan, so shaders output WGSL's y-up clip space.

## Design

`engine_render` exposes `Renderer`, `View`, `Capture` and `RenderError`, with no `vulkane` type.

- `world.rs` turns a `Map` into vertices, index ranges and an atlas. It's plain data, so it's tested with no GPU.
- All the styles of a face sit side by side in the atlas. A vertex carries the first position and the step to the next, plus four style numbers.
- `Frame` uniforms hold the camera, time and 64 style values, so animated lights cost no uploads.
- When the view leaf changes, the visible faces are rebuilt into one index buffer, grouped by texture. Brush models are always included.
- Four pipelines share one layout: lit, cutout, liquid and sky.
- One frame is in flight. The renderer waits for the last frame at the start of the next, not at the end.
- Mailbox is preferred, with FIFO as the fallback. The `vid_vsync` cvar selects FIFO, which paces frames evenly at the cost of some latency.
- `capture` draws the same scene to an off-screen image and returns its pixels. It's used for `screenshot`, for tests with no window, and later for small preview images.
- The window handle reaches the renderer as `engine_core::window::NativeWindow`. `engine_platform::Window` holds the `solapp` lease for as long as the renderer lives.
- `fov` is horizontal at 4:3 and widens with the window.
- Textures use nearest filtering, for the original look.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** one frame in flight is simple and leaves some GPU overlap unused. It's fine at Quake's scale, and easy to raise later.
- **Acceptance criteria:** tests cover mesh building, atlas packing and matrices with no GPU. A GPU test draws a small synthetic map off-screen, and skips with no Vulkan device. Real maps draw correctly in captures.
- **Future extension:** alias models, dynamic lights and particles add pipelines. The reduced mode can draw through `capture`.

## Review

See [review 0005](../reviews/0005-increment-2.md).
