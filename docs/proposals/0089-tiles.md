# Proposal 0089: tiles

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) leaves tiles to research, with rules from the human: a player never hits an unloaded tile, several tiles load and details fade in with view distance, there's no hard distance culling of the world, the server pays for simulation online, inactive entities sleep locally, regular behaviour runs from a seeded PRNG so it doesn't look reset, and long-running jobs have to make sense.
- The engines in the note share a shape: a grid, an index of what exists, a load range round the player, and something always loaded.
- The human noted that with few meshes, more can sit in memory. Collision, the server and the renderer all hold one `Map`. Splitting those would touch every system, and streaming geometry in and out buys nothing at today's sizes while risking the one rule that matters.

## Decision for the human's review

Tiles are real files under a manifest, on a coordinate grid, and all of a map's tiles load with it. So a player can't reach an unloaded tile, for any speed or teleport, by construction, and a server holds every tile for every player. Geometry doesn't stream out of memory. What the ring round each player governs is what's awake. If maps outgrow memory, loading tiles on demand is the next step, and it would need `Map` split by tile.

## Design

- A map's `tile_grid` block sets `size`, and each `tile` block gives its `x`, `y` and `file`. A tile is a document of kind `tile` under `maps/`, with everything in coordinates local to its corner, so tiles can be moved and reused.
- `MaterialLibrary::prepare` merges tiles into the working map, moved into place, then places meshes. It returns the text of everything it read, which a bake's hash now covers, so a stale tile or mesh invalidates a bake.
- `Map::tile_of` turns a point into a tile and a local position. Positions stay `f32`, and a grid wider than 32 tiles each way from the origin is refused, which keeps steps well above collision's epsilon.
- With tiles, the server puts entities to sleep beyond `wake_reach` of every player, by default one and a half tiles. Sleeping entities don't think or move, and cost nothing but memory. Waking runs an entity's `wake` function with how long it slept, so a script can catch up. `engine.seeded` gives a repeatable number from an entity and a step, for regular behaviour.
- An entity with the `FLAG_ALWAYS_AWAKE` flag never sleeps, for long-running jobs such as an NPC sent somewhere.
- A material with `detail = true` fades out past `r_detail_distance`: blended surfaces by alpha, the rest by a dither. Nothing else in the world has a distance limit.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** memory holds every tile, as the decision says.
- **Acceptance criteria:** a map from tiles plays with everything in place, a tile's edit invalidates the bake, a point maps to its tile and back, an entity past the ring stops thinking and wakes with how long it slept, a flagged one doesn't sleep, and a detail fades with distance.
- **Future extension:** loading tiles on demand.

## Review

See the increment 11 review.
