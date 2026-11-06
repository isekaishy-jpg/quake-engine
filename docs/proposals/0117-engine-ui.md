# Proposal 0117: the engine's own UI

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

The open question was the human's design: a real text renderer, nine-slice frames, menus and animated pictures, for the art in `content/ui` and `content/fonts`. Cultist Sans is proportional, so the console needs its own answer. That was decided here, as the sprint asks.

- Text was Quake's `conchars`: 8 by 8 bytes from a picture.
- The kit comes with atlases at three densities, JSON with each piece's rectangle and slices in source pixels, and a reference for how nine-slice works: corners keep their size, in logical units, whichever density they're read from.
- Moving pictures come with `animation.json`, with each frame's time. The spinner is eight kit pieces at 80 ms.
- [`fontdue`](https://docs.rs/fontdue) rasterizes TrueType in Rust, and is small. There's no shaping, which Western text doesn't need. A shaper would matter for scripts that join up.
- The art stays out of git until the human says, and no test or build may need it, so all of it has to be optional.

## Design

- **`UiTheme`** in `engine_ui` holds fonts, a kit and moving pictures. A game without them goes on as it was.
- **Fonts bake on demand,** by style and size in pixels, to a picture of Latin characters, so text is sharp at any scale.
- **Text** has a measure, wrapping between words, and breaking a word too long for a line.
- **Frames:** `nine_slices` works out the nine rectangles, and `draw_frame` draws them. `draw_piece` draws icons and controls.
- **The console's answer:** a grid. Each character sits in the middle of a cell of its own, so columns line up though the font doesn't.
- **Pictures from content.** `.png` names load through the same bank as the rest, and `Canvas::pixels` draws exact rectangles of them.
- **In the game:** the splash as it starts, the console's background and text, a zone's title in the display font as it fades, and the spinner while a level loads.
- JSON is read in `engine_app`, which already has `serde_json`, and `engine_ui` only sees the result.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** no shaping, so scripts that join letters aren't possible yet. Fonts bake Latin only, as that's what Cultist Sans holds.
- **Acceptance criteria:** with the art there, the engine starts on its splash, the console and titles use it, and frames keep their corners. Without it, nothing changes.
- **Future extension:** menus built on this are item 17.

## Review

The console, a zone's title and its light were looked at in a test level. Tests cover measuring, wrapping, nine-slice and moving pictures, with no art needed.
