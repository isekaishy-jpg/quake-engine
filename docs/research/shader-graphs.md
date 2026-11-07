# Shader graphs and compute

Research for items 13 and 14 of the [work list](work-list.md), 2026-11-07. Design comes next, in [proposal 0145](../proposals/0145-shader-graphs-and-compute.md). The [effects note](effects-water-shaders.md) has what the human asked for: node graphs plus surface code, whole shader files for rare cases, one graph editor shared with particles, and a reach as far as compute.

## What we have

- **A shader is WGSL in `shaders/`,** with hooks: `shader_vertex`, `shader_uv`, `shader_surface`, `shader_whole` and `shader_post`. What it leaves out stays as it is.
- **Around that it's all there:** compiled ahead of time, a green surface when it's broken, material instances, `check_shaders`, and packing.
- **Compute is there, for the engine's own work:** `cs_deform` and `cs_poses`, on buffers. `vulkane` also has storage images. Nothing a level's author writes runs as compute.
- **The editor is on `egui` 0.37,** pinned.

## Graphs (13)

- **What a graph is, underneath:** the usual way is to [walk back from the output, order the nodes, and write one variable a node](https://github.com/actnwit/RhodoniteTS/blob/4219ddad/src/foundation/materials/core/ShaderGraphResolver.ts). [A smaller engine does the same in so many words](https://github.com/Krilliac/SparkEngine/wiki/Shader-Graph), with a default for an input that's not joined up, though that's only an illustration.
- **What a person wants of one:** [ports, edges, a preview, and a way to see the code it makes](https://docs.unity3d.com/Packages/com.unity.shadergraph@17.4/manual/Node.html).
- **The way out to code:** Godot has [an expression node, with code typed into it and as many ports as it needs](https://docs.godotengine.org/es/stable/classes/class_visualshadernodeexpression.html), which can't declare functions, and [a global one that can](https://docs.godotengine.org/es/stable/classes/class_visualshadernodeglobalexpression.html). So a graph never has to stop at what its nodes can do.
- **The editor's crate:** [`egui-snarl` 0.12](https://docs.rs/egui-snarl/latest/egui_snarl/) is the live one, and it's on `egui` 0.36. Ours is pinned to 0.37, so it doesn't fit. A node view is a canvas, boxes and curves, and it's used for shaders and then particles, so it's worth having as our own.

**For us:** a graph is a file beside the WGSL, under the same name, that compiles to surface code. It then has everything shaders have, with nothing new in the renderer: precompiling, green when broken, instances, packing. A person can look at the code it made, and carry on from there by hand. It has an expression node, so that a graph doesn't run out. Which hook a node lands in follows from the output it goes to, and a node that can't be read there is an error that says so. There's no branching in a graph, so `shader_image` is always read where it's allowed.

## Compute (14)

- **The closest thing to what's asked:** Niagara's simulation stages. They're [compute shaders in all but name, each run over an iteration source, such as every cell of a grid](https://threadreaderapp.com/thread/1574293239447769090).
- **What they've learned:** [what a stage reads is from before it began](https://forums.unrealengine.com/t/smoothed-particle-hydrodynamics-in-niagara/481743?page=2), which is why it takes stages, and [they aren't free](https://dev.to/dinesh_04/how-simulation-stages-work-in-unreal-engine-gpu-emitters-149c), as cost goes with iterations.

**For us:** the same shape, and the small end of it. A compute shader answers one question: what is this cell of a grid now, given what the grid was. That covers what compute's wanted for: ripples, flow, spreading fire or wet, and growth. It can't write out of bounds, can't race with itself, and needs no barriers from its author. It's a hook, like the others, so it's a function of a few lines. A grid is called a **field**. It's a picture to whatever reads it, so a material takes one as a shader image, and a surface shader or a graph needs nothing new to use it.

Particles by compute, item 90, come after this, on the same way of running things. Arbitrary buffers and dispatches from a level aren't needed for any of that, and they'd bring what a hook keeps out. Whole `shader_whole`-style compute can come if something asks for it.

## Sources

Marked for what they are: Unity's and Godot's manuals and Epic's are first hand, forum and blog posts are what someone reported, and the two small engines are illustrations.
