# Proposal 0084: live lights

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) keeps moving lights in a category of their own, on a budget, working with the others and with occlusion. Raw dynamic lights are expensive, and shadow maps add to that.
- Clustered shading splits the view into boxes and lists the lights reaching each, so a pixel only pays for lights near it. Lists usually need storage buffers and offsets. With a cap of 64 lights a cluster's list fits in 64 bits, which keeps the whole grid in a uniform buffer.
- Shadow caching redraws a light's shadow map only when it or something near it moves. Most lights in a level stand still most of the time.

## Design

- A `live_light` entity has a `color`, a `radius`, an optional spot, `shadows`, and a `path` and `period` to move along. Compiling records them in `Map::live_lights`.
- The view picks the nearest `r_live_lights` of them, at most 64.
- `cluster_masks` marks a 16 by 9 by 16 grid, with slices spaced by distance, from each light's bounds on screen. The shaders find a pixel's cluster and walk its bits. Baked light and these add up in `light()`, so materials get them too.
- A light with shadows has six 256 pixel tiles of a 2048 pixel atlas, so ten can have shadows. A tile is redrawn only when its light has moved or changed, or a moving model near it has moved, and at most two lights a frame are redrawn, nearest first, so a burst of change spreads out.
- Shadows are drawn from an index buffer of the whole world's opaque faces, plus moving brush models. Characters keep their capsules.
- Models take these lights through the same sum as other dynamic lights.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** characters cast capsules, not silhouettes, from these lights. A light past the first ten shadowed ones shines through walls until one frees up, so `shadows` should be used with care.
- **Acceptance criteria:** a live light brightens what it reaches in its colour and leaves the rest alone, a pixel only walks lights in its cluster, a wall between stops it, a shadow is kept when nothing moves and redrawn when something does, and redraws stay within budget.
- **Future extension:** none planned.

## Review

See the increment 11 review.
