# Proposal 0107: shaders

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

[The note](../research/effects-water-shaders.md) sets the direction: add shaders at leisure, in a common container, with surface code and whole files for rare cases. They reach surfaces, vertices, particles, post effects and compute. Material instances. Branching by default, with compiled variants for speed. A broken shader shows green. There's one way of drawing.

- Materials could only choose from fixed options, and the older path for plain pictures sat beside them with its own pipelines.
- [Godot's shader types](https://docs.godotengine.org/de/4.4/tutorials/shaders/introduction_to_shaders.html) show what surface code can be: short functions the engine calls, with the rest done for you.
- The note assumed compiling at each launch was a cost, so it was measured. The whole world shader takes 5 ms in `naga`, and every pipeline 11 ms, of 294 ms for the renderer. Precompiling WGSL would save nothing anyone could notice, so it isn't done.
- That 11 ms is with this driver's own disk cache warm. A driver with a cold cache is slower, and [a saved Vulkan pipeline cache needs its own checks](https://about.roblox.com/newsroom/2021/02/creating-a-robust-pipeline-cache-with-vulkan), since drivers don't always reject a stale one.

## Design

- **Shaders are files.** `shaders/<name>.wgsl` sits in a container as textures do, and goes in a pack.
- **Surface code.** A file replaces any of four hooks: `shader_vertex`, `shader_uv`, `shader_surface` and `shader_whole`. `surface_source` adds a stand-in for each it leaves out, so what isn't replaced goes on working. `shader_whole` is the whole-file case. WGSL doesn't care about order, so code is just appended to the module.
- **The final pass** has one hook of its own, `shader_post`, set by `post_shader` on `worldspawn`.
- **Instances.** A material with `parent` takes everything from it that it doesn't say itself, with a limit on depth so a circle is an error.
- **Numbers and pictures.** Eight numbers and two more picture slots, in a material's parameters.
- **Variants.** Each shader in use has its pipelines, made as a map loads, and found again by its code. Most surfaces stay on the standard one, which branches, and the `PLAIN` variant goes on serving what has nothing unusual about it.
- **Depth.** A shader that moves corners or discards can't follow the depth pass, so it's drawn without it. That's read from its code.
- **One way of drawing.** Every surface now goes through the material path. The fixed `solid`, `cutout`, `liquid` and `sky` pipelines are gone, and pictures with no material get `classic`, `classic_cutout`, `classic_liquid` and `classic_sky`, which are shaders like any other. A container can replace them by name.
- **What stays in the binary.** `world.wgsl` and the rest of the core aren't files in a container, as a broken one couldn't show green, or anything else. `shader_whole` is the way round them.
- **Broken shaders.** Missing or not compiling shows green and goes to the console. Bad post code is left out and noted. `check_shaders` in `engine-tool` finds them ahead of time.
- **A crate for shaders.** Tools may not know the renderer, so every WGSL file and how surface code joins them moved to `engine_shaders`, which needs no GPU and depends on no other crate. `naga` is its backend, and the crate rules say so.
- **Pipelines kept.** A pipeline cache is kept in the user's folder. Its file starts with a hash of the GPU, driver and engine, and a hash of the rest, so anything stale or cut short is ignored. `vulkane` needed a small patch to pass a cache to pipelines.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** surface code is WGSL, which is a real language to learn. Graphs will sit on top of it. Text is what Claude writes best, which was the first concern.
- **Acceptance criteria:** a shader can change a surface, its vertices, or all of it, and a level's final pass. Instances work. Broken or missing shaders show green and say why. Old levels look as they did, and frame rate doesn't fall.
- **Future extension:** the graph editor and compute are item 13, with particles, which share them. Those use the same hooks.

## Review

- The first version lost every shader on a change of samples: `set_antialiasing` remade the pipelines and a map's textures still pointed at the old places. Two existing tests caught it, and `shaders_survive_a_change_of_samples` now covers it directly.
- The original E1M1 and Vesper Deep were compared with captures from before. Vesper differs by 0.02 to 0.04 of a byte on average, and E1M1 by 0.18 in the room. Frame rate is the same in both, at about 205 to 211 FPS at 1280 by 720.
