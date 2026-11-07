# Terrain

Research for items 43 and 125 to 132 of the [work list](work-list.md), 2026-11-07. Design comes next, in [proposal 0144](../proposals/0144-terrain.md). Sources are marked for what they are: the geometry clipmaps and hex tiling papers are primary, and the rest are forums, project pages and engine documents, so they show what people do and not what's measured.

## What we have

- **Tiles** ([proposal 0089](../proposals/0089-tiles.md)): a grid, 2048 units a tile in `tile_world`, all loaded with the map, with light and paths streamed in side files ([proposal 0095](../proposals/0095-streaming-tiles.md)). Geometry is held in memory for every tile, and that's measured to be fine.
- **Light** lives on faces: a face has a rectangle of the atlas, by style, and it streams by tile. Copies of a mesh now keep one face each, which stands in for them in leaves and says where their light is.
- **Collision** is Quake's: a point traced through hulls that are already grown by a box's size. Those are built from brushes, so nothing smooth or fine fits them.
- **Painting** is layers on a material: three at most over its base, sharing eight slots, with no normal or shine of their own. The mega texture bakes layers and paint into one 2048 image a map.
- **Hex tiling** is there already, as a material's `hex_tiling`.
- **`vendor/box3d`** has a height field, with a flag for each triangle that can make a hole, for what its rigid bodies roll on.

## Level of detail (125)

- **Geomipmapping** gives each patch a level, and [neighbours of different levels need strips or skirts](https://gamedev.net/forums/topic/493389-terrain-patch-stitching-preferences/4220825/). [Skirts hide cracks but aren't right, and don't cope with large ones](https://www.gamedev.net/forums/topic/610521-terrain-stitching/).
- **Geometry clipmaps** are [nested grids round the viewer](https://www.hhoppe.com/geomclipmap.pdf), stitched with zero-area triangles. They're at their best with one huge height map that streams, and they're built round the eye rather than the ground.
- **CDLOD** [uses a quadtree, with level chosen by true distance in three dimensions](https://github.com/fstrugar/CDLOD), and its vertices slide from one level to the next as they near the boundary, so there's nothing to stitch and nothing pops. That's the author's claim, with source to read.

**For us:** CDLOD. It's a quadtree, which fits chunks on a grid, one small grid mesh is drawn for everything, and the human noticed no pop on meshes because it's built not to show.

## How far a heightfield goes (126)

- A heightfield has one height at each place, so [it can't make a cave by itself](https://discussions.unity.com/t/how-to-make-underground-areas-for-terrain/17708).
- What engines do is [a hole in the land, and meshes for the rest](https://forums.unrealengine.com/t/terrain-caves/2290). In Unity that's seen as [a poor answer](https://discussions.unity.com/t/its-embarrassing-how-primitive-unitys-terrain-and-open-world-solutions-are-in-2023/928847), and [its holes have left collision behind](https://discussions.unity.com/t/its-embarrassing-how-primitive-unitys-terrain-and-open-world-solutions-are-in-2023/928847). [Digger](https://discussions.unity.com/t/digger-caves-overhangs-for-unity-terrains/726282) goes further, with voxels and marching cubes beside the heightfield.
- [Research has tried hybrids of heights and voxels](https://core.ac.uk/works/29101128) for the same reason.

**For us:** the human wants to sculpt what's underground, and a prefab isn't sculpting. Voxels are a whole second system, with their own light, levels and collision. There's a cheaper way that gets most of it: **more than one sheet of heights**, each a heightfield of its own, and any of them facing down. Then:

- A cave is a hole in the ground, a floor below it, and a sheet facing down for its roof.
- An overhang is a sheet facing down that meets the ground at its edge.
- A bridge over a road is a sheet facing up with one facing down under it.

Each sheet is the same thing to draw, collide with, light and paint. What it can't do is a shape that folds back on itself within one sheet, and that's what meshes are for. Where sheets meet, an author lines their edges up, and a hole hides what's left.

## Painting (128) and blending (131)

- Plain splatting costs [a read for each layer, and it adds up](https://gamedev.net/blogs/entry/1682713-terrain-texturing): one developer found 10 layers could come to about 80 reads.
- **Only the layers that matter:** [work out the N most important for each patch and read only those](https://gamedev.net/blogs/entry/1682713-terrain-texturing). WoW's terrain has [up to four a chunk](https://github.com/mangosArchives/Mangos-Zero-server-old/wiki/ADT-Files).
- **Virtual texturing** [renders layers into a cache once](https://www.gamedeveloper.com/game-platforms/virtual-texture-terrain), so a pixel costs the same however many there are, at [a cost in sharpness far off](https://www.leadwerks.com/community/blogs/entry/1112-virtual-texture-terrain/). It's a big system, and it's what the mega texture is the simple end of.

**For us:** a GPU that skips what has no weight pays for what's there, and most pixels have one or two layers. So land holds eight layers in texture arrays, each with colour, normal and shine, and a pixel reads only those with weight. That's up to four reads a layer, with height in the normal's blue channel to blend by. It's measured in the proposal's review, not assumed.

For far land, the same layers baked into one colour for each chunk are what a distant pixel reads, the way the mega texture already mixes a surface down. That also answers item 131's question for big land: near land blends, and far land is baked, a small image a chunk, kept with the chunk's light.

For **materials**, more blended layers each with its own normal and shine go the same way, as it's the same sum, and that's for the review to measure.

## Tiling (132)

- **Hex tiling** [is the most effective, and costs the most](https://docs.unity3d.com/Packages/com.unity.shadergraph@17.4/manual/Shader-Graph-Sample-Terrain-Solutions.html): [6 to 9 reads a layer](https://jcgt.org/published/0011/03/05/paper.pdf), with a blend that has to keep contrast. We have it.
- **Rotation** is [two reads, and cheaper, but not as good](https://docs.unity3d.com/Packages/com.unity.shadergraph@17.4/manual/Shader-Graph-Sample-Terrain-Solutions.html).
- **Large-scale variation:** [a soft grunge or colour map over the whole](https://bugnet.io/blog/how-to-fix-unity-terrain-splatmap-tiling-repetition-artifacts) is nearly free, and tiling each layer at [a size that doesn't line up with the others](https://bugnet.io/blog/how-to-fix-unity-terrain-splatmap-tiling-repetition-artifacts) costs nothing.

**For us:** those two cheap ones for every layer, always. Each layer has its own world size, and land has a colour variation from low noise. Hex tiling is there to turn on for a layer that needs it, near the view, as it's six reads.

## Collision (129)

- A heightfield [is thought faster than a mesh for its regular grid](https://discourse.panda3d.org/t/performance-of-collision-heightfield-vs-collision-mesh/11885), though [one engine's developer sees little difference but memory](https://forum.babylonjs.com/t/is-the-hp-shape-createheightfield-function-usable/44191).
- **Holes are where engines fall down:** [Bullet's has none](https://hub.jmonkeyengine.org/t/heightmap-with-ground-holes-not-caves/20609), and [nor did Newton's](https://www.leadwerks.com/community/topic/61295-collision-and-terrain-holes). [Box3D's has a flag for each triangle](https://box2d.org/documentation3d/group__height__field.html).

**For us:** the human asked for collision straight from the heights. Hulls can't do it, so a moving box is swept against the triangles of the cells it crosses, which is few, and read from the heights with no mesh kept. A surface only stops what's going into it, from the side it faces, which is what lets sheets stack, and is forgiving at their edges. Box3D gets the same land for what rolls.

## Water (130)

Pools are boxes. Item 130 waits on item 17, water on meshes with depth, at stage 11, so it's done there, on this land.

## What this means for the design

- A sheet of heights on a grid, chunks of it on the tile grid, as many sheets as a level wants, each facing up or down, with holes.
- CDLOD: one small grid, a quadtree, and vertices that slide.
- **A chunk is a stand-in face**, as a copy of a mesh is now. So light, tiles, streaming and leaves are what's there already, with samples lifted onto the land.
- A box swept against triangles from the heights, one-sided.
- Eight layers in arrays, read by weight, with far land baked.
