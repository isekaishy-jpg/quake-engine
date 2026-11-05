# Proposal 0080: surface extras

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) lists decals, hotspot texturing, reflections, flow maps and fake rounded edges for checking, since textures are the most expressive thing brushes have.
- [Hotspot texturing](https://developer.valvesoftware.com/wiki/Hotspot_texturing) marks areas of a sheet, and each face fits itself to the one nearest its shape, so trim needs no aligning.
- Flow maps move a texture along a painted direction, with two phases half a cycle apart hiding each other's restart.
- Reflections in a final pass can walk the screen, using the colour and depth we already keep. Off the picture they can only guess, and sky is a fair guess outdoors.

## Design

- A map's `decal` block presses a box against the world. Compiling clips the faces under it, lifts the pieces just off them, and gives them the decal's material and UVs. Faces it doesn't face are left alone. Each piece is an ordinary lit face.
- A material's `hotspots` lists areas of its image, and a face using it fits the area nearest its shape.
- A material's `flow` slot and `flow_strength` and `flow_speed` move its base colour.
- A material's `reflect` goes out in the scene's alpha. Blended surfaces leave that alone, which `vulkane`'s blend state is patched for. The final pass walks the mirrored ray across the screen, falling back to sky, more strongly at a glance. Pools mirror from their surfaces, with a ripple.
- `r_edges` sets a width in pixels for fake rounded edges in the final pass: from the normals around a pixel, outer edges lighten and inner ones darken.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a reflection only shows what's on screen. Rounded edges follow depth, so they show at geometry, not in textures.
- **Acceptance criteria:** a decal stays inside its box and only on what it faces, a face takes the hotspot of its shape, a flow map moves, a polished floor mirrors a pillar, a pool mirrors the sky, and edges read as rounded, with captures of each.
- **Future extension:** none planned.

## Review

See the increment 11 review.
