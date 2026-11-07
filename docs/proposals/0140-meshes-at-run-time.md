# Proposal 0140: meshes at run time

Status: accepted and implemented, 2026-11-07, the [work list](../research/work-list.md), items 7 to 12.

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
- **Levels hold the meshes they use.** The compiler gathers them from placements, entities, and a `meshes` list on the level for what scripts spawn, with materials resolved as they are for faces. This is Quake's own rule, that models are known when a level loads. Moving meshes are kept in `Map`, as the server and the client both have that, so both see the same skeletons.
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
- **Morphs:** sparse, with entries sorted by vertex: which target, then position and normal differences as half floats. One thread reads its own vertex's entries, so no atomics. Morphs go before skin, as glTF has it. With no target showing, a mesh's corners don't look for morphs at all.
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

### Found on the way: levels that blew up

The barrels added to the proving ground for item 7 took its kept geometry from 150 KB to 670 MB, and compiling from under a second to 19. Nothing failed, and it only showed when a load seemed to hang. Bisecting by placement found that one barrel turned 45 degrees did it alone, at 564 MB. Three causes came out, each measured, after four wrong guesses:

- **Cells copied a graph as a tree.** A tree shares what's identical, so it's really a graph. `Cutter::copy` had no memory of what it had copied, so a shared node was copied once for each way to reach it, which doubles at every plane crossing a cell. Axis-aligned planes fall to one side of a cell, which is why only something turned showed it. It now remembers, within a cell. That one barrel went to 1.6 MB, and `vesper_deep`'s kept geometry from 14 MB to 7 MB.
- **A cut that doesn't divide anything.** What a cut crosses goes down both sides, so one that leaves nearly everything on each side doubles the work. A cut now has to leave either side at most three quarters. It's a small gain, but it stops a real doubling.
- **Collision was finer than can be felt.** Pieces were fitted to a voxel of 1 unit, and what collides with them is a box 32 wide. A barrel had about 36 pieces, and neighbours' pieces overlap once they're grown by that box, which multiplies. The smallest voxel is 4 units now, an eighth of a player, and half of a step's 8. At 8 units collision tests fail. The map's geometry is 454 KB.

Tried and dropped: fewer brushes to a part made no difference, and cutting along the way turned planes face made it slightly worse. Neither was the cause.

Other levels keep what they've baked until their next compile. `wide_world`'s 167 MB should shrink then.

### Item 10: skeletons

Built as designed, on Vulkan 1.3 ([proposal 0141](0141-vulkan-1-3.md)), and tried with a real model: an arm of three bones made in Blender, which waves. It moves in the game as it does there, is lit with what's around it, casts a shadow that follows its pose, and a box of shells held to its tip goes with the bone, on the real server.

- **Everything above a joint is a joint.** A rig's own node moves its bones, and being a joint it needs nothing special, even if it's animated.
- **A corner's four strongest joints** are kept, as bytes that come to 255.
- **The GPU test can fail.** With weights ignored in the shader, the test reads 0 of 21.5. With skinning broken, it reads no movement.

Found in review and in trying it:

- **A queue was only checked for graphics,** and it now does compute too, which Vulkan doesn't promise of every queue. It's checked for both now.
- **The arm didn't draw at first.** A model's name was only kept if a file loaded, and a moving mesh has no file.
- **It flared white beside a fire.** Moving light was added on the CPU, and the shader adds it too.
- **A place on a skeleton failed the check that a place exists,** as it's only known once it's posed. Those are checked when the level runs, and say so on the console.

Limits, as they stand:

- **A moving mesh draws its finest level.** Its corners are all moved anyway, so a coarser level would only save drawing.
- **A moving mesh has no collision of its own.** It has the box its entity has, as alias models do.
- **A clip's clock is the map's,** so what a client shows is as good as its clock is. Bad networks haven't been tried.

### Item 11: morph targets, and blending clips

Built as designed, with one change: scripts can set four targets on an entity at once, as that's what goes over the network, and a clip can move any of them. `engine.set_morph` says when there's no room.

Tried in the game with the Blender arm's shape key, driven by a script: it swells at the middle and thins again, lit and shadowed as before.

**Clips blend now.** A client blends over a fifth of a second, which is only for looks, so the server's poses don't, and what's held to a joint goes by the server. Measured in the game, with the arm side on and the clip changing at full bend, the tip came back in steps of at most 8 pixels. With the blend off, the same change was one jump of 21.5. The first try at measuring this proved nothing, as the clip had already swung upright by the time it changed.

- **The test for order had passed with nothing on the screen.** Its mesh had no place, so it was squashed to a point. With that fixed, it fails if morphs come after joints, and passes as they are.
- **A mesh with morphs and no joints** has a joint that moves nothing, or corners would read past what's there.

Limits, as they stand:

- **Only position and normal are morphed.** glTF asks a client for tangents too, and the engine works those out from screen derivatives, so it has none to change. UV and colour targets aren't read.
- **Static copies don't morph.** They show the mesh as it was made.

### Item 12: the fixture's models stay as they are

The rule was that alias models move if pictures of the fixture come out the same and it's no slower. Neither holds, so they stay on their own path.

**It's slower.** The numbers first given here were wall-clock times too noisy to mean anything, and the reason given for them was wrong. It said the cost was built in, and most of it was a poor vertex format. [What meshes left undone](../research/mesh-limits.md) has what was measured with a clock on the GPU, and what was done. For 280 copies of 900 corners it's now 0.114 ms against alias's 0.061. Alias blends two poses as it reads them, which is less work than skinning, so it stays ahead, but by a margin that's small next to what pixels cost.

**The pictures wouldn't be the same either.** How the two answer the same light came out closer than expected, within a few percent over most of the range. But alias models are lit once per corner by Quake's rule, from one side, with fullbright pixels in their skins, and a mesh is lit per pixel. Keeping the look means keeping alias's own fragment shader as a special case, and then only the vertex stage is shared, which is the part that's slower.

So there are two ways of drawing models: one for the fixture's, and one for everything new. **This goes against "one way of drawing", so it's for the human to say if that's wrong.** The fixture's models are the only ones on the old path, and nothing new should use it.

The outline hull was left for this item to settle. As alias stays, moving meshes have one of their own.

