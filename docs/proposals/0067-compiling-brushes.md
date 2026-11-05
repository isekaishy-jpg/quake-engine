# Proposal 0067: compiling brushes

Status: accepted, 2026-11-05, increment 11.

## Research

- Our own format means `qbsp` can't build our maps, so turning brushes into what the engine uses is ours to write.
- `engine_map::Map` already says what the engine needs: faces, a point tree, clip trees for player and large sizes, models and entities.
- Every convex brush is the set of points behind all its planes. Inserting a brush into a tree needs no polygon splitting: at a leaf, a chain of its planes marks the inside, and at a node the brush goes to whichever sides it reaches. The result is exact for any overlap.
- `qbsp` grows each brush by the box's support along each plane, and adds axial bevels so sharp corners don't reach too far.
- With no visibility data, `Tree::mark_visible` shows everything, so a working map plays with no visibility step.

## Design

- `engine_compile` is a library, so the game, the tools and a command line all use it. It depends on `engine_core`, `engine_format` and `engine_map`.
- `source` reads a map document into typed brushes and entities. A `cuboid` field is shorthand for six planes. Unknown fields on entities pass through.
- `geometry` clips polygons in double precision. Faces are cut against other solid brushes, so hidden parts go. Touching faces both go, and of two coplanar faces the first stays.
- `tree` builds the point tree and both clip trees by insertion.
- Each brush entity becomes a model, named `*1` onwards, as the engine's scripts already expect.
- Leaves are shared by contents until a visibility compiler needs them split.
- The app loads `maps/<name>.emap` when it exists, and otherwise goes to the game's own importer. So a map plays straight from its text.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** tree size grows with overlapping brushes, and T-junctions between cut faces can show as cracks. Both go on the plan's review list.
- **Acceptance criteria:** a compiled room has walls a player collides with at the right distance for both hull sizes, a point trace stops at the surface, overlapping brushes have no faces inside them, water reads as water, a door entity gets its own model, and a real server and client play a map from text with the fixture's scripts.
- **Future extension:** placements, lighting and visibility join the compile in later stages.

## Review

See the increment 11 review.
