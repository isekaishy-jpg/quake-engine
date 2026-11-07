# Proposal 0146: effects that reach

Status: proposed, 2026-11-07, the [work list](../research/work-list.md), items 78 to 97 and 16, from [the research](../research/effects.md). Item 9 was done at stage 7.

## Design

### Drawing (78 to 85)

- An emitter can name `image`, a picture or a sheet, with `flipbook = columns rows`, and `shader`.
- The pictures of a level's effects go in one array, of the largest size up to 512. A particle says its layer, its frame, and how far to the next.
- Particles draw in a **pass of their own** after the scene, with no depth attached. They read the scene's depth as a picture and test against it themselves. `soft` says over how many units one fades into what's behind it.
- **One list, one blend.** Premultiplied, so that `add` is alpha 0, sorted far to near. Shaders make groups, in order of their nearest.
- `rotation` and `spin` turn a sprite. `stretch` draws it out along how it moves.
- `lit = true` multiplies a particle by the probes where it is, and moving lights.
- Particles behind see-through surfaces draw ahead of them: the scene splits there when it has both.

### What moves them (86 to 90)

- `collide = bounce`, `stick` or `die`, with `bounce` for how much it keeps. What's traced is the world, land and all.
- `on_hit` starts an emitter, as `on_death` does. A `timeline` starts them at set times.
- `force` blocks: `attract`, `vortex`, `wind`.
- `shape` gains `ring`, `cone`, `disc`, `line` and `mesh`, the last on the surface of a named mesh.
- `gpu = sim` runs an emitter in compute: a ring of slots, stepped each frame, with the forces, a `shader_motion` hook, and a field to read.

### Control (91 to 93)

- `engine.effect` gives back a handle. `engine.effect_stop(handle, seconds)` stops new ones and fades what's there.
- `engine.effect(name, origin, follow, { color, size, strength })` sets one instance.
- `r_effect_budget` is a number of particles. Past it, what's furthest makes less. `far` on an emitter is where it thins out and stops.

### Making them (94 to 97)

- Effect files reload when they change.
- An **Effects** tab: emitters in the node view, joined by what starts what, each one's settings beside it, and a preview at the camera.
- Quake's particles become effects, made in code from the palette, and `particles.rs` goes.
- `effects/` gets a library: spells, explosions, weather, sparks, with pictures made by a script in `tools/`.

### Mesh particles (16)

- `kind = mesh` with `mesh = name` draws a level's moving mesh for each particle, spun as it goes, on the path moving meshes have.

### Other approaches

- **Selected approach:** the above, in that order: drawing, then what moves them, control, making, and meshes.
- **Tradeoffs:**
  - One array means one picture size, so a small picture costs as much as a large one.
  - Lit particles are lit by what's near their middle, not per pixel.
  - A ring draws slots that are dead, as nothing.
  - Each force is written in Rust and in WGSL.
- **Rejected:**
  - A batch for each picture: order goes, or draws go up.
  - Copying depth: a pass of their own needs none.
  - Dead and alive lists: for what lives a set time, a ring does the same with a fraction of the code.
  - A graph for what moves them: the stack and forces cover what's asked. The graph joins emitters, and shaders have their own.
- **Acceptance criteria:**
  - A spell, an explosion, weather and sparks can be made, from files, and look like what they are.
  - Particles fade where they meet a wall, and sort right.
  - Quake's particles look as they did, with no second system.
  - 100,000 compute particles run at a good rate, and what they cost is measured.
  - A script can start, tint and stop an effect.

## Review

Filled in as each part lands.
