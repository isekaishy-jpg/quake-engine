# Proposal 0147: fog in a place, and water of any shape

Status: proposed, 2026-11-07, the [work list](../research/work-list.md), items 70 to 76, 17, 98 to 102 and 130, from [the research](../research/fog-and-water.md).

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

Filled in as each part lands.
