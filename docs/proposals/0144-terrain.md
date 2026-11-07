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

### A sheet, walked on and lit (125, 129)

Built as designed, and tried in the game on a land of 4 by 4 chunks.

- **Each chunk is one chart.** The bake already had charts, a light map for a mesh with a sample on the surface and facing as it does. A chunk's samples are lifted onto the land, so it needed no new way of lighting. Land blocks light by stepping along a ray and asking if each place is in it.
- **Collision** sweeps a box against the triangles of the cells it crosses, by when each separating axis stops separating them. The real movement code walks it: a gentle slope is climbed, a cliff stops someone, and rough land never gets them stuck in it.
- **The crack test took five tries, and they're worth writing down.** From straight above a crack can't be seen, as it's an upright slit. At a low angle what's behind a crack is more land. Then it failed on nothing but the sky, and on the corner of the sheet. It looks down at a slant now, across a change of level, at white land: 0 pixels show through, and with sliding turned off, 959.
- **A small land's file can be valid text,** so it's read as binary and not guessed at. The test for that found it.

### Holes, shadows and paths (126, 129)

- **Holes** are left out of the picture and depth by a bit for each cell, and light goes through them in the bake and in shadow maps, as a roof with a hole in it shows.
- **Shadow maps** draw every chunk a light reaches, at a fixed level, since what a light reaches has nothing to do with where the view is.
- **Paths** needed one change: the walkable grid asks if a box fits, and that now looks at land too, by the corners of its feet, or of its head under a roof.
- **Leaves:** land in no leaf that can be seen isn't drawn.

Five bugs were put back, and each fails a test: light ignoring holes, land left out of shadow maps, holes left in the picture, land casting nothing in the bake, and paths not knowing it's there.

### Stacking, shaping and the brush (126, 127, 45)

- **Bridges and caves work.** The real movement code walks a road under a bridge and stands on its deck, falls through a hole into a cave, and walks on under the land, with a roof over it. A bridge is a sheet up and a sheet down, with holes everywhere but its strip. So one sheet can be a lot of holes for a small thing, and a sheet's a whole chunk at the least.
- **Thickness matters.** A sheet with none is solid as far as it goes, so a roof under ground needs a thickness, or everything above it is in solid. The first cave test showed that.
- **Shaping** is in `engine_map`, so `engine-tool` and the editor do the same sums: `shape_terrain`, for raise, lower, flatten, smooth, hole and fill, then `stamp_terrain`, which scatters too, and `copy_terrain`.
- **The brush** is a real circle: it lies on land, following its shape, or flat on the face of a block, whichever's nearer. Tried in the editor, through its commands: a hill came up under it, and a hole went through to what's below. A step goes straight to the screen, with `Renderer::update_land`, and the map's built again when the stroke ends. **Undo land** goes back a stroke at a time.
- **Pictures of the editor were wrong.** A capture drew the scene across the whole window, not in the view, so what's drawn over the view didn't line up in it. The brush seemed to miss the block. A capture now puts the scene where the window has it.
- **Every land operation has been failed on purpose:** with lower going up, an unknown source let through and copying doing nothing, the test says so. Flatten now needs its height, and won't guess.
- **The Terrain tab,** item 44, is still at stage 15, with the other tabs. For now the tools are under Surfaces.

### Painting (128, 132, and land's part of 131)

- **A pixel reads two pictures a layer, not four.** Each layer's material becomes a colour and a surface, which is bumps in red and green, height in blue and shine in alpha, at compile. A map's go in two arrays. `engine-compile --textures` compiles them to `compiled/land`, and pixels stand in where that's missing or behind.
- **Weights sit by the heights,** in a buffer, two words a sample, not the two images the proposal said. It's a texel a cell, as Unreal's is, and height blending gives the edge its detail. A brush writes it in place, with no image to bind for each sheet.
- **Height blending is one sum.** A layer's share is its weight times two to the ten times its height, so it needs no second pass over what's been read.
- **Far land has no bake.** The proposal baked a colour for each chunk. Past 6000 units a layer reads its own last mip, which is the mean of the picture. That costs next to nothing, and it can't go out of date as paint changes.
- **Cliffs are read from the side.** The proposal left this out, and the first picture showed why it can't be: a picture drawn out down a cliff. Where land's steep each layer reads along the side too. What a vertex says of a cliff can be far from true on a coarser level, and the test for this caught that, so the triangle's own slope has the last word.
- **Tiling:** each layer has its own world size, a soft colour change from noise goes over all land, painted or not, and hexagon tiles are there for a layer that asks, up to 2400 units away.
- **A bake lasts through paint,** as it goes by the land's shape. A sheet's painted without light being baked again.
- **Tools and editor:** `set_terrain_layer` and `paint_terrain`, and a paint tool beside the other land tools, with a list of the layers of the sheet last touched. `validate` says if a layer has no material.
- **What it costs,** with every layer on every pixel of a full-screen view, which is the worst there can be: lowest of two rounds, in ms for the scene. None 1.07, one 1.01, two 1.17, four 1.47, eight 2.04. So a layer is about 0.14 ms at worst, and most pixels have one or two. One with hexagon tiles is 1.17, and four 2.15, so hexagon tiles cost about twice as much as plain layers, as they're near the view. The GPU's clock moves a scene by more than that between runs, which is why it's the lowest.
- **Every test here was seen failing:** with height blending off, one picture for all, paint not sent, far land never plain, hexagon tiles never and always, no side reading, and no triangle slope. The far land test passed with its bug at first, since a picture's own mips came to the same thing. It now has a repeat too wide for that.

### Blending on materials (the rest of 131)

- **How many:** eight on land and three on a material. A layer's two reads, and it's about 0.14 ms over a whole screen at worst. Land's the only thing a player can see a whole screen of, and it wants eight, as a landscape has that many grounds. A material's on a wall or a floor, and paint on it is a stain or a patch of moss, so three stays, and paint keeps its four channels.
- **A layer has its own bumps and shine.** A `layer` block can name `normal`, `specular` and `height`, as a surface does. They're packed into one surface picture, which is one read more for a layer that has one, and nothing for one that doesn't. They blend as its colour does. Without them a layer keeps what's under it, as it always has.
- **A material has twelve slots,** up from eight, as three layers with a colour and a surface each wouldn't fit with a base, a normal and a specular. That's four more bindings, and a material that doesn't use them reads nothing more.
- **Baking takes the cost away,** as the item said it could. The mega texture now mixes a layer's bumps and shine into its normal and specular images, as it did its colour. `flatten` has only the colour to go on, so a layer with a surface stays live rather than lose it.
- Tried in the game: wood painted over rock, with each one's own bumps, and the wood's shine under a lamp.

Filled in as each part lands.
