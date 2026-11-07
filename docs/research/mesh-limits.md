# What meshes left undone

Research from 2026-11-07, for what [proposal 0140](../proposals/0140-meshes-at-run-time.md) listed as limits. The human's view was that a list of known limits reads as work half done, and that most of them were research targets. So each is taken here as a target, with what's known and what it means. Design comes next, and nothing here is approved to build.

Sources are marked for what they are. Nearly all are forum threads, engine documents and project issues, not measurements. Numbers here that say "measured" were measured on this machine, a GTX 1070.

## Why moving meshes cost more

Item 12 said a moving mesh cost 1.7 times an alias model, and called that built in. Asked why, and how to reduce it, the first answer was that nobody had looked.

### The first numbers weren't good enough

They were wall-clock times of `capture` over an empty frame. Taken again with stages switched off, the alias path ranged from 0.14 to 2.09 ms with its code unchanged, so nothing could be concluded from them. Interleaving the runs and taking medians helped a little, but the spread was still as big as the difference.

So the renderer has a GPU clock now: `Renderer::time_gpu`, and `gpu_times` on the console. It stamps each end of a part of the frame on the GPU, and costs nothing when it's off. `speed_of_the_two_paths` in `crates/engine_render/tests/gpu.rs` is ignored by default, and prints the comparison when it's asked for.

### What it showed

280 copies of 900 corners, which is within the frame's room:

| | ms |
|---|---|
| Alias, drawn | 0.061 |
| Moving mesh, drawn | 0.044 |
| Moving mesh, corners worked out | 0.219 |

**Drawing was never the slow part.** A moving mesh draws faster than an alias model. All of the difference was the compute pass.

### A wrong guess, and the real cause

The pass ran one dispatch for each copy, 15 workgroups each, which can't fill a GPU. That was the first guess. Doing every copy in one dispatch didn't help, at 0.248 ms against 0.219.

The real cause was in the vertex format. A moved corner was a whole world vertex of 40 bytes, of which 16 change. Each thread copied the other 24 along, word by word, at a stride of 40 bytes, which GPUs do badly. Taking those words out halved the pass by itself.

| Step | Corners worked out, ms |
|---|---|
| A dispatch for each copy, 40-byte corners | 0.219 |
| One dispatch | 0.248 |
| Nothing copied along | 0.115 |
| 16 bytes at a stride of 16 | 0.092 |
| Place and normal apart from UVs, read and written whole | 0.083 |
| Joints with no weight skipped | 0.072 |

**One dispatch does pay, once memory stops hiding it.** With the rest done, a dispatch for each copy reads 0.116 against 0.085. So both stay.

A mesh's corners are two streams now, 16 bytes of place and normal, and 8 of UV. That's for meshes placed as copies too, so they read 24 bytes a corner and not 40.

**In all, 0.263 ms became 0.114 ms** for 280 copies. Over alias's 0.061 that's 1.9 times, and what's left is the skinning itself. Moving meshes also pay it once, where alias pays again in every pass that draws it.

### What the real game showed

With 24 of the test arm waving in the proving ground at 1280 by 720, overlapping each other:

| | ms |
|---|---|
| Corners worked out | 0.011 |
| Moving meshes, drawn | 0.456 |
| The whole scene | 1.38 |

That's a very different picture from the test, whose meshes are tiny on the screen. **In play the cost is pixels, not corners.** Each arm runs the full material shader on every pixel, including those another arm then covers, as moving meshes skip the depth pass. That one limit is a third of this scene.

## The depth pass

- **Now:** the world's solid faces go through a depth pass, and copies and moving meshes don't. Measured above, that's the biggest single cost found here.
- **What's known:** the pass costs by geometry, and what it saves goes by overdraw times the cost of a pixel, so it [suits what's overdrawn a lot](https://forums.unrealengine.com/t/376644) and [wants measuring](https://gamedev.net/forums/topic/698413-dx11-what-is-the-best-way-to-order-rendering-of-a-scene). [One write-up](https://cprimozic.net/blog/threejs-depth-pre-pass-optimization/) found the gain was all in heavy pixel shaders. What's cut out by alpha [has to be left out or masked](https://jysandy.github.io/posts/mesh-shaders).
- **For us:** our pixels are heavy and these meshes are cheap to draw twice, more so now a depth pass reads only the 16-byte stream. Solid materials go in, and masked ones stay out, by the rule `depth_first` already has for faces.

## Level of detail for what moves

- **Now:** a moving mesh draws its finest level, so a far crowd pays full price.
- **What's known:** engines [drop update rates by distance, staggered across frames](https://github.com/tariq-98/aldermoor-rs/issues/215), freeze what's off screen, and [turn off bone chains per level](https://github.com/pavlobielousov/WoWee/issues/20). One project's numbers went [from 67 bones to 14](https://github.com/pavlobielousov/WoWee/issues/20) by moving weights to parents. [Low rates look choppy](https://moonjump.com/game-dev-mechanics-skeletal-animation-skinning-how-it-works/) without interpolation, and [skipping skinned work off screen](https://discourse.threejs.org/t/optimization-of-large-amounts-100-1000-of-skinned-meshes-cpu-bottlenecks/58196) is the first gain in crowds.
- **For us:** levels are extra triangles on the same corners, so a coarser level saves drawing and nothing else. If corners were sorted, coarsest level's first, a level would use the first so many corners, and the pass could stop there. That's cheap, and saves both. Rates matter for the CPU, which samples each joint of each model each frame. That hasn't been measured, and should be before anything's built.

## Hit volumes

- **Now:** a moving mesh has its entity's box, as an alias model does. A shot that passes through the gap under an arm hits.
- **What's known:** games keep [a simple shape for moving, and per-bone shapes for shots](https://counterstrike.fandom.com/wiki/Hitbox), which [follow the bones after animation](https://github.com/shadow-kernel/Vortex-Engine/issues/212). [A cheap broad test comes first](https://github.com/shadow-kernel/Vortex-Engine/issues/213). Online, a server [keeps a short history of poses to rewind](https://github.com/SoftHamzzi/UE5-EmploymentProj/pull/2), [with a limit on how far back](https://steamcommunity.com/app/1245620/discussions/0/3279192886584619380). These are a wiki, project issues and a forum. One [is a vendor's page](https://www.fab.com/listings/0a897de8-58de-48f6-bc8a-cc343fdb5483?lang=en), and another [a project's plan](https://github.com/MatthewLacerda2/rusty/issues/464), so they show the shape of it and no more.
- **For us:** movement keeps its box. A mesh can name capsules on joints, and `traceline` tests them after the box says it's near. The server already works out poses for what's held, with the same shared code, so it needs no new way of doing that. The engine has no rewinding of any kind, for boxes either, so that's its own question, and it's for all entities and not for meshes.

## Static copies and morphs

- **Now:** a copy shows a mesh as made, whatever its morphs say.
- **For us:** glTF's `weights` on a mesh are its rest weights, and a mesh with rest weights should look that way everywhere. They can be applied when the level compiles, for the corners a copy draws. No research is needed past that, and it's a bug more than a limit.

## How many morphs a script sets

- **Now:** four, as that's what goes over the network. A clip moves any of them.
- **What's known:** glTF's floor for a conformant client is [eight morphed attributes](https://github.com/KhronosGroup/glTF/issues/1675), with position and normal counting as two each. That's four targets of both, so four is the floor exactly, but it says nothing of a target that has only positions.
- **For us:** four isn't below the floor, but it's a number chosen by a network message, which is the wrong reason. Only what's set need be sent, so room for eight costs nothing when it isn't used.

## UV and colour targets, and colours

- **Now:** UV targets aren't read. Meshes have no vertex colours at all, so colour targets have nothing to change.
- **What's known:** glTF asks a client for position, normal and tangent, so UVs and colours are extra. [There's a test model for `COLOR_0`](https://www.github.com/cx20/gltf-test/blob/master/tutorialModels/VertexColorTest). What a colour target means [isn't agreed between tools](https://forum.nomadsculpt.com/t/morph-target-colors-arent-additive-but-replacing/6430), and [loaders have failed on them](https://forum.babylonjs.com/t/c4d-animation-gltf-load-error/11384).
- **For us:** now UVs have their own stream, a UV target can be done by moving that stream for the meshes that have them. Vertex colours are a real feature with a real use, such as tinting and blending: they want their own stream and a look in the material shader. **Colour targets are worth leaving out** until tools agree on what they mean, and then it's a sentence in the standard and not a limit.

## Tangents

- **Now:** glTF asks a client for tangent targets, and they aren't read.
- **For us:** this isn't a gap. The engine has no tangents. It works them out from screen derivatives, from position and UV, so they follow any morph with no help. There's nothing to read them into, and they'd do nothing. The limit should go from the list.

## See-through copies

- **Now:** see-through copies and moving meshes aren't sorted, among themselves or with the world's.
- **What's known:** the usual way is [to sort instances on the CPU, back to front, and batch those next to each other that match](https://www.gamedev.net/forums/topic/652835-dx11-instancing-and-transparency-affording-draw-order/). [Instanced draws lose their order](https://discussions.unity.com/t/sorting-order-layer-with-graphics-drawmesh-instanced/887511) otherwise. [Order-independent transparency](https://en.wikipedia.org/wiki/Order-independent_transparency) needs no sort, at a cost in memory or accuracy, and [Unreal has it as a project setting](https://forums.unrealengine.com/t/translucency-sorting-within-one-material/2346871). Sorting by middle [fails for what intersects](https://github.com/shimakaze09/Engine/issues/985).
- **For us:** sorting by middle is enough for props. What's see-through is a small share of copies, so the batches it breaks up don't matter. Sorting them against the world's faces is the harder half, and it depends on how those draw now. That has to be read first.

## A copy's triangles on the CPU

- **Now:** each copy's triangles stay in `Map::faces`, at about 150 bytes a triangle by reckoning, for the bake and for leaves. Nobody has measured it.
- **For us:** the bake is at compile time, so the game may not need them at all. It's a measurement first: what a level with many copies holds, and what's read at run time.

## What's held to what's held

- **Now:** something held to something held can lag a tick, by the order they're in.
- **What's known:** a scene graph [updates parents before children](https://docs.vulkan.org/tutorial/latest/Advanced_glTF/Scene_Graph_Hierarchy/02_engine_expansion.html), and [what reads a world place should read it after that](https://github.com/Yellow-Dog-Man/Resonite-Issues/issues/1492). [Dirty flags](https://gameprogrammingpatterns.com/dirty-flag.html) save the work where most things don't move, and [other engines do the same](https://doc.babylonjs.com/lite/architecture/11-scene-hierarchy-parenting).
- **For us:** `follow_attachments` goes in the order of the entity list. Few things are held, so a sort each tick costs nothing, and dirty flags aren't worth having yet. A loop of things holding each other has to break rather than hang.

## Places and effects on what turns

- **Now:** a place on a brush model follows the model's offset and not its turn, in `point_on`, and an effect on a model does the same. Brush models don't turn when drawn yet, so nothing shows it.
- **For us:** the turn should go in both now, while it's small, as doors that swing are coming at stage 16. A place on a skeleton turns already, by another path, so this makes the two agree.

## Clips on a bad network

- **Now:** a client is told a clip and when it began, and works out the time. Bad networks haven't been tried.
- **What's known:** sending [which clip began when](https://www.gamedev.net/forums/topic/601271-coordinated-animations-in-fps-networked-game/) and letting clients work out the time is the usual way, as [ours is](https://www.gamedev.net/forums/topic/708635-interpolating-between-fsm-states/). [Late joiners need putting right](https://forums.unrealengine.com/t/persistent-animation-replication-on-multiplayer/1638908), which state sent in full does for free. [Clocks drift and need setting again](https://doc.photonengine.com/fusion/current/technical-samples/animations), and [one guide](https://mocaponline.com/blogs/mocap-news/multiplayer-animation-sync-guide) and [Unreal's documents](https://dev.epicgames.com/documentation/en-us/unreal-engine/networked-physics-overview) say the same of buffering. Those are forums, a vendor's blog and engine documents.
- **For us:** it just wants trying, with `net_fake_loss` at 2%, and a client joining part-way through a clip. If it holds, that's the end of it.

## What this means for the design

- **The depth pass** comes first. It's the largest cost measured.
- **Fixes with no open question:** static copies with rest weights, parents before children, the turn in places and effects, eight morphs.
- **Measure, then decide:** a copy's triangles on the CPU, and what posing costs the CPU.
- **Building:** a coarse level moving fewer corners, hit volumes on joints, UV targets, vertex colours, sorting what's see-through.
- **Trying:** clips on a bad network.
- **Not limits after all:** tangents, as the engine has none to change. Colour targets, until tools agree.
- **Bigger than meshes:** rewinding for shots online covers every entity, so it goes to the network's own list if it's wanted.
- **Item 12:** alias models are still cheaper, at about half, and the pictures would still differ. But the gap is now the real work of skinning, and at the scale of real scenes it's a tenth of what overdraw costs, so speed is no longer much of a reason. The look is. **They haven't moved:** there are still two ways of drawing models. Moving them means carrying alias's lighting over as a fragment shader of its own on the mesh path, which hasn't been built or tried.
