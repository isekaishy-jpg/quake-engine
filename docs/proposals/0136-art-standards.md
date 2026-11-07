# Proposal 0136: standards for art and meshes

Status: accepted and written, 2026-11-06, the [work list](../research/work-list.md), items 26 and 133.

## Research

The human's word on the texture standard: it's poor, and doesn't account for textures that aren't tiled, how we pack them, the system we use to save space, or kitbashing, and models can now make specular and other maps too. The test repo was given as an older example of what can be done, and not to copy: sheets with named regions, UI in every state, and scenes with maps beside them.

The first standard came from one fault, a line along picture edges, so it only covered tiling sets, and told people to ask a model for colour only.

## Design

- **[The texture standard](../texture-standard.md)** starts from what kind of art something is, and has a section for each: repeating sets, trim sheets, kit sheets, unique textures, decals, cutouts, terrain sets, flipbooks, sky art, UI art and painted scenes.
- **Saving space** gets its own section, with what each picture costs, and the order to reach for things: reuse, share, pack, size by distance, kits first.
- **Asking a model** now asks for each map outright, and keeps the request beside the source art. `derive_maps` is a fallback.
- **[The mesh standard](../mesh-standard.md)** is new: delivery, scale for our player, origin and snapping, a triangle budget, texturing, collision, prefabs, and what to ask a model for.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the terrain and flipbook sections say what's known now, and firm up as that work lands.
- **Acceptance criteria:** the human can make textures to it with no more said, and a model can be given it with a request.
- **Future extension:** none planned.

## Review

Each number was checked against what the engine does: BC7 and BC5 as compiled, 32 units to a metre in `import_gltf`, a player's size, and `alpha`, `repeat`, `parent` and `metal_rough` as materials read them.
