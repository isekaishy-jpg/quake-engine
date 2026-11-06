# Proposal 0104: a texture standard

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

The [direction](../research/texture-standard.md) says there's no standard, and that the next assets are made to one, above all when a model is asked for them.

- Pixels set how big a surface looks. A brush face's axes are in texels, a texel is a world unit at the usual scale, and `uv` is that over the picture's size. So a picture scaled to 256 from 226 made everything in it 13% bigger, which the human saw at once.
- Four places turn a picture's size into a surface's: `texture_size` for vertices, `add_triangle` for meshes, `hotspot_axes`, and the mega texture's `share_of` and `surface_texel`.
- The first packs needed a repair because nothing said what a picture had to be. They came with a dark line at each edge, and many didn't quite tile.
- What real engines ask for is mostly the same: power of two sizes, a stated texel density, sets of one size, and a naming pattern.
- Comparing a row with the rows near it finds a border, but tone changes across a picture also trip it. A border stands out from how much any row stands out, so that's what's measured.

## Design

- **`world_size`.** A material can say how many world units one repeat covers. `Texture::tile_size` gives that, or the picture's size, and everything that placed a surface by its picture now asks that. Without the field nothing changes.
- **`repeat`.** A material says `both`, `across` or `none`. Nothing draws differently for it. It says what's to be checked.
- **The standard** is [a document](../texture-standard.md) that can go with a request as it is, with a section on asking a model.
- **`check_textures`** is an operation in `engine-tool`, so an agent can use it too. It reads each material and says what's wrong: size, a missing field, maps of another size, a border, or edges that don't meet. `check_picture` does the looking, in `engine_compile`, and it's plain enough to test.
- Every material now states both fields. World sizes are what their pictures made them, so nothing moves.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a real ledge or join on a repeat line reads as a step, so the check can only say what to look at. The 232 pictures from proposal 0103's sprint fail the size rule, on purpose, until they're made again at a proper size.
- **Acceptance criteria:** a material's `world_size` sets how big its repeat is, in every place that places a surface. Pictures of any size stay the same size in a level if the field does. A border and edges that don't meet are found, and a picture that tiles passes.
- **Future extension:** maps change with the surface model in sprint item 7.

## Review

`check_textures` over all 128 materials finds what's known: the 232 pictures, and one sewer wall with a line along its top. It found two plank pictures with a joint along their tops, which are now `across`, as they are.
