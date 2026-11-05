# Proposal 0086: stamping

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) wants a stencil to turn a flat wall into bricks, or a decal with writing to carve its letters. It's detail only and doesn't change collision. Its main point is that carving normally leaves new surfaces with no texture, such as the sides and floor of a letter, and inpainting fills them.
- It suggests working maps keep the flat brush and the stencil, compile does the rest, and new surfaces default to projecting from the side, with an inner material as an override.

## Design

- A map's `stamp` block has a `material`-free box like a decal's, an `image`, a `depth`, and an optional `inner` material. The image's brightness is height: mid grey leaves the surface alone, brighter stands out, and darker cuts in.
- Compiling samples the image on a grid of `detail` unit cells, in four levels each way. It takes the face's piece under the box out, and puts back rectangles at their heights, merged along rows, clipped to the face, with the face's own material and mapping, so a texture carries straight across. Between cells of different heights go walls, which take the `inner` material if there is one, and otherwise the face's, projected from the side.
- A carved surface sits inside a solid, where a light sample would see nothing. So each new face records how far to lift its samples, past the original surface, and it's lit as if it were there. Light across a stamp varies with the wall's, and its shape comes from its normal.
- Collision comes from brushes, so it doesn't change.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a stamp's detail is in steps of its grid, so curves stair at the `detail` size. Many small cells make many faces.
- **Acceptance criteria:** a dark stencil cuts in and a bright one stands out, walls join the levels with no gaps, carved floors and walls have material and light, the flat face is gone from under it and stays elsewhere, and collision is unchanged.
- **Future extension:** none planned.

## Review

See the increment 11 review.
