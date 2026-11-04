# Proposal 0039: the canvas

Status: accepted, 2026-11-04, increment 8.

## Research

- `draw.c` has one idea: pictures and characters at positions on a 320 by 200 screen or larger. Everything flat in the original is that, plus a solid fill and a fade.
- Pictures come from two places: `gfx.wad`, a WAD2 file of small pictures, and `.lmp` files with the same layout. Index 255 is clear. The font, `conchars`, has no header, and index 0 is clear, as `Draw_Init` arranges.
- The FPS counter's overlay from proposal 0021 said the UI work would replace it. It had its own font and its own pipeline.
- The engine doesn't know Quake, and `engine_ui` can't depend on `fixture_quake`. What's shared has to be plain values in `engine_core`.

## Design

- `engine_core::canvas` holds `Canvas`, `Quad`, `Image`, `Picture` and the `PictureSource` trait. A canvas measures in units and scales to whole pixels, so a picture's own pixels stay even.
- Anything that draws asks a canvas, by name. It doesn't know where pictures come from or who draws them.
- `Drawables` gains a list of quads, which the renderer draws last. Each picture is its own small texture, and runs of quads that share one are a single draw.
- `fixture_quake::wad` reads the file formats, and `engine_app` joins them to the renderer as pictures are first asked for.
- The scale is the screen's height over 360, or `ui_scale`.
- `gamma` is applied at the end of the world, model and sprite shaders, rather than by rebuilding a palette, since nothing is drawn through one.

## Proposal

- **Selected approach:** the above, with `overlay.rs` removed.
- **Tradeoffs:** the frame rate now needs the game's font, so with no game data it isn't shown. A picture per texture is more binds than an atlas, at around 50 a frame, which is nothing, and it needs no packing or partial uploads.
- **Acceptance criteria:** a GPU test shows a picture with clear pixels, and a solid colour blending, in the right place. Every picture the screens use is found in the game's data.
- **Future extension:** text from a scalable font, or a second font, is another picture name.

## Review

See [review 0011](../reviews/0011-increment-8.md).
