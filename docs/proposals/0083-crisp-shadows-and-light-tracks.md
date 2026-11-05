# Proposal 0083: crisp shadows and light tracks

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) wants fixed lights baked, with some technique to add crispness where it's needed, and limited motion mapped by baking keyframes.
- A light map's blur is set by its spacing. [Valve's signed distance fields](https://steamcdn-a.akamaihd.net/apps/valve/2007/SIGGRAPH2007_AlphaTestedMagnification.pdf) showed a coarse grid of distances blends into a far sharper edge than a coarse grid of values does.

## Design

- `crisp = true` on a light, or `sun_crisp = true` on the world, puts that light on style 62 without shadows, and works out each sample's distance to its shadow's edge, from a grid four times finer. `Map::light_edges` holds those distances beside the samples, going into the alpha of the atlas.
- The world shader cuts a crisp layer by that alpha, which blends between samples and stays sharp. Only one light per face can be crisp, since a sample has one edge.
- A light with `keyframes` and a `period` becomes a `LightTrack`: each keyframe is a style, and the view blends them by time, each brightest at its moment and fading into the next.
- With no period a script drives it, through `engine.light_track`, and the entity's `keyframe_style` field names its first style.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** finding edges costs 16 rays a sample for a crisp light. A soft light's blur is lost on its crisp layer, so `crisp` suits a sun or a small lamp.
- **Acceptance criteria:** a crisp shadow stays sharp at a coarse spacing where a plain one blurs, a track blends between its keyframes and comes round, and a bake keeps edges.
- **Future extension:** none planned.

## Review

See the increment 11 review.
