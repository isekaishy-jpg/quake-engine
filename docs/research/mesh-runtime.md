# Meshes at run time

Research for items 7 to 12 of the [work list](work-list.md), 2026-11-06: instancing, level of detail, attach points, skeletal animation, morph targets, and whether the fixture's models move over. [The mesh note](mesh-support.md) set the order. This goes through design next, and nothing here is approved to build. Sources are marked for what they are: only the glTF specification and the Kavan paper are primary, and the rest are engine documents, project notes and forums.

## What we have

- **World meshes are baked.** `place_mesh` turns a placement into triangles of the map, batched by material, with baked light. Each placement costs its triangles again. [Proposal 0109](../proposals/0109-meshes.md) chose that, and said instancing wouldn't save anything at that scale.
- **Nothing moves but alias models.** `EntityDraw` carries a model, two poses and a blend, and `record_models` draws one at a time, each with its own push values.
- **So there's no run time mesh.** All six items need one: a mesh that lives on the GPU once, and is drawn wherever it's asked for. That's the first thing to design, and the items hang off it.

## Instancing

- Draw many copies in one call, with the mesh uploaded once and [a buffer of what differs per copy](https://api.playcanvas.com/engine/classes/MeshInstance.md). That buffer [can hold more than transforms](https://stefangroenewoud.artstation.com/blog/wBAPD/day-39-deep-dive-mesh-instancing), such as colours and material values.
- Plain instanced paths often skip culling: [Unity's documents say so](https://docs.unity3d.com/560/Documentation/ScriptReference/Graphics.DrawMeshInstancedIndirect.html) of theirs. The usual answer is a compute pass that tests each copy and writes the survivors to an indirect draw buffer, so there's no reading back.
- These sources are engine documents and write-ups, not measurements. How many copies a level has should decide if a compute pass is worth it. A level's visibility is by leaf already, so copies can start by being culled on the CPU the way faces are.

## Level of detail

- **Choosing by size on screen** beats distance, which [ignores how much of the screen something covers](https://ktstephano.github.io/rendering/stratusgfx/lod). [One engine's documents](https://pub.dev/documentation/flutter_scene/0.20.0/scene/LodComponent-class.html) take the projected share of the view's height, which also makes it independent of resolution and aware of the field of view.
- **Hysteresis** stops flicker at a boundary: [Unreal switches back at the size plus a margin](https://dev.epicgames.com/documentation/unreal-engine/API/Runtime/Engine/FSkeletalMeshObject/FSkelMeshObjectLODInfo/LODHysteresis?application_version=5.5).
- **Making levels:** [one engine feeds each mesh through `meshopt_simplify`](https://ktstephano.github.io/rendering/stratusgfx/lod), and each result into the next. A new dependency needs its own proposal.
- **Crossfading:** the common way is a dither mask, which stays on the opaque path and so avoids [the depth sorting and occlusion trouble of real transparency](https://github.com/Construkted-Reality/3DT-Local-viewer/blob/main/docs/research/cross-fade-lod-transitions.md). The old and new levels use complementary patterns, so each pixel is covered by exactly one of them. That note aims for about half a second, and suggests capping how many fades run at once. [Both levels draw during a fade](https://github.com/godotengine/godot-proposals/issues/5240), which costs overdraw, and [dithering can show as noise](https://pulsegeek.com/articles/lod-setup-best-practices-detail-where-it-matters/). These are project notes and proposals, not measurements. Tried here, it wasn't worth keeping: see [proposal 0140](../proposals/0140-meshes-at-run-time.md).

## Skeletons and morphs

### What glTF asks for

From [the specification](https://registry.khronos.org/glTF/specs/2.0/glTF-2.0.html):

- Skinning is linear: a vertex's matrix is a weighted linear sum of the joint matrices, with a joint's inverse bind matrix applied before its node's transform.
- A set of `JOINTS_n` and `WEIGHTS_n` has up to four joints a vertex. There can be more sets, and a client may read only one, though that can change the animation. Weights aren't negative and sum to 1.
- The transform of a skinned mesh's own node is ignored: only the joints apply.
- Morph targets are differences, applied before skinning or any other matrix. A target can carry position, normal, tangent, UVs and colours, and a client should support the first three. Every primitive of a mesh has the same targets in the same order.
- An animation's channel aims at a node's translation, rotation, scale or morph weights, with `LINEAR`, `STEP` or `CUBICSPLINE` between keys. Rotations should use slerp. Appendix C, on the details, couldn't be read here, so what it says of shortest paths and normalising isn't confirmed. Taking the shortest way and normalising is safe either way.

### Linear or dual quaternion skinning

[Kavan and others, 2007](https://users.cs.utah.edu/~ladislav/kavan07skinning/kavan07skinning.pdf), read directly:

- On a GeForce 6600 GT, the chart reads about 0.32 ms for linear blending and 0.46 ms for dual quaternions, on one model. That's a 2007 card and the numbers come from text pulled out of a chart, so it shows the size of the difference and no more. The paper calls its method "slightly slower".
- Dual quaternions need 8 floats a transform, against 12 for a matrix's useful rows.
- They're limited to rigid transforms, so they suit no model whose parts scale or shear. glTF joints can scale.

Linear blending has [the candy wrapper fault, losing volume at a twist](https://www.researchgate.net/figure/A-demonstration-of-the-artefacts-of-linear-blend-skinning-LBS-and-Dual-quaternion_fig4_318590047). Dual quaternions fix that and [bulge at a bending joint](https://www.researchgate.net/figure/A-demonstration-of-the-artefacts-of-linear-blend-skinning-LBS-and-Dual-quaternion_fig4_318590047) instead, and [Disney had to extend them for production](https://disneyanimation.com/publications/enhanced-dual-quaternion-skinning-for-production-use/). So neither is free of faults.

**Linear is the answer.** It's what glTF defines, so a model looks here as it looked in the tool it came from. It takes scale. A twist can be fixed by a twist bone in the model. Dual quaternions would be a second way of doing the same thing.

### Where skinning happens

- **In the vertex shader:** needs a joint palette reached per draw. Nothing extra is kept, and the work repeats in each pass that draws the mesh.
- **In a compute pass first:** [deforms each visible instance into scratch space](https://github.com/DaFrancc/Assisi/issues/8), and everything after treats it as a static mesh. [The result is reused by other passes](https://forums.unrealengine.com/t/gpu-skin-caches-efficiency/440512), such as shadows. It costs scratch memory for what's on screen. Those are a project's issue and a forum answer.
- **Bounds** have to come from [the current pose, not the bind pose](https://github.com/DaFrancc/Assisi/issues/8), or culling pops.

**What we have matters here.** The renderer has no compute pipeline and no storage buffer today: its GPU particles run in a vertex shader, and it asks for Vulkan 1.1. A compute pass would be new to the renderer, though `vulkane` has compute pipelines, dispatch and storage buffers, so it wouldn't need new work there. Moving light shadows are off the list, so there's no second pass of skinned meshes to save work for yet, past the outline hull. The proposal has to weigh that.

### Laying out joints

Nothing found gives a standard layout, and sources differ: [a texture](https://community.khronos.org/t/skinning-on-the-gpu-vs-the-cpu/73169) or uniform buffers. The rest is ours to choose. One storage buffer of every palette this frame, with each instance saying where its own begins, needs no binding per draw. A joint is three rows of `vec4`, as a matrix's last row is always the same, which is 48 bytes. WGSL's layout table couldn't be read here, and `vec4` rows need nothing from it.

### Morph targets

- **Sparse:** only keep what moves. [Stride measured](https://github.com/stride3d/stride/pull/3569) a character at 90 MB against 1 GB dense, and 22 ms against 166 ms on 16 cylinder meshes, on an RX 6800, with 61,000 vertices and 600 targets. That's one project's bench, at far more targets than we'd use.
- **Gather:** entries sorted by vertex, so that one thread reads its own and writes once, with no atomics. Scatter only touches active targets, but needs atomics and a second pass. Stride chose gather, with no direct timing between them.
- **Half floats** for differences, as they're small: 24 bytes an entry there.
- **Vertex shader or compute:** Stride's vertex shader version was 3 to 17 times slower, as it repeats the loop in each pass and instance. That's at 600 targets.
- **Few active targets:** [cost goes with active targets and the vertices they touch](https://mocaponline.com/blogs/mocap-news/blend-shapes-morph-targets-guide), and [zero weights needn't reach the GPU](https://github.com/dvoyni/cog/issues/92).

### Attach points

- [Unreal's sockets](https://dev.epicgames.com/documentation/en-us/unreal-engine/skeletal-mesh-sockets-in-unreal-engine) are named places in the skeleton, with an offset from their bone. [The same name on different characters](https://nhance-school.com/tools/glossary/socket) lets one item fit them all.
- glTF has no standard for them. [One project's proposal](https://github.com/r6e/paksmith/issues/578) writes each as a node named for it, under its joint. That's one source, and it's the natural way: an empty in Blender is such a node, so an author needs no new tool.
- **A trap:** [a weapon right in the editor and off at run time](https://forums.unrealengine.com/t/weapon-socket-offset-on-retargeted-characters-ue-5-6-correct-in-editor-offset-at-runtime/2669313), suspected to be reading the socket before the pose is worked out. So a place is read after the pose, each frame.

## What this means for the design

- One new thing, a run time mesh, and one way to draw it. Instances are the case of many, a skeleton and morphs change corners, and an attach point is a place in it.
- Skinning is linear, as glTF defines it, with four joints a vertex and joints in one storage buffer.
- Attach points are named nodes in the file, read after the pose.
- The proposal chose compute. A moving mesh is plain vertices after that, so there's no second way of drawing.
- Baked placements stay right for what wants baked light. A placement chooses, as `light = probe` already does.
- Item 12 follows from this: alias models are a mesh with morph-like poses, so moving them is a question of whether that one path can draw them as they are.
