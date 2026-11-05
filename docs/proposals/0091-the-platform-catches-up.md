# Proposal 0091: the platform catches up

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) wants the editor to have a simple side through prefabs and an advanced side with Luau scripting, entity building and support for all the rendering features, with Claude able to drive it. [The editor's proposal](0074-the-editor.md) has front ends call the same operations.
- Since it was written the map format gained meshes, decals, stamps, paint, splines, tiles and more, none of which the operations know about, and the view still draws under the panels.

## Design

### Operations

- `add_block` adds anything the format allows, and `set_block` makes or changes a block a map has one of, such as `sky`, `fog` or `water`.
- `place_mesh`, `add_decal`, `add_stamp`, `paint` and `add_spline` say what each needs. `append_point` grows a stroke or spline. `set_material` covers a whole brush. `list` says what the content offers.
- `read_material` and `write_material` edit a material's text, which must parse. `read_script` and `write_script` edit the map's own script.
- Moving things now moves their lines too. One function does it, for moves and for prefabs, and it knows a sun's `light` is a direction.

### Scripts in maps

- A map's own Luau sits beside it as `maps/<name>.luau`, and loads after the game's. So an entity type is a function in that file, and a prefab can place it: that's a prefab entity. `Level::scripts` carries it, so saves keep it.

### Editor

- The 3D view is drawn into the middle panel, and picking and overlays work in that rectangle.
- A tool is a prefab, a mesh, a decal, a stamp, a paint layer or a spline. A click places it on what's under the pointer. Strokes and splines grow with each click until they're ended.
- The left panel has tabs: prefabs, surfaces, world, materials and script. World adds sky, fog, water and lights. Materials applies one to the selection and edits its text. Script edits the map's script.
- Splines draw as a floating line with bubbles at their points, as the note asks.
- Moving lights show in the view, along their paths.
- `camera.rs` moves to a library, so it has tests.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a decal's normal comes from the box under the pointer, so it suits brushes.
- **Acceptance criteria:** each operation works from the command line and leaves undo working, a bad material is refused, a move carries a spline, a map's script runs in the game, the view stays inside its panel, picks land where clicked, and a pixel and a ray agree.
- **Future extension:** none planned.

## Review

See the increment 11 review.
