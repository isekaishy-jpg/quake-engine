# Proposal 0103: levels that load

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

Vesper Deep took 5 seconds to load, and review 0016 put 3.7 of them down to compiling the map from its text at each load. Timing the stages:

| Stage | Time |
|---|---|
| Reading the text | 28 ms |
| Materials and meshes | 7 ms |
| Compiling, of which textures are 143 ms | 3.5 s |
| Physics | 590 ms |
| Renderer | 260 ms |

- `.ebin` is only the map's document as binary. Nothing compiled is kept.
- Textures weren't the cost, as was assumed. It was geometry.
- Writing geometry to a file gave 1.5 GB, for a map of 24,000 faces. The tree had 492,532 point nodes and 75,492,456 clip nodes: about a gigabyte in memory at every load.
- A brush goes into a tree down whichever sides its corners reach, and puts a chain of its planes at each open leaf. A second brush overlapping it reaches those chains and does the same at each of their leaves, so overlapping brushes multiply. Clip hulls grow each brush by the player's size, so they overlap far more than the brushes do. Nearly all those nodes say the same as others.

## Design

- **Trees share what's the same.** Nodes never change. A node's number is found by what it says, so identical parts are kept once, and a node with the same thing either side is that thing. A brush makes new nodes for what it changes, and remembers what it made for each node it passes, so it goes through what's shared once. What's left behind is dropped at the end.
- A tree is a graph now, not a strict tree. Everything that reads one walks down from the top, so nothing that uses it notices, and nothing needs to change.
- **Geometry is kept.** `engine-compile` writes `maps/<map>.egeo`: points, faces, models, lifts, the tree, and the materials in order. It carries the same hash as a bake, so anything that would make a bake stale makes it stale, and then a load compiles as it did. A file that won't read does the same.
- What needs a level's pictures, or takes no time, still comes from the map: entities, sky, stencils, pools. `compile_known` runs the ordinary compile with models already there, so there's one path.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** one more file beside each bake. It's ignored by git, as bakes are.
- **Acceptance criteria:** a bake made with the new trees is byte for byte the bake made with the old, so they say the same. Overlapping brushes stop multiplying. Kept geometry gives the same map as a compile. A damaged or stale file is refused.
- **Future extension:** none planned.

## Review

| | Before | After |
|---|---|---|
| Vesper Deep's clip nodes | 75,492,456 | under 960,000, held by a test |
| Geometry written out | 1.5 GB | 13.6 MB |
| Vesper Deep, start to in the game | 5.0 s | 1.55 s |
| The Ember Keep | not measured | 1.0 s |

These times include starting the program. Without the kept file, Vesper Deep takes 2.15 s.

The bakes of the Ember Keep and Vesper Deep came out byte for byte the same on both trees.

A test of 14 overlapping boxes has 28,697,808 clip nodes on the old trees, and under 4,000 on these.
