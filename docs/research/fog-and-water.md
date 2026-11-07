# Fog and water

Research for stage 11 of the [work list](work-list.md): items 70 to 76, 17, 98 to 102 and 130.

## What's there

- **Fog** is one for a level. It thickens with distance and lies low, and where the level has a probe grid it's marched and lit by the bake. There's nothing for fog in a place.
- **Water** is a box with a flat top. A brush with `contents = water` makes a `Pool` of its bounds, and the post pass takes the 8 nearest and works out what a ray does in each.
- **A level has one `water` block**, so every pool of a liquid looks the same.
- **The tint under water** is set by `tint` and `tint_strength`, which go with the pool the eye's in, so item 74 only wants checking.
- **Splashes and bubbles** are effects, and nothing starts them.

## Fog

- [Godot's volumetric fog](https://docs.godotengine.org/en/stable/tutorials/3d/volumetric_fog.html) is the usual froxel kind: a grid of cells in the view, filled, lit and summed. Its own notes give what that costs and where it goes wrong. It has a finite range, thin volumes flicker, and smoothing it over time leaves ghosts behind what moves.
- A level here has few volumes, and they're boxes, so how far a ray goes through one is an exact sum. Marching only that span, with few steps and a different start for each pixel, needs no grid, no history, and no range. What it costs goes with how much of the screen a volume covers, and it's nothing where there's none.
- Clear fog needs no steps at all: a box's thickness along a ray is length times density. Steps are only for what varies along the ray: wisps and light.

**Decided:** marched in the post pass, as the level's fog already is. No froxels. They'd pay for a whole grid every frame, to serve a few boxes.

## Water

- A river down a slope, or a pool that follows the land, isn't a box. The surface has to be a shape, and swimming has to know where it is.
- [Treating water as a height field is the usual trade](https://arxiv.org/pdf/1109.6494): it brings 3D down to 2D, and it can't do what overhangs, such as falls. Falls here are particles and meshes already.
- So a shaped body is **a grid of heights**, however it was made: from a mesh, or painted on land. One thing answers both where the surface is for the view, and whether something's in it for swimming. What's drawn is the same grid.
- Boxes stay for what's a box. They're exact, and all the levels have them.

## What moves on it

- [A damped wave equation on a height field](https://hgpu.org/?p=6574), by finite differences, is the standard answer, and it's what [simple GPU versions](https://apps.amandaghassaei.com/gpu-io/examples/wave2d/) do: two pictures, stepped each frame, with what touches the water pushed in. Normals come from its slope.
- The engine has this already in fields: a compute step that reads last frame's picture and writes the next. A field can't be told where to push, so one follows the view, and it's told.
- A wake is the same thing pushed in a shape. Trails want a mask an author draws, so a pushed-in picture, turned to how something's going, does for a V and anything else.

## Edges

- [Foam goes by the gap between the surface and what's behind it](https://bugnet.io/blog/how-to-fix-godot-water-shader-not-showing-shoreline-foam), with a smooth step and a moving picture. [The same gap fades the water at the shore](https://godotshaders.com/shader/water-with-foam-and-depth/).
- [It only sees what's on screen](https://forums.unrealengine.com/t/tidal-water-foam/14682), so what breaks the surface can show a hard edge at the side of the view. That's small, and a baked mask would cost a level a picture of every shore.

**Decided:** by the gap. The post pass already has both.
