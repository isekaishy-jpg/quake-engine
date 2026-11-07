# Land

Land is sheets of heights on a square grid. It's lit by the bake, casts and takes shadows, and is walked on straight from its heights. [Proposal 0144](proposals/0144-terrain.md) has the design and what was measured.

## A sheet

A sheet is a file under `terrain/`, with heights, holes, and what it's painted with. A level names it:

```text
terrain { source = hills origin = [-2048 -2048 0] material = ground }
```

- **Heights** are a sample every 32 units unless a sheet says, which is a player's width.
- **A chunk** is 64 cells, 2048 units at that spacing, and it's the size of a tile. A sheet is a whole number of them.
- **`origin`** is its low corner. On a tiled level, keep chunks on the tile grid, so each goes with its tile.
- **`facing = down`** makes it a roof, solid above and not below.
- **`thickness`** is how far down it's solid. With none it's solid as far as it goes, so a sheet with anything under it needs a thickness.

## Caves, overhangs and bridges

Sheets stack, and that's what makes these.

- **A cave** is ground with a hole in it and a thickness, a roof under that, facing down, with the same hole, and a floor further down.
- **A bridge** is a sheet with holes everywhere but its strip, and an underside facing down below it.
- **An overhang** is a roof sheet over ground, with holes round its edge.

Sheets don't fold. What folds back on itself is a mesh. Where sheets meet, line their edges up, as nothing does that for you. A hole is a whole cell, so its edge steps. Hide an edge that shows with a mesh.

## Shaping and painting with tools

`engine-tool` does all of this, and so can an agent. The editor does the same sums.

| Operation | What it does |
|---|---|
| `make_terrain` | A new sheet, of so many chunks, with hills if it's asked |
| `add_terrain` | Puts a sheet in the level |
| `shape_terrain` | A circle of raise, lower, flatten, smooth, hole or fill |
| `stamp_terrain` | Presses one sheet's shape into another, or scatters it with `count` |
| `copy_terrain` | Copies a circle of land to another place |
| `set_terrain_layer` | Sets one of the eight layers it's painted with |
| `paint_terrain` | A circle of a layer |

A circle has `at`, from the sheet's low corner, a `radius`, and `soft`, how much of it fades.

## In the editor

The land tools are under **Surfaces**. Choose one and drag across the land. The circle lies on the land, or on a block if that's nearer. Land changes on the screen as you go, and the level's built again when you let go. **Undo land** goes back a stroke at a time.

To paint, add a layer by its material's name, choose it in the list, and choose **paint**. The list is of the sheet you last touched.

## Layers

A sheet has up to eight layers. The first starts out everywhere, and the rest come in as they're painted.

- **Any material will do.** Land uses its colour, normal, height and specular. See the [texture standard](texture-standard.md#terrain-sets).
- **They blend by height,** so stones stand out of dirt. Give a material a true height where that matters.
- **`size`** is how wide one repeat is. Give layers sizes that don't line up.
- **`hex`** breaks up a layer's repeat near the view, for three times the reading. Use it on the one or two that show it.
- **A cost for each layer a pixel has,** and none for the rest. It's about 0.14 ms over a whole screen.
- Paint doesn't need a new bake, as light goes by the land's shape. Changing its shape does.

`engine-compile --textures` compiles what land's painted with. Without that it loads from pixels, which is slower and takes more memory.

## What it does for you

- **Steep land reads its pictures from the side,** so a cliff isn't smeared.
- **Far land is a plain colour** past 6000 units, with a soft change of colour over all of it.
- **Levels of detail** are worked out each frame, so there's nothing to set up.
- **Paths** go over land, under roofs and through holes.
- **Everything that moves** collides with it: players, monsters, and what's thrown.
