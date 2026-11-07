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

### Drawing (78 to 85)

- **Tried in the game:** smoke from a sheet of frames, with flame and sparks, in a room with a wall behind them. The smoke looks like smoke, and fades into the wall.
- **Particles have a module of their own,** `particle.wgsl`, and sprites stay where they were.
- **No groups by shader.** The proposal had groups in order of their nearest. Sorting everything by distance and drawing each run of one shader is simpler, and it's right, at the cost of a draw for each run.
- **What's behind glass** is found by a line from the eye to each particle, against the bounds of the nearest 64 see-through faces. That's right for what's flat and square to the world, which most glass is. Those go in ahead of what's see-through, in a pass of their own, and the scene's pass carries on after them. It costs nothing when there's no such particle.
- **Lit particles** take the probes' brightness, by a sum on the CPU for each one.
- **Graphs** gained `particle`, with `tinted` and `life`. `vendor/vulkane` gained `premultiplied_blending`, as its other blends are engine extensions.
- **Every test here was seen failing** with its bug put back, fourteen in all.

### What moves them, and control (86 to 89, 91 to 93)

- **Collision** is a line trace for each particle that asks for it, with land, doors and all, through `Client::line_hit`. A mark that `stick`s faces what it hit, so `kind = decal` with `collide = stick` is a mark on any wall.
- **Forces** live in `engine_map`, so compute can be held to the same sums.
- **A handle** comes from the script's side and goes in the message, as an effect starts on every client. `Effect::Named` carries it and the settings, so the protocol's gone up by one.
- **The budget** starts to bite past 60%. What's within 2048 units keeps most of its rate, and what's further gives way first.
- **Mesh shapes** come with mesh particles.

Filled in as each part lands.

### Compute (90)

- **Tried in the game:** 100,000 drops falling from the ceiling of a room and coming off the floor, in a box of their own, drawn out along how they move. It looks like rain. A swirl round a ring, by a vortex and wind, stays the shape the same forces give it on the CPU.
- **What it costs,** at 1920 by 1200 on the card here: 100,000 take 0.19 ms a frame to step, and no more of the scene than the clock's own wander. A million take 1.5 ms to step and about 2.5 ms to draw.
- **Everything an emitter says works.** The first version had two colours, a ball for most shapes, and no sheets, turning or stretching. Rain needs a box, so that wasn't enough. Colour and size are now eight steps along a life, turning comes from the slot's number, and `lit` takes one light for the whole effect.
- **Stick** is its own thing: it stops at the surface, and stays, gravity and all.
- **What ends isn't heard of.** `on_death` and `on_hit` would want a list back from the GPU a frame late, for what the usual kind already does. So they're an error, and so is another `kind`.
- **What another emitter starts** starts where that one was, so compute isn't told of it. It runs as usual.
- **Stopping** fades them as a whole, over what's asked. They're kept stepping until then.
- **The depth a step reads is the last frame's,** so it's read with the last frame's view. Reading it with the new one missed everything when the view jumped.
- **Bugs found in review:** a field that wouldn't build moved every later one along by one. Changing `r_msaa` left a level's particle shaders and motions made for the old samples. Two emitters at one step threw the same way, and slots near each other could too, as a slot's number and the step were added. `strength` and `far` weren't in a burst. The slots had no barrier against the last frame still reading them.

### Reloading (94)

- **Tried in the game:** with a fire going, its colour changed in the file, and in under a second it burned the new colour. A broken file changed nothing, and the console said what was wrong.
- **A thread looks at dates,** twice a second, so that a slow disc can't hold up a frame. Reading the files is on the main thread, as it's rare and it's only for those making things.
- **What's going starts again,** with what's timed from the start, so a change to a timeline or a burst shows. Particles already made keep the emitter they had.
- **Only the map's effects change.** The level isn't touched, and nothing that holds it has to find out.

### The Effects tab (95)

- **What's edited is the file as it reads,** so a setting nobody has a box for stays. Numbers and curves are typed as the file has them, which is one thing to learn. Comments go, as nothing that writes these files keeps them.
- **Where a box is** goes in the file, on the emitter. `read_effect` doesn't look at it.
- **A preview runs the system the game runs,** in the editor, so it can't look different. What the game draws of effects moved to the renderer and the map's crate so that both have it.
- **A floor stands in for the level** in a preview. Tracing the level would want the collision world the editor doesn't keep.

### Quake's particles (96)

- **Tried in the game:** a rocket fired down the first corridor of a level, in the old build and the new, in pictures at six moments. Its trail, its smoke, the explosion and how it goes look the same.
- **Each way of moving is an emitter,** made from the palette when a level loads: its ramp is a colour curve of steps, what speeds up has a `drag` below 0, fire has a `gravity` below 0, and half of a blob needed `flat_drag`, which an author can have too. `kind = square` is what the fixture's look is.
- **Where they start stays code.** Sums like the lava splash's aren't a shape, so `fixture_bursts.rs` places each one, and `EffectSystem` runs them. `particles.rs` has gone, and so has its loop, so nothing else moves particles on the CPU.
- **How far down its ramp one starts** is how old it starts, which is the same thing.
- **The most at once** went to 16,384, as the fixture had. What's placed isn't held back by the budget, as the budget is for what an author made. What the fixture has counts towards it.
- **What it costs:** 16,384 of them take 0.74 ms of a frame on the CPU, where a frame with one explosion takes a twentieth of that.
- **A bug it found:** `set_gravity` never reached effects, so authored ones fell at the usual rate on a level that changes it.

### A library (97)

- **Trying the library found a bug in reloading.** Particles already going kept the emitter they were made by, with its old picture number, and the numbers had changed. They showed as hard squares of the wrong picture. They now take up what their emitter says, by its name, and those of an emitter that's gone go. The review of item 94 said the opposite, and was wrong.
- **Sixteen new effects,** each tried in the game in a room a moment after it starts, and changed until it looked like what it is. Too many fireballs adding up gave one flat colour, so there are fewer, and fainter, and the smoke's own shape shows in them.
- **The flame sheet had a hard straight base.** It came from cutting a triangle out of noise. It's now a tongue, round at its foot, and it sways.
