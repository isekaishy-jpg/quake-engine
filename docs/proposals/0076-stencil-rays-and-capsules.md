# Proposal 0076: stencil rays and capsule shadows

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) records the human's stencil idea: an invisible surface, like a specular map, in a window, grate or canopy, where colour tints and alpha blocks. Each glass piece is a ray, its frame cuts the light, and an extra authored line groups pieces into a box for occlusion, which uses existing collision. A box fades between blocked and clear.
- A plain cookie doesn't put a pattern in the air, and a short march along the view does, at a cost set by step count and screen size.
- Capsule shadows ground characters cheaply, steered by a dominant light direction from the bake.

## Design

- `ray_stencil` entities compile into `Map::stencils`, with an `image`, a `corner`, `across` and `down` edges, a `light` position or `sun`, `strength` and `reach`. An optional `marks` image has red lines grouping pieces into boxes, and a tool step reads them, so the runtime only sees boxes.
- The final pass marches 16 steps along each view ray to the surface behind it. Each step projects towards the light, finds where that crosses the stencil, and reads a blurred mip, so rays stay chunky and bright panes stay coloured.
- Up to four stencils draw a frame, each with a 16 by 16 occlusion grid in the frame uniforms.
- The app traces one ray per box through existing collision, starting past the surface, and eases each box towards open or blocked over a short time.
- `Drawables::capsules` lists characters. World and material shaders dim light behind each one, along the direction from the probes, softly, with no shadow map.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a box goes blocked or clear as a whole, which the fade softens. Capsules are soft blobs, not silhouettes.
- **Acceptance criteria:** a stencil in a dark room shows tinted rays that stop at surfaces, lines group into the right boxes, a mask closes a box, and a capsule darkens the floor on the side away from the light, with controls.
- **Future extension:** none planned.

## Review

See the increment 11 review.
