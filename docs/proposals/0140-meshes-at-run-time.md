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
- **Making them:** `import_gltf` takes `<name>_lod1` and so on if the file has them, as an author's hand beats a tool. If not, it simplifies with `meshopt`, each from the last, and `make_levels` does the same for a mesh already in the content. Levels are kept in the `.emesh`, as extra triangles on the same corners. That's a new dependency, for the tools only, as levels compile when a level loads and a finished game mustn't carry it.
- **Changing** is a plain switch. A crossfade was designed and built, and the review has what it found.

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
- **Scripts:** `engine.attach_point(entity, name)` says where one is now, and `engine.attach(entity, parent, name)` keeps an entity there. `engine.place(name)` finds one in the level. An entity with `attach` starts at one, and with `attach_to` it stays on it, effects included.
- **Copies of a prefab** carry a number, so each finds its own place first.
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
  - `meshopt` is a C++ library behind a Rust crate, in the tools only, and the finished game doesn't carry it. `meshopt-rs`, a pure Rust port, has no simplifier and was last updated in 2022. The engine needs a C++ compiler for Luau already.
  - Linear skinning loses volume at a twist unless a model has a twist bone. The mesh standard will say so.
- **Acceptance criteria:**
  - A prop placed many times costs its triangles once in memory, and one draw a part.
  - Levels of detail change with no flicker, at any resolution.
  - A real model loses triangles, and not just a clean generated one.
  - A skinned, morphing model from Blender moves in the game as it did there, lit, with an outline where it should have one, and casts into shadow maps.
  - A thing attached to a joint stays on it.
  - Old levels and the fixture look as they did.
- **Future extension:** culling on the GPU, if a level ever needs it. Shadows from moving lights on the list's second pass will find skinned meshes ready, as they're plain vertices by then.

## Review

### Item 7: the mesh and instancing

Built as designed, with one simplification: the compiler doesn't list leaves. A copy keeps its faces in the tree, so a face seen means its copy is seen, and nothing new is stored. An instance takes its light from `make_vertex` on its first face, so it works with light tiles, where a far face has one texel of its average.

Tried in the proving ground beside a baked barrel: copies turned, stretched and tipped on their side all stood where they should, took the flame's moving light, and cast on the floor. The light on a copy differs from the baked one, as it should, as it's one sample.

Found in review: a placement scaled to nothing would have made a NaN of a normal. The kept geometry had no format, so a file from before this would have been misread and not refused. It has one now.

Limits, as they stand:

- **The GPU keeps a mesh once, but the map on the CPU doesn't.** Each copy's triangles stay in `Map::faces`, for the bake and for leaves. That's the cost of finding copies by leaf, about 150 bytes a triangle. It matters at many copies of a heavy mesh, and it hasn't been measured on a level like that yet.
- **See-through copies aren't sorted** against each other or the world's see-through faces. An opaque prop is what this is for, but a glass one in a row could draw wrongly.
- **Copies skip the depth pass,** so they pay for what's behind them in the same draw.

### Item 8: level of detail

Built, with two changes from the design.

**Levels are made at import, not when compiling.** Levels compile when a level loads, so making them there would have put `meshopt` in the finished game.

**The crossfade was built, measured and taken out.** It was two draws with complementary dither masks. A copy can show either level at one spot, by which way it came from, because of the margin. That gave the exact size of a change with the camera still, on a ball of 6,016 triangles, over the ball's pixels:

| Change | Changed by more than 2 of 255 | More than 24 of 255 |
|---|---|---|
| Finest to next, large | 0.53% | 0.01% |
| Next to third, small | 0.18% | 0.00% |

So a change is a few pixels at the edge. A third test, finest to next on the small ball, read exactly zero, and probably never crossed the boundary, so it isn't counted. Against that, the fade cost every mesh pixel the early depth test, as a shader that can discard loses it, and each change drew twice. It wasn't worth it, which is what the design said to do then. The margin stays, as that's what stops flicker. These are still pictures, so **how a change feels in motion needs a person.** The fade was never seen working part way: the test harness couldn't catch it, and it went on the measure of the change alone.

**The first version did nothing on real models,** and its test passed. Working out normals before simplifying split corners wherever smoothing differed, so `vesper_fall`, of 5,558 triangles, got no levels. A clean generated well simplified fine, which is why the test didn't see it. Now it simplifies the mesh as it's joined, and each level works out its own normals from its own shape. `vesper_fall` goes 5,558, 2,690, 1,302, 644. The first test written for this also passed with the bug put back, so it was replaced by one using that real file, which fails with the bug.

A mesh that can't lose much without changing shape keeps most of its triangles: the statue goes 288, 264, 238, 204. A barrel of 120 gets no levels, which is under the budget anyway.

### Item 9: attach points

Built as designed. Static places come first, and joints join with item 10.

Tried in the proving ground with a real `func_door`, a mesh of a ball with a place on top, a box of shells and a campfire, both `attach_to` it. The player walked into the ball and it rose, with the box staying on top. The flame went with it, and its light left the floor. The first try showed nothing, as a teleport doesn't count as touching a door.

- **An effect isn't an entity,** so it can't be held as one. `EffectPoint::model` had been there unused, and it follows the model's offset on the client. That means an effect on a model that also turns won't turn with it, but brush models don't turn in the renderer either.
- **A place follows by the model's offset,** so a brush model that turns doesn't carry its places round. Models don't turn when drawn yet, so nothing shows this. It changes with skeletons, where a place goes by its joint.
- **Something held to something held** can lag a tick, by the order they're in.
