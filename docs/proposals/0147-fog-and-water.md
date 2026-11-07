# Proposal 0147: fog in a place, and water of any shape

Status: accepted and implemented, 2026-11-07, the [work list](../research/work-list.md), items 70 to 76, 17, 98 to 102 and 130, from [the research](../research/fog-and-water.md).

## Design

### Fog (70 to 73)

- `fog_volume` blocks: a box, by `low` and `high`, with `color`, `density`, `edge` for how softly it ends, `wisps` from 0 to 1 and `wisp_size`, `drift` as a way and a speed, and `lit`. Up to 8 nearest the eye reach the post pass, as pools do.
- A plain volume is a sum. One with wisps, light or soft sides is marched over the span a ray has in it, in 10 steps, starting somewhere different in each pixel. That's done with light in the air, for a quarter of the pixels.
- `lit` takes the bake's light at each step, and the 4 nearest moving lights. It's the usual kind, where there's a probe grid. The level's own fog takes moving lights too.
- What rays there are brighten with the fog they pass through.
- Fog stops at water as it does now, so air's fog isn't under it.
- `deep` in a water look says how much thicker it gets with depth. As depth along a ray changes evenly, the sum stays exact.

### What a level says of water (74 to 76)

- `water name { ... }` blocks, as many as a level wants, and a brush says `water = name`. One with no name is what those with none get, as now.
- `view_effect` in a look is an effect that goes with the view while the eye's in that water, with `motes` in the library for it.
- Tint is checked in the game.

### Water of any shape (17, 130)

- A **water sheet** is a grid of heights, with `depth` under it. It comes from a mesh placed with `water = name`, or from land with `water` painted on it.
- `Contents` at a point asks sheets too, so swimming, sounds and the view all work.
- Sheets draw into a picture of their own before the post pass: how far away the surface is, which body it is, and which way it faces. The post pass reads that where a box would have given it the sum.
- Each sheet is one of the 8 nearest as a pool is, with a flag and its bounds.

### What moves on it (98, 99, 102)

- One **ripple field**, a square of 1024 world units on 512 pixels, follows the view, stepped by a damped wave equation at 60 steps a second. It snaps to its cells so that it doesn't swim.
- Whatever's in water and moving pushes into it. A `trail` picture in a look, turned to how it's going, is what's pushed, which makes a V or anything else. A ring is pushed for what goes in, what's shot and what's rained on, by `rain_rings` a second.
- The surface's normal adds the field's slope.
- `splash` and `spray` in a look are effects, with sounds, started by what crosses a surface, and by what goes fast through one.

### What's on it (100, 101)

- `overlay` is a picture over the surface, scrolling, with `overlay_strength`. A look can have two, for foam and scum. They go in one array, as effects' pictures do.
- `foam` is how wide it is at the shore, in world units, by the gap between surface and what's behind it, and shows a look's `foam_picture` or a noise. `shore` fades the surface out by the same gap.

## Other approaches

- **Selected approach:** the above, in the order fog, looks, shapes, ripples, edges.
- **Tradeoffs:**
  - Marching costs by how much of the screen a volume fills. It's measured below.
  - A grid of heights can't overhang. Falls stay particles and meshes.
  - One ripple field covers what's near. Far off there are only the look's own waves.
- **Rejected:**
  - Froxels: see the research.
  - Water as a surface material alone: what's under it, and swimming, need a body.
  - A field for each body: a level can have many, and only what's near shows.
- **Acceptance criteria:**
  - A level can have a fog bank in a room with clear air next to it, which drifts and is lit by a torch going past.
  - Two pools have different looks.
  - A sloped river made from a mesh can be seen, swum in, and looked out of.
  - Land can have a lake that follows it.
  - Wading leaves a wake, a shot leaves a ring, going in splashes.
  - The shore has foam, and the water fades into it.
  - A murky look like the fixture's still works.

## Review

### Fog (70 to 72)

- **Tried in the game:** a bank of drifting fog in a room, lit by a light going through it, with clear air beside it and a wall coming out of it. Standing in it works, and so does looking along its side.
- **What it costs,** at 1280 by 720, with the final pass at 0.16 ms without any: marched in the final pass, a bank across most of the view took it to 1.4 ms, and four over everything to 5.2 ms. That's too much for something soft, so it moved to the pass light in the air has, at a quarter of the pixels, and nothing about how it looks changed. There the same bank takes 0.55 ms, and the same four 1.37 ms. One over everything takes 0.42 ms.
- **`gpu_times` couldn't see any of this.** It had no span for the final pass or for light in the air, so the first numbers read as if fog cost nothing. It has both now, as `post` and `air`.
- **Plain fog stays in the final pass,** as a sum, so it keeps a hard edge against what's in front of it, and it's free.
- **Rays** were meant to brighten with all the fog in a pixel, and they do, by what's left of the marched fog and the level's own.

### What a level says of water (73 to 76)

- **Tried in the game:** a clear pool and a bog side by side. Under each, the view takes that one's tint and its thickness, and the clear pool's motes go with the view. The tint was already right, so item 74 needed no change.
- **`deep` is exact,** as depth changes evenly along a ray, so its middle gives the whole of it.
- **What goes with the view** is started and stopped by the look, and moved each frame, under a handle no script can get.

### Water of any shape (17)

- **Tried in the game:** a river down a slope, made from a mesh of four corners. From above it falls away between its banks and mirrors them. Under it the surface shows from below, with shafts through it and motes round the view. Dropped in without noclip, the player sank 71 units in about 1.7 seconds, which is swimming.
- **What it costs,** at 1280 by 720: the final pass goes from 0.16 ms to between 0.42 and 0.63 ms with a river in view, most of which any water costs.
- **The picture of surfaces holds 32 bit floats.** A shore goes by the gap between the surface and what's behind it, and a half float can't tell that to a unit at any distance.
- **A sheet's box is only where it might be.** What a ray does in one comes from the picture, or from being in it, and not from its box.
- **`Map::liquid_or`** is the one place that says open air is water, and physics, the view and the look all go through it, so that none can disagree.
- **A sample a little past a mesh's edge** takes the plane of what's nearest, or the edge of a sheet would stop a cell short of the mesh.
- **The picture doesn't look at the scene's depth.** It did at first, and putting the bug back showed that doing nothing changed: the final pass already leaves out a surface further than what's in front of it. So that went, and its binding, its barrier and a version for samples went with it.

### Water on land (130)

- **Land says how deep water is at each of its samples,** which is another thing to paint. The same grid does what a mesh's does, so all that's new is how it's made.
- **Tried in the game:** a dip lowered in flat land and flooded to a level, with the tools, seen from the bank, from above and from under it.
- **Samples next to water carry on at its height, and the land hides them.** At first they sat on the ground, so that the surface came down to nothing at the shore, and from above that shore was a saw blade, the shape of the grid. Now it's where the two meet, to the pixel.
- **`flood` fills to a level,** as most land wants a flat lake, and depth by itself can only follow the land.
- **What's solid comes first.** A sheet on land goes down as far as the deepest water, and land stops it, so it needs no bed of its own.

### What's on it (100, 101)

- **Tried in the game:** the lake from before, with foam along its shore, a fade into the bank, and a cloud drifting over it.
- **Foam goes by how deep it is at that pixel,** which is the surface's height less where the scene is. That's exact for a shore, and needs nothing baked.
- **Overlays are in one array,** as effects' pictures are, so that a level's waters don't each cost a binding.
- **How much shows rides in the fraction of how wide it is,** to keep a body's overlay to one entry of the frame.

### What goes through it (98, 99, 102)

- **Tried in the game:** rain rings on a pool, moving what it mirrors. Swimming across it leaves a V behind the swimmer, which comes out of the wave equation by itself, from nothing but rings. At first that was strong enough to tear what the water mirrored, so a wake pushes less hard than anything else.
- **What it costs:** the final pass with ripples in view is 0.39 to 0.47 ms at 1280 by 720, and the steps add about 0.01 ms a frame, which `gpu_times` shows with what moves meshes.
- **Pushes wait for a step.** There are 60 steps a second and frames come faster, so a push on a frame with no step was lost. Three in four were, at 200 frames a second. They're kept until a step takes them.
- **A wake goes by how long a frame is,** or it'd be four times as strong at 240 frames a second as at 60.
- **It goes with the view by whole cells,** and what's there moves the other way, so ripples stay where they are in the world.
- **Who's in water is worked out on the client,** from where things are, as it's only for show. It's all in one place and takes no window, so tests can run it.
- **Rain is the level's to say.** Nothing joins `rain_rings` to the `rain` effect, as a water doesn't know what's falling on it.
