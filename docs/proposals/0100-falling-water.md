# Proposal 0100: falling water

Status: accepted and implemented, 2026-11-05, increment 12.

## Research

- The Fall in `vesper_deep` was a tall box of water, so it read as a flat blue slab, lit from inside, on a black wall.
- A body of liquid is a box to the renderer: it tints what's seen through it and shows caustics. The swirls on the slab were those. That suits a pool and not something falling, and each one takes one of eight places in a frame.
- A mesh can follow a curve with its texture running along it, where a brush's texture is laid flat. But a mesh with no hull is given its bounds as one, and it casts shadows, so a sheet of water would be a wall that darkens the rock behind it.
- A material moves at one speed. Falling water speeds up, and it stretches as it does.
- See-through faces are drawn far to near, each by its own centre. A big see-through quad near a sheet of small ones is split by that order, one triangle on each side of them, which shows as a hard diagonal. A nearly solid layer drawn the same way leaves small dark ticks.
- A mirror that finds nothing falls back on the sky if the map has one. In a cave under a sky that shows a sky it can't see.

## Design

- A `mesh` can say `only_seen = 1`: it has no hull and light goes through it.
- `kit.py` gets `falling`, the course of water over a series of lips, and `ribbon`, a sheet down that course. Along a ribbon, `v` counts seconds rather than distance, so a texture moving at one speed stretches as the water speeds up.
- A fall is an opaque body with sheets of white water over it. The body is opaque so that it never has to be sorted. Each thread of white water has its own speed, from a flow map.
- Mist is small puffs in front of the water, in the same mesh, so that each is wholly in front of what it covers. Foam at the foot is a fine lace on the pool.
- `Map::open_above` says whether anything solid stands over a point. A mirror only falls back on the sky if the view is under it.
- Vesper Deep's canal gets its own dark water, as the old blue was the brightest thing in the city.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the Fall can be walked through, and doesn't push or wet anyone. Mirrors go by where the eye is, not where the mirror is, so someone indoors looking out at a pool sees it mirror the pool's own colour where it finds nothing. That's a small error where the old one was a large one.
- **Acceptance criteria:** nothing collides with what's only seen, light goes through it, a roof stops a mirror seeing the sky, and the city still plays from start to end.
- **Future extension:** none planned.

## Review

Seen in `vesper_deep`. The first go had big crossed quads for mist and a wide patch of foam, and the captures showed both as hard white shapes, which is how the order of see-through faces was found.
