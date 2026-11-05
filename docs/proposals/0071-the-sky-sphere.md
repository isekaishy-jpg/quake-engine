# Proposal 0071: the sky sphere

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) decides on a sky sphere with a gradient horizon instead of a cube, where applicable, and a cloud layer that's pre-created or procedural, as the author chooses. Indoor maps may have none.
- A zenith-to-horizon gradient is about as cheap as a sky gets, and two scrolling noise layers make clouds. Clouds pinch at the poles unless they're sampled by direction.

## Design

- A map document's `sky` block fills `Map::sky`: zenith, horizon and ground colours, a band height, a sun, and an optional `clouds` block. Clouds with an `image` use it, and otherwise they're generated in the shader.
- The sky draws first, on a cube made from the vertex number around the eye, at the far plane, so nothing needs sky brushes outdoors.
- A map with no `sky` block is unchanged.
- A face of `sky` kind in a map with a sphere becomes a window onto the same sky, for skylights in closed maps.
- Sky values go in the frame uniforms.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** procedural clouds cost five noise samples a pixel, on sky pixels only. They're a flat layer with no volume.
- **Acceptance criteria:** a map with a sky shows the gradient, sun and clouds, a map without one is unchanged, an image cloud layer and a procedural one both work, and clouds drift.
- **Future extension:** none planned.

## Review

See the increment 11 review.
