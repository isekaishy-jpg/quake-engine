# Effects, water and shaders

Status: a direction, not a design. This hasn't been through the workflow and nothing here is approved to build. It goes through research, design and proposal later, as one piece of work. Other decisions still to come will change it, so doing it now would be wasted.

The human set this direction on 2026-11-05 and 2026-11-06.

## Why it came up

- Particles are still a port of Quake's: a position and a colour, in seven built-in motions. A level can't define its own. The Fall's mist had to be fixed quads (proposal 0100).
- Water is a box with a flat top, and the 8 nearest bodies get its effects each frame.
- Materials can only choose from fixed options, and every shader is compiled from source at each launch.

## Direction

### Particles

- What an author, or Claude, can make matters most, even if advanced things are extra work.
- A mix of graph and modular, with scripts attaching where needed, and shaders.
- Simulation on the CPU and the GPU, chosen by need.
- A particle can be a sprite, ribbon, beam, mesh, light or decal, among others.
- A visual graph editor early, not after the runtime.
- Attach points for maps and prefabs, which may open the way for models.
- It should reach spell effects on the scale of an online role-playing game, rocket splashes, fog and dust with turbulence, sparks and general player effects.

### Water

- A standard general system, for all types of water.
- Screen-space reflections and the usual features, meshes and normals, depth fog, underwater effects and particles, and particle waterfalls.
- Much of this falls under shaders.
- Higher quality water and lava must still be able to take a murky look like Quake's when wanted.

### Shaders

- Add them at leisure. They're precompiled and cached.
- One common container, used everywhere as textures are, so sizes don't balloon.
- Node graphs plus surface code, with whole shader files for rare cases.
- They reach surfaces, vertices, particles, post effects and compute.
- One graph editor, shared with particles.
- Material instances.
- Branching by default, with compiled variants where something needs the speed.
- The engine's own shaders live in the container too, with a default set out of the box, as prefabs are.
- A missing or broken shader shows a green texture or similar.

### Shadows

These now have their own note: [Shadow rendering](shadow-rendering.md).

### The Quake fixture

- The older drawing path is retired, not kept alongside. There's one way of drawing.
- If the fixture needs its look kept, it gets a shader on the new system.
- Moving its levels, models and sprites across is its own part of the work.

## Notes for the research

These came up on the way, and are starting points to check again then.

- [Niagara](https://dev.epicgames.com/documentation/en-us/unreal-engine/key-concepts-in-niagara-effects-for-unreal-engine) is the closest match for graph and modular together: a stack of modules, each a graph underneath.
- [Niagara's events only work on the CPU](https://dev.epicgames.com/documentation/unreal-engine/events-and-event-handlers-in-niagara-effects-for-unreal-engine?lang=en-US), so a choice of CPU or GPU has to say what each can do.
- [Unreal compiles a new material for every combination of static parameters](https://dev.epicgames.com/documentation/en-us/unreal-engine/instanced-materials-in-unreal-engine), and [The Shader Permutation Problem](https://therealmjp.github.io/posts/shader-permutations-part2/) sets out the ways of keeping counts down.
- [Godot's shader types](https://docs.godotengine.org/de/4.4/tutorials/shaders/introduction_to_shaders.html) are a model for surface code that also covers particles.
- [Screen-space reflections only see what's on screen](https://docs.unity3d.com/Packages/com.unity.postprocessing@2.0/manual/Screen-space-Reflections.html), so they need something to fall back on.
- [Portal 2's water](https://www.advances.realtimerendering.com/s2010/Vlachos-Waterflow(SIGGRAPH%202010%20Advanced%20RealTime%20Rendering%20Course).pdf) is the reference for flow maps, and [Single Layer Water](https://dev.epicgames.com/documentation/en-us/unreal-engine/single-layer-water-shading-model-in-unreal-engine) for absorption and refraction from one depth layer.
- [A saved Vulkan pipeline cache needs its own checks](https://about.roblox.com/newsroom/2021/02/creating-a-robust-pipeline-cache-with-vulkan), as drivers don't always reject a stale one.
- [`egui-graph-edit`](https://docs.rs/egui-graph-edit) and [`egui_snarl`](https://docs.rs/egui-snarl/latest/egui_snarl/) are node graph crates for the editor's `egui`. Neither has been checked against its version.
- `vulkane` has compute, a pipeline cache and specialization constants. The engine uses none of them yet.
- The engine has no bones or attach points, and `Effect` is `Copy`, so it can't carry an effect's parameters as it stands.

## Still open

- How an effect's timeline and events work, and what the first modules are.
- How water's look is split between shaders and the water system itself.
