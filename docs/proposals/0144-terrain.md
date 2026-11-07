# Proposal 0144: terrain

Status: proposed, 2026-11-07, the [work list](../research/work-list.md), items 43 and 125 to 132, from [the research](../research/terrain.md).

## Design

### Sheets (125, 126)

A level has any number of **sheets**. A sheet is heights on a square grid, by default a sample every 32 units, which is a player's width.

- It's kept in a file of its own, under `terrain/`, as heights, holes, and paint. A level names it in a `terrain` block, with where it goes.
- It can face up or down. Up is ground, solid below. Down is a roof, solid above. Each has a `thickness`, so a bridge is solid for only so far.
- A hole is a cell with nothing in it, for drawing, collision and light.
- It's cut into chunks of 64 cells, 2048 units, which is a tile.

### Each chunk is a stand-in face

A chunk has one face in `Map::faces`, flat, which never draws. That gives it what a copy of a mesh has, and nothing new is needed for:

- **Light:** a rectangle of the atlas, by style, streamed with its tile. Its samples are lifted onto the land when they're baked.
- **Leaves:** a chunk is found through the leaves it touches, so what's out of sight isn't drawn.
- **Tiles:** a chunk goes with the tile it's in.

### Drawing (125)

- Heights and normals go up to the GPU as storage buffers, once.
- CDLOD: each frame a quadtree over the chunks picks a stride for each node by distance. Strides go 1, 2, 4, 8 and 16 cells.
- A node is one instance of a small grid mesh, in a few sizes. A vertex reads its height from the buffer, and near the edge of its range it slides onto the next coarser lattice, so nothing's stitched.
- It comes out as a `VertexOut`, so all that follows is what the world has.
- It goes through the depth pass, and into shadow maps.

### Everything else (129)

- **Collision:** `Collision::move_box` sweeps a box against the triangles of the cells it crosses, read from the heights. A surface stops what goes into it, on the side it faces. A line is the same with no size. `in_solid` and `contents` know what's under a sheet.
- **Light:** the bake's surface samples go by the heights, and sheets block light by their triangles, as meshes do.
- **Visibility:** leaves, as above.
- **Paths:** the walkable grids are built with collision, so they find land by themselves. That's to be tried.
- **Box3D** gets a height field for what rolls.

### Painting (128, 131, 132)

- A sheet has up to eight layers, each a material's colour, normal with height in blue, and shine, in texture arrays of one size.
- Weights are two RGBA images over the sheet, at a texel a cell or finer.
- `fs_terrain` reads weights, and reads only layers with some, blended by height.
- Each layer has its own world size, and land has a large soft colour variation from noise. Hex tiling can be turned on for a layer.
- Far land reads a colour baked for each chunk, past a distance.
- How far this goes for materials is measured in review.

### Shaping (127), and the brush (45)

`engine-tool` gets operations for all of this, as everything else has: make a sheet, raise, lower, flatten, smooth, stamp, splat, copy, paint, cut a hole, by a circle with a soft edge. Those go through one piece of code that the editor's brush calls too. Item 45 asks for a real circle on terrain and geometry in the editor, and that's built here, as 127 and 128 wait on it. The Terrain tab, item 44, stays at stage 15 with the rest of the tabs.

### Water (130)

At stage 11, after item 17, which it waits on.

## Proposal

- **Selected approach:** the above, in this order: a sheet drawn, then collision, light and visibility, then holes and stacking, then painting, then shaping.
- **Tradeoffs:**
  - Sheets stack but don't fold. What folds is a mesh.
  - Authors line sheets up where they meet.
  - One face a chunk means a chunk has one set of light styles.
- **Rejected:**
  - Voxels: a second world, with its own of everything.
  - Clipmaps: they want one huge image that streams.
  - A mesh baked from the heights at compile time: no levels of detail, and a level's size would go with its land.
  - Terrain in Quake's hulls: fine shapes blow them up, as the barrels showed.
- **Acceptance criteria:**
  - A sheet draws, with no cracks or pops, at a good frame rate over a large land.
  - A player walks it, up slopes and not up cliffs, and falls through a hole.
  - It's lit and shadowed by the bake, and casts on what stands on it.
  - What's behind a wall isn't drawn.
  - A cave, an overhang and a bridge can be made, and walked.
  - Eight layers paint, and what it costs a pixel is measured.
  - All of it can be made with `engine-tool`.

## Review

Filled in as each part lands.
