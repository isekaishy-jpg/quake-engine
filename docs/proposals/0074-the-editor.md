# Proposal 0074: the editor

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) leans to Rust for the editor, as simpler even with some extra work, in the style of Campfire and Scopework, with a simple prefab workflow and advanced options on hand.
- `egui` outputs textured triangles, so it needs no toolkit of its own to draw: the existing canvas pipeline can take them, and panels and the 3D view share one window with no hosting limits. Scopework already uses it.
- The host reports pointer position, which `engine_platform` didn't pass on.

## Design

- `engine_editor` is a binary, `engine-editor <content> [map]`, on the same `Session` and operations as every other front end.
- `Renderer::set_ui` takes triangles with clip rectangles, drawn after the canvas. Nothing in `engine_render` knows `egui`.
- `InputEvent::Pointer` gives the pointer's position. Text comes from key names on a US layout.
- The look follows Campfire's flat Precision direction: dark square panels, a thin amber accent, monospace text and a status bar.
- Simple: pick a prefab, click in the view to place it, snapped to 8 units, with JSON parameters. Advanced: every thing's fields are editable, and fields can be added.
- The right button flies the camera, with WASD, Q and E, and Shift for speed.
- The view recompiles after every edit, so it's always the real result. A bake that's current shows lit, and a stale one shows the map unlit, as the game does.
- JSON lines on standard input run the same operations live, plus `capture`, `camera`, `select`, `tool` and `click`, so an agent can work on the map a person is watching.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the 3D view draws under the panels rather than in its own rectangle, so it spends some pixels unseen. Text input assumes a US layout.
- **Acceptance criteria:** the editor opens a map, shows it with the real renderer, places a prefab by clicking, selects by clicking and from the list, edits a field, and saves, compiles and plays through the same operations.
- **Future extension:** further tools join the shell.

## Review

See the increment 11 review.
