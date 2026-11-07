# Effects

Research for items 78 to 97, and 16, of the [work list](work-list.md), 2026-11-07. Design comes next, in [proposal 0146](../proposals/0146-effects-that-reach.md). The [effects note](effects-water-shaders.md) has what the human asked for: spell effects on the scale of an online role-playing game, rocket splashes, fog and dust with turbulence, sparks, and general player effects. What an author, or Claude, can make matters most.

## What we have

- **Effects are files,** with a stack of settings for each emitter. Kinds are soft sprites, ribbons, beams, flat marks and lights.
- **The CPU** runs up to 8192 particles, and **the GPU** works out `gpu = true` emitters from a number and the time, with nothing kept.
- **Every particle's the same soft dot.** There's no picture at all, which is what item 78 says. Nothing like a spell can be made from that.
- **There are two systems.** Quake's particles are in `particles.rs`, with seven motions of their own, and effects are in `effect_system.rs`.
- **What draws them:** one buffer, with what covers first and then what adds, in no order, last in the scene's pass.
- **Item 9, attach points,** was done at stage 7, and an effect can already follow one with `attach` and `attach_to`, so it's only checked here.

## Pictures and drawing (78 to 85)

- **One blend for everything.** With premultiplied alpha, [one blend state does both: alpha 1 covers and alpha 0 adds](https://github.com/dtrebilco/PreMulAlpha), and [Wicked Engine had used separate targets for the two, which it calls wasteful](https://wickedengine.net/2017/10/which-blend-state-for-me). So what covers and what adds can be sorted together into one draw, if pictures are in one place.
- **Soft edges** [read the scene's depth, and fade by how near a particle is to it](https://developer.download.nvidia.com/whitepapers/2007/SDK10/SoftParticles_hi.pdf). [It's not valid to read what's bound and being written](https://gitlab.com/OpenMW/openmw/-/merge_requests/980), which OpenMW got round by copying depth.
- **Flipbooks** are a sheet of frames, and reading between two frames is what stops them stepping. Rotation and stretching along movement are how every engine does sparks and rain, and are cheap sums in the vertex shader.

**For us:**

- **Pictures go in one array,** as land's do, so a particle says which layer and which cell, and all of them are one draw.
- **Everything sorts, far to near, in one list,** with one blend. Item 85 comes nearly free.
- **Soft edges need no copy.** Particles go in a pass of their own after the scene, with no depth attached. They read the scene's depth as a picture, and do their own test, as the fade is that test.
- **Lit particles** read the probes on the CPU, as a colour for each one, as moving models do. It's a sum for each particle, and it's no work on the GPU.
- **Shaders reach particles** with a hook, `shader_particle`, and a graph output. A shader needs its own pipeline, so what has one is drawn as a group in the same list, in order by group.

## What moves them (86 to 90)

- **Collision on the CPU** is a line trace for each particle that asks for it, which we have, and the budget's the limit.
- **Compute particles:** the usual way has [a dead list, two alive lists swapped each frame, an atomic counter to spawn, and indirect drawing](https://wickedengine.net/2017/11/gpu-based-particle-simulation), and [Bevy's has the same shape](https://medium.com/@Sou1gh0st/gpu-particle-research-bevy-hanabi-part-1-9797d3f08535). That's there to keep a compact list. A ring needs none of it: when what's spawned has a life, the oldest slot is the next free one, and a dead slot is drawn as nothing. It costs some empty draws, and it's a fraction of the code.
- **Forces:** attractors, vortices and wind are a few lines each, and work the same on the CPU and in compute.

**For us:** `gpu = sim` is a third way of running an emitter, a ring of slots in a buffer, stepped by compute, with a hook of its own for what a field shader could do. It can read a field, so that particles can follow what's been worked out. Each force is written twice, in Rust and WGSL, and tests hold them to the same answer. Timeline events and `on_hit` go with `on_death`, as what an emitter does when something happens.

## Control and limits (91 to 93)

- Niagara's scalability is the model: [significance by distance or age, culling what's least significant, a limit shared by a type, culling past a distance, and a global budget](https://dev.epicgames.com/documentation/en-us/unreal-engine/python-api/class/NiagaraSystemScalabilitySettings). [Spawn rate scales too](https://docs.unrealengine.com/4.27/en-US/PythonAPI/class/NiagaraSystemScalabilitySettings.html).

**For us:** a budget of particles, a rate that falls with distance past what an emitter says, and what's far culled first. An effect that's started gets a handle, and scripts can stop it, with a fade, and give one instance its own colour, size and strength.

## Making them (94 to 97)

- Files reload when they change, while the game runs.
- An **Effects** tab in the editor uses the node view for how emitters start each other, with a preview in the level.
- **Quake's particles** become emitters, with their ramps as stepped colours, so there's one way of doing it, and pictures say whether the look held.
- **A library** to start from needs pictures. They're made by a script here, as a puff, a spark, a ring and a flipbook of smoke.

## Sources

Marked for what they are: NVIDIA's paper, Epic's docs and OpenMW's merge request are first hand. Wicked Engine's and Bevy's write-ups are what an author reports of their own engine.
