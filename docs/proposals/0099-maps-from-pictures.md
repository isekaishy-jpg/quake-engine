# Proposal 0099: maps from pictures

Status: accepted and implemented, 2026-11-05, increment 12.

## Research

- The texture packs came as pictures only, so their stonework was lit flat. The human suggested a tool, to make what's needed from what we have.
- Tools for this, such as the normal map filters in common image editors, take brightness as height and differentiate it. Taken as it is, that makes a shadow painted on a wall into a dent, and a wide stain into a slope. Taking away a blur of the picture first keeps the mortar, cracks and rivets and drops the broad tone.
- `content/generate.py` already draws normals for its own art, so the sense of the slopes has to match what it draws.
- The pictures tile, so anything the tool does must wrap at the edges or a seam shows.
- Pictures differ in contrast. A faint limestone and a hard brick need the same usual slope, or one would be flat and the other crumpled.

## Design

- `engine_compile::derive_maps` turns a picture into a normal map and a specular map. It takes the picture's brightness, takes away its broad tone, settles fine grain, and works out slopes, all wrapping. Slopes are scaled so that a picture's usual slope comes out the same whatever its contrast.
- What stands proud shines more than what's sunk, which keeps mortar dull.
- `engine_tools` gets a `derive_maps` operation, taking one `material` or a `prefix`, and an optional `strength`. It writes `textures/<name>_normal.png` and `_specular.png`, and rewrites the material to use them. How much a material shines goes by what it's called. What flows, and sky, is left alone. Running it twice gives the same material.
- The material shader's bump response was a plain ratio of the bumped and flat light, which glared where a surface was nearly edge on to its light. It's now held to 1.5 and to a floor under the flat light.

## Proposal

- **Selected approach:** the above, run over all six packs at a strength of 0.42. Higher made stone look like modelled clay.
- **Tradeoffs:** brightness isn't height. A dark stone in a bright wall comes out sunk. That's right more often than not on stonework, and hand-made maps are always better where they exist. It adds about 33 MB to `content/textures`.
- **Acceptance criteria:** a flat picture is flat, a ridge's sides lean apart in the same sense as `generate.py`, slopes carry across the seam, broad tone isn't relief, and a material can be run through it twice.
- **Future extension:** none planned.

## Review

Seen in `ember_keep` and `vesper_deep`. With relief on, Vesper Deep's light read brighter than it should, so its ambient and bounce came down.
