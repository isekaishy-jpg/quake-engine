# Proposal 0073: the development platform

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) decides on a tool that's more than an editor, with its own binary, driven by Claude through a command line and MCP, for testing, mapping and map testing.
- Scopework's JSON command line and Campfire's rule, that agent tools call the same operations as the UI, are the patterns to follow.
- The simple workflow goes through prefabs, and advanced options stay open. Parametric prefabs fit any gap, and Luau lets anyone add one.

## Design

- `engine_tools` is the core: a `Session` holds a map document with undo history, and every edit is a named operation taking and returning JSON. Nothing is UI-only.
- Each placed thing gets a hidden `id` field, stable across saves, and authors never set it. Compile ignores it.
- Operations: `new`, `open`, `save`, `describe`, `add_brush`, `add_entity`, `place_prefab`, `set_field`, `move`, `delete`, `undo`, `redo`, `prefabs`, `validate` and `compile`, plus `shot` and `playtest`, which run the real game.
- Prefabs are Luau files in a container, at `prefabs/<name>.luau`, returning a table with `build(params)`. They output plain brushes and entities into the map, so the format knows nothing of prefabs. A placed prefab is ordinary data from then on, which can be edited by hand.
- `engine-tool` runs operations from its arguments, or a batch from a JSON file. `engine-mcp` serves the same operations over MCP on standard input and output.
- The editor UI is a separate binary calling the same `Session`.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** undo keeps whole documents, which is simple and fine at text map sizes.
- **Acceptance criteria:** every operation works headless, undo and redo restore a map exactly, IDs survive saving and reloading, a prefab places geometry that compiles and collides, a bad operation reports an error without changing the map, and MCP lists and runs the operations.
- **Future extension:** further tools join the shell, sharing the operations layer.

## Review

See the increment 11 review.
