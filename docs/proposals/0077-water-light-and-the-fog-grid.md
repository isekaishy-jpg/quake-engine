# Proposal 0077: water light and the fog grid

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) asks how light enters water and how it behaves in fog. Light that reaches a pool should show on what's in it, and depth should change colour.
- Real-time water usually gets caustics from a looping or procedural pattern projected along the sun, depth tint from an exponential of path length, and shafts from sampling the same pattern through the volume. None of them needs a simulation.
- Fog that shows light needs to know the light at points in the air. Froxel volumes compute that per frame. Our bake already stores it in the probe grid, so a march can read it for almost nothing, and it agrees with what the bake put on surfaces.

## Design

- Compiling records `Map::pools` from the world's liquid brushes: bounds, contents and a look. A map's `water` block sets the look, with defaults by contents.
- The final pass cuts each view ray to the nearest pools. Path length gives depth tint. A point on a surface in a pool gets a caustic pattern, projected along the sun, thinning with depth. A ray in a pool collects the same pattern as shafts.
- A pool's surface can be `alpha = blend` with `kind = liquid`, so the floor shows through from above and all of this applies to it.
- `r_fog_grid` turns the height fog's closed form into a march that reads the probes, uploaded as stacked slices, so fog is bright by lights and dark away from them. Maps with no probes keep the closed form.

## Proposal

- **Selected approach:** the above, all in the final pass.
- **Tradeoffs:** pools are boxes, so a sloped liquid brush gets its bounds. Eight pools can show at once, nearest first.
- **Acceptance criteria:** a pool shows caustics that move and are thinner with depth, deeper water is more tinted, shafts show from under water, and a lit fog is brighter near a light than away from it, with a capture of each and tests for pools from brushes.
- **Future extension:** none planned.

## Review

See the increment 11 review.
