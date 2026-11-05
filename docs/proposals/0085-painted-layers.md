# Proposal 0085: painted layers and flattening

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) wants blended layers like WoW's terrain, sharing about eight slots, painted on surfaces in the editor, with flattening on compile as a way to cut cost.
- A mask image repeats with its texture, so it can't say where moss is on this wall. That needs a mask per surface. Light maps already give every surface a place of its own, so a paint map can use the same place and the same UVs, at no cost to vertices.

## Design

- A map's `paint` block is a stroke: a `layer`, a `radius`, a `strength` and the `points` it passes through.
- Baking works out each light sample's weight for each layer, from its distance to the strokes, into `Map::paint`.
- The renderer puts those in an atlas laid out like the light atlas. A material layer with `mask = paint` reads its weight from there, by its own position among the layers, and joins along the height of what's below as other layers do.
- A material with `flatten = true` has its image-masked layers mixed into its base when it's read, so they cost nothing at runtime. Painted layers stay.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** paint has the light map's resolution, so a soft edge is what it does best, and maps need a bake to show it.
- **Acceptance criteria:** a stroke shows a layer where it passes and nowhere else, its weight falls off with distance, a flattened material looks the same and has no layers, and paint survives a bake file.
- **Future extension:** none planned.

## Review

See the increment 11 review.
