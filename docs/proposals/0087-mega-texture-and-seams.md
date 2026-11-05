# Proposal 0087: mega texture and seams

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) lists a mega texture among the wants, and notes [id dropped it for id Tech 7 in favour of streaming](https://www.wikipedia.org/wiki/Id_Tech_7), and that layers, decals and flattening may cover most of the look.
- What's left for it is surfaces where layers and paint are many, and worth paying memory to draw with one read. Streaming pages of a huge image is the expensive part of id's system, and maps of our size don't need it: a surface gets unique texels in one image, which is what the feature is for.
- Filling and hiding seams is the second sense of inpainting. Faces already share one texture mapping per plane. Light seams come from samples at a shared edge disagreeing.

## Design

- A material with `mega = true` opts in. After a bake, each of its faces gets a rectangle of a 2048 pixel image, at one texel per texel of its mapping, with two texels of padding. Its layers and paint are mixed in, and its texture and offsets point there, with its light moving the same way, so light stays put.
- Normal, specular and emissive maps go in images of their own, laid out the same way, and only if a marked material has one. Faces of many materials share these, so each material's strengths go into the texels: specular and emissive are stored against a strength of 4, and gloss against 256 in alpha.
- The images go in the bake, as one lit material. A face that doesn't fit keeps its own material, so a full image costs detail rather than failing.
- Light samples are moved onto their faces, as [meshes needed](0078-meshes-and-occlusion-sidecars.md). Neighbouring coplanar faces then sample the same places along an edge, which fills light seams, and a test now holds that.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a material's strengths above those limits are cut to them. Parallax and flow don't carry over, as they move what's read. There's no streaming, so a map's surfaces share 16 megabytes, and 16 more for each kind of map they use.
- **Acceptance criteria:** a marked surface draws from the image with its paint, layers and other maps, in the same place as before, its light doesn't move, a bake keeps it, what's unmarked is left alone, and two coplanar faces agree along their edge.
- **Future extension:** streaming, if maps outgrow one image.

## Review

See the increment 11 review.
