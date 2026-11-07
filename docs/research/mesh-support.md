# Full mesh support

Status: a direction, not a design. This hasn't been through the workflow and nothing here is approved to build. It goes through research, design and proposal later.

The human set this direction on 2026-11-06.

## Why it came up

Things like a well belong in a level as mesh prefabs. A ring of 128 sides looked much better than one of 16 ([Shadow rendering](shadow-rendering.md)), and nobody would build that from brushes. Being a mesh didn't help by itself, though: a 16 sided mesh looked the same as 16 sided brushes.

[Proposal 0078](../proposals/0078-meshes-and-occlusion-sidecars.md) added meshes as map geometry, and that's a first version:

- `.emesh` files have no normals, so meshes draw flat.
- Nothing imports from a modelling tool. Scripts write the files.
- Collision is boxes. With none set, the mesh's bounds are one solid box, so you couldn't step inside a well.
- Each triangle is its own baked face, which doesn't suit a model of thousands of triangles.

## Chosen

- **Import glTF 2.0.** What an artist hands over and what the engine keeps are two formats, as they are for textures. Levels stay in our own format, as decided before. This only covers bringing models in.
- **Generate light map UVs automatically.** Artists don't have to supply a second set.
- **Generate collision shapes automatically.** A mesh gets convex pieces that fit it, so a well can be stepped into without anyone placing boxes by hand.

## Suggested, not settled

- `.emesh` stays the engine's format, binary by default, and gains normals, tangents, a second set of UVs, vertex colours and later skin weights.
- Skip FBX unless a real need appears. Leave USD for later. OBJ adds little once glTF is in.
- Large static meshes get one light map on their second UVs, and small props use the existing probes, by the author's choice.
- A proper mesh drawing path with real normals, which the fixture's models then move onto, as there's one way of drawing. They have: see [proposal 0142](../proposals/0142-the-fixtures-models-on-the-mesh-path.md). It overlaps with the [shader work](effects-water-shaders.md), so the two are designed together.
- A mesh file can also carry its own collision as separate parts, to override what's generated. Boxes stay as the simple choice.
- Materials match engine materials by name, with the green fallback. glTF's material model isn't converted, at least at first.
- Order: static meshes done properly, with instancing and level of detail. Then attach points, which [effects](effects-water-shaders.md) also wants, then skeletal animation, then morph targets.

## Notes for the research

These are starting points. The first search was thin, so check them all again.

- glTF stores triangles, so quads don't survive import. One baked face per flat polygon means the compiler merges flat triangles back together.
- Generating light map UVs is real work: charts, packing and padding, on any mesh it's given. It needs its own research, and a mesh that can't be laid out needs a way to fail clearly.
- Generating collision means splitting a mesh into convex pieces, which needs its own research: how close a fit is enough, how many pieces, and what a hollow shape like a well turns into. Collision goes through the map's trees now, as brushes, so pieces have to fit that or it has to change.
- [`gltf`](https://github.com/gltf-rs/gltf) looks like the obvious crate. Its [listed extensions](https://docs.rs/gltf) didn't show mesh compression, which only matters if that's wanted. A new dependency needs its own proposal.
- [One argument](https://gist.github.com/emperorofmars/d8abf0f4b9bd5434f9543511b243a254) is that glTF suits delivery better than authoring. It's from around 2020.
- [One source](https://www.a23d.co/blog/different-3d-model-formats) says FBX is in maintenance. Nothing else backs that yet.
- Tangent conventions differ between tools, so normal maps need checking on import.
