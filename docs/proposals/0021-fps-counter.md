# Proposal 0021: the FPS counter

Status: accepted, 2026-11-03, increment 2.

## Research

- The human's direction: an FPS counter that updates no faster than 4 Hz, doesn't affect performance, and is smoothed so it gives a stable number to read.
- The engine runs borderless by default, so a title bar can't show it, and it has to be drawn. There's no text drawing yet.
- Quake's own font lives in the game's data, in `gfx.wad`. A counter shouldn't need game data, or wait for the UI work in increment 8.
- Exponential smoothing with a fixed time constant gives a steady number that still follows real changes, and it needs no history.

## Design

- `engine_core::time::FpsMeter` counts frames and returns a whole number at most every 250 ms. Each reading is a frame count over its real span, smoothed with a one second time constant. It's plain arithmetic, so it's tested exactly.
- Per frame it costs one addition and one comparison.
- `engine_render` gets `set_overlay_text`, with a small built-in 5 by 7 font for digits and a few letters. Text sits top right on a dark backing, scaled to the window's height.
- The overlay's vertices are only rebuilt when the text or size changes, so at most four times a second. Drawing it is one small draw call.
- The `show_fps` cvar turns it on and off. It's on by default, and saved.
- The meter resets in reduced mode, so a minimized spell doesn't drag the number down afterwards.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the built-in font only covers what the counter needs. The UI work replaces it.
- **Acceptance criteria:** tests show a steady rate reads exactly, updates never exceed 4 a second, an uneven rate reads steadier than its raw readings, and a real change comes through. The overlay shows in a capture. Frame rate with it on matches frame rate with it off.
- **Future extension:** frame time and other readouts can share the overlay.

## Review

See [review 0005](../reviews/0005-increment-2.md).
