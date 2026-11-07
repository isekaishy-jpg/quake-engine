# Proposal 0140: meshes at run time

Status: proposed, 2026-11-06, the [work list](../research/work-list.md), items 7 to 12.

## Research

[The note](../research/mesh-runtime.md) has the sources. In short:

- World meshes are baked into the map, and each placement costs its triangles again. Only alias models move, and they have a path of their own. There's no run time mesh, and all six items need one.
- glTF defines skinning as linear, with morphs before skinning and a skinned node's own transform ignored.
- Dual quaternions are a little slower, take no scale, and bulge at bends. A twist bone answers linear blending's worst fault.
- Sparse morphs save a great deal, and gathering by vertex needs no atomics.
- The renderer has no compute and no storage buffers yet, though `vulkane` has both.
- Probe-lit meshes already exist: a mesh's whole light is one chart in the atlas, and `normal.w` tells the material shader to give that light a shape.

## Design

**One new thing: a mesh on the GPU, drawn wherever it's asked for.** Everything else is a case of it.

### The mesh (items 7 and 8)

- **`Renderer::add_mesh`** uploads a mesh once: vertices in the world's own `Vertex` format, in the mesh's own space, indices, its parts by material, its levels of detail and its bounds.
- **`vs_mesh`** is a second vertex entry in `material.wgsl`, beside `vs_main`. It reads a second vertex binding at instance rate: three rows of a matrix, then the light chart's place, step and styles, and a fade. It sets `normal.w` to 1, as a probe face has. **The fragment shaders don't change,** so every material and surface shader works on a mesh with nothing new to write.
- **Levels hold the meshes they use.** The compiler gathers them from placements, entities, and a `meshes` list on the level for what scripts spawn, with materials resolved as they are for faces. This is Quake's own rule, that models are known when a level loads.
- **Which placements are instances:** those with `light = probe`. They have one light each, so they lose nothing. Others stay baked, with a light map of their own, as a hero piece wants. So old levels look as they did until an author asks.
- **Light and collision don't change.** A placement's triangles still go to the bake, to cast shadows, and to collision. They just aren't drawn as faces.
- **Visibility.** The compiler lists the leaves each instance touches. When the view changes leaf, the visible set is rebuilt, as for faces. Each frame, those are tested against the view, given a level of detail, grouped by mesh and level, and written to the instance buffer. A part is then one draw for all its copies.
- **Shadows.** Lamps' shadow maps draw instances too, so a prop doesn't stop casting by being one.

No compute pass culls instances for now, as visibility by leaf comes first and a level has hundreds of copies, not millions. The measurement goes in the review.

**Level of detail.**

- **Choosing** is by share of the view's height, so that it holds at any resolution and field of view. By default a level changes at 0.25, 0.1 and 0.03, and a mesh can say otherwise with `lod_sizes`. Coming back needs 10% more, so that nothing flickers at a boundary.
- **Making them:** `import_gltf` takes `<name>_lod1` and so on if the file has them, as an author's hand beats a tool. If not, `compile` simplifies with `meshopt`, each from the last. That's a new dependency, for the tools only, and it's here to be approved.
- **Changing** crossfades over half a second, by complementary dither masks, on the opaque path. It's tried on a real mesh against a plain switch. The fades are capped, and past that a change is plain. If it isn't worth what it costs, it goes.

### Moving things (items 10 and 11)

**A compute pass deforms meshes before the frame's drawing, into scratch vertices in the same format.** After that a moving mesh is a mesh, drawn by `vs_mesh` with one instance. The alternative, skinning in a vertex shader, means another vertex entry for each pipeline, and repeating the work for the depth pass, the outline hull and shadows. This costs one compute pipeline, the first storage buffers, one barrier, and scratch memory for what's on screen, which has a budget. Past the budget a mesh draws in its rest pose and the console says so.

- **Skin:** linear, four joints a vertex, as `u8` indices and weights that sum to 1, so a mesh has up to 256 joints. More than one set in a file is reduced to the four strongest, and the importer says so.
- **Joints:** one storage buffer holds every palette this frame, with each mesh saying where its own begins. A joint is three rows of `vec4`, 48 bytes.
- **Morphs:** sparse, with entries sorted by vertex: which target, then position and normal differences as half floats. One thread reads its own vertex's entries, so no atomics. Morphs go before skin, as glTF has it. Targets with no weight are left out of the list a dispatch gets, and with no active target or joints the pass is skipped for that mesh.
- **Clips:** `.emesh` keeps glTF's animations, as `LINEAR`, `STEP` or `CUBICSPLINE`, sampled with rotations by slerp the short way and normalised. A clip can also move morph weights.
- **Bounds** come from the pose, not the rest pose, or culling pops.
- **Who works it out:** one function in shared code samples a clip to a pose, so that the server and client agree. An entity's state says its clip and when it began. The client blends between clips over a short time, which is only for looks. Scripts set morph weights by name.

**Light on what moves.** A moving thing has no chart in the atlas. A vertex can say so with a style of `OWN_LIGHT`, and its light fields then hold a colour. `light()` returns that, and the direction read is skipped. It's the only change to the fragment path, and the colour comes from `light_at`, as alias models' does.

### Attach points (item 9)

- **In a file,** an attach point is a node named `attach_<name>`, under a joint or not. An empty in Blender is one, so an author needs no new tool. `.emesh` keeps its name, joint, place and turn.
- **The same name** on different models lets one thing fit them all.
- **In a map or prefab,** a `point { name = ... }` block is a named place, with no model.
- **Scripts:** `engine.attach_point(entity, name)` says where one is now, and `engine.attach(entity, parent, name)` keeps an entity there. Effects and lights in a prefab can name one.
- **When:** after the pose, each frame. Reading it before is the known way to get a weapon that's off at run time.

Attach points come before skeletons in the list, and static ones are built first. Joints join them with item 10.

### The fixture's models (item 12)

An alias model is two poses blended, which is a morph with one weight, lit by its own rule. So the deform pass can write its vertices, and they draw as a mesh with a skin for a material and a Quake shading mode. **It moves if pictures of the fixture come out the same and it's no slower,** as there's meant to be one way of drawing. That's tried, not assumed. If it doesn't hold up, the review says why, and alias models stay.

## Proposal

- **Selected approach:** the above, in order: the mesh and instancing, level of detail, attach points, skeletons, morphs, then the fixture.
- **Tradeoffs:**
  - A level has to know what it can spawn, as Quake's did.
  - Instances have one light each, so a large mesh that wants real shadows on it stays baked.
  - Compute scratch memory goes with what's on screen.
  - `meshopt` is a C++ library behind a Rust crate, in the tools only, and the finished game doesn't carry it. `meshopt-rs` is a pure Rust port, which would keep C++ out of the build. It's young, at 0.1, so item 8 tries it first and takes whichever simplifies well.
  - Linear skinning loses volume at a twist unless a model has a twist bone. The mesh standard will say so.
- **Acceptance criteria:**
  - A prop placed many times costs its triangles once in memory, and one draw a part.
  - Levels of detail change with no flicker, at any resolution.
  - A skinned, morphing model from Blender moves in the game as it did there, lit, with an outline where it should have one, and casts into shadow maps.
  - A thing attached to a joint stays on it.
  - Old levels and the fixture look as they did.
- **Future extension:** culling on the GPU, if a level ever needs it. Shadows from moving lights on the list's second pass will find skinned meshes ready, as they're plain vertices by then.

## Review

Not built yet.
