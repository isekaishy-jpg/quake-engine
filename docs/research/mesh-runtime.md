# Meshes at run time

Research for items 7 to 12 of the [work list](work-list.md), 2026-11-06: instancing, level of detail, attach points, skeletal animation, morph targets, and whether the fixture's models move over. [The mesh note](mesh-support.md) set the order. This goes through design next, and nothing here is approved to build.

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
- **Crossfading** by a dither pattern is [described by a low authority source](https://bugnet.io/blog/fix-unreal-skeletal-mesh-lod-popping-artifacts), which warns the cost adds up in crowds. Its timing isn't verified. Nothing found settles if it's needed, so that wants trying on a real mesh.

## Skeletons and morphs

- **glTF** has [a skin that names its joints and inverse bind matrices](https://gltf-tutorial.readthedocs.io/en/latest/gltfTutorial_019_SimpleSkin/). A joint's matrix is [its world transform times its inverse bind matrix](https://daslang.io/doc/_sources/stdlib/generated/gltf.rst.txt), which is what would go to the GPU.
- **Morph targets** are [weights on a mesh, which can be animated](https://www.khronos.org/developers/linkto/gltf-2.0-reference-guide). [Sparse accessors](https://registry.khronos.org:443/glTF/specs/2.0/glTF-2.0.pdf) store only what differs, which suits them, so a compiled mesh can keep only the corners that move.
- **The `gltf` crate** is in use already, for import.

## Gaps

The search found nothing on linear blend against dual quaternion skinning, or on how to lay joints out on the GPU. Both need looking at before item 10 is designed. Attach points weren't searched: they're joints or named places, and depend on the design of the rest.

## What this means for the design

- One new thing, a run time mesh, and one way to draw it. Instances are the case of many, a skeleton and morphs change corners, and an attach point is a place in it.
- Baked placements stay right for what wants baked light. A placement chooses, as `light = probe` already does.
- Item 12 follows from this: alias models are a mesh with morph-like poses, so moving them is a question of whether that one path can draw them as they are.
