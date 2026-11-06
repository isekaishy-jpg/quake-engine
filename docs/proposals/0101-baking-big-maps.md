# Proposal 0101: baking big maps

Status: accepted and implemented, 2026-11-05, increment 12.

## Research

- `wide_world`, of 113,345 faces, took 40 minutes to bake, where `vesper_deep`, with a fifth of its samples, took 80 seconds. Something grew faster than the map.
- Timing each stage on 6 by 6 and 12 by 12 versions of it: direct light went from 9 seconds to 130 for four times the faces, and bounced light was most of the rest.
- Every ray asked about every mesh and every pane of glass in the map. Those grow with a map, and so do its rays, so the cost went with the square of its size.
- A ray to the sun ran 16,384 units, far above the world, and a long ray has to cross every plane in the tree that it passes, even in empty air.
- Each thread was handed a fixed share of the faces, so all of them waited on whichever had the busiest part of the map.
- A sample's bounced light asks up to 1,152 sources. Bounced light is soft, and [ericw-tools](https://ericwa.github.io/ericw-tools/doc/light.html) works it out more coarsely than direct light for that reason.
- Paths were worked out on one thread, which was 15 seconds of what was left on the small map.

## Design

- `RayGrid` keeps meshes and glass by where they stand, and a ray asks only about what's near it. Something across several cells counts once, as glass tints what passes through it once.
- A ray to the sun stops just above the top of the world.
- Threads take the next few items as they come free. Results still come back in order.
- Bounced light is worked out at every second sample, and what's between is drawn from it. The two samples at each end of a row are always worked out: a row's end hangs off its face, often in a wall, and drawing from that darkened corners when it was tried.
- Paths are worked out a row at a time, in parallel. Linking only reads places, so it can be too.

## Proposal

- **Selected approach:** the above. `wide_world` bakes in 2 minutes 17 seconds, and all twenty maps in 6 minutes.
- **Tradeoffs:** bounced light loses detail finer than twice the sample spacing, which it barely had. Older bakes are ignored.
- **Acceptance criteria:** a ray meets what's near it once and nothing that's far, each face keeps its own light however the threads fall, bounced light has no steps in it, and lighting tests pass as they did.
- **Future extension:** none planned.

## Review

Timed on all of the maps. The dark edge on bounced light was found by a test written for smoothness.
