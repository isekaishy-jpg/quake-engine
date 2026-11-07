# Texture standard

What a picture has to be to go in the engine. Give this to whoever makes art, a person or a model, along with the request, and check what comes back with `check_textures`.

Why it exists: the first packs came with a dark line along every picture's edge, and nothing said what a picture had to be. Art of many kinds goes in the engine, and only some of it tiles, so the first thing to settle about any picture is which kind it is.

## The kinds of art

| Kind | What it's for | Section |
|---|---|---|
| Repeating set | Walls, floors, rock, anything that tiles both ways | [A set](#a-set) |
| Trim sheet | Moulding, edging and bands, shared by many pieces | [Trim sheets](#trim-sheets) |
| Kit sheet | One sheet for a family of meshes that go together | [Kitbashing](#kitbashing) |
| Unique mesh texture | One model with its own UVs | [Unique mesh textures](#unique-mesh-textures) |
| Decal | Stains, cracks and writing pressed on surfaces | [Decals](#decals) |
| Cutout | Railings, grilles, leaves: detail with holes in it | [Cutouts](#cutouts) |
| Terrain set | Ground that's painted, not placed | [Terrain sets](#terrain-sets) |
| Flipbook | Fire, smoke and anything that animates | [Flipbooks](#flipbooks) |
| Sky art | Clouds, horizons and sky pictures | [Sky art](#sky-art) |
| UI art | Frames, icons, pointers, backgrounds and splashes | [UI art](#ui-art) |
| Painted scene | A whole picture shown as it is | [Painted scenes](#painted-scenes) |

## A set

A set is the pictures of one surface, and a material that says how they're used.

| File | Holds | Needed |
|---|---|---|
| `textures/<name>.png` | Colour, with alpha if the surface needs it | Always |
| `textures/<name>_normal.png` | A tangent space normal map, Y up | For anything with relief |
| `textures/<name>_specular.png` | Specular colour, with gloss in alpha | For anything that shines |
| `materials/<name>.emat` | How they're used | Always |

`<name>` is `<pack>_<kind>`, in lower case with underscores, such as `qg03_elder_crypt_wall_blocks`. A pack's id is two letters, two digits, then its name.

## Pictures

- **Format:** PNG, 8 bits a channel. RGB, or RGBA where alpha means something.
- **Size:** square or not, each side a power of two, from 64 to 2048. A surface seen close up wants 1024.
- **Sharpness:** 2 to 4 pixels for each world unit, on anything a player can walk up to. So a tile 256 units across is 512 to 1024 pixels.
- **All of a set is one size.**
- **No border.** Nothing drawn along an edge: no line, frame or vignette. The last row and column are ordinary picture.
- **No baked light.** No shadows from a light, or highlights, in colour. Crevices can be dark, as they would be in any light.
- **Colour is sRGB.** Normals and specular are data, not colour.

## How it repeats

Every set says one of these, in its material, and what it says is checked.

| `repeat` | Means | What's checked |
|---|---|---|
| `both` | It tiles in both directions | Opposite edges meet, in shape and in tone |
| `across` | It tiles side to side only, as a trim does | Left and right edges meet |
| `none` | It's shown whole, as a door or a panel is | Nothing at its edges |

For `both` and `across`, opposite edges have to meet as if the picture went on. Mortar, cracks and planks that reach an edge carry on at the other side, at the same place and in the same tone. Don't get there by blending or mirroring edges: make it tile from the start. If something is meant to fall on the edge, like a plate's join, it falls on all four sides in the same way, and the set is still `both`.

A strong one-off feature, such as a big stain, shows at every repeat. Keep those for layers and decals.

## The material

```text
format 1 material

surface {
    base = textures/uc01_blight_sewers_wall_blocks.png
    normal = textures/uc01_blight_sewers_wall_blocks_normal.png
    specular = textures/uc01_blight_sewers_wall_blocks_specular.png
    world_size = 256 256
    repeat = both
}
```

`world_size` is how many world units one repeat covers, across and down. It's what sets a surface's size in a level, whatever size its pictures are, so a set can go up in resolution and no level changes. A brick is about 32 by 16 units, a door 64 by 112, and a player 56 tall.

## How a surface shines

There are two ways to say it, and a material can use both.

| Fields | Says | Comes from |
|---|---|---|
| `specular`, `gloss`, `specular_strength` | A map of shine colour, with gloss in alpha | Our own sets, or `derive_maps` |
| `metal_rough`, `metallic`, `roughness` | A map with occlusion in red, roughness in green and metal in blue, or just numbers | glTF, or anything made for it |

With a map, `metallic` and `roughness` scale it. Without one they're the whole answer, so `metallic = 1` and `roughness = 0.3` is polished metal. Roughness is gloss the other way round, and metal says that a surface's colour tints what it reflects, not what it's lit by. A surface using both ways adds them. See [proposal 0108](proposals/0108-metal-and-roughness.md).

## Trim sheets

A trim sheet is a stack of bands, each of which repeats side to side. A brush face or a mesh takes one band, so a building's edges, mouldings and pipes come from one small set of pictures.

- **`repeat = across`,** with left and right edges that meet, along the whole height of the sheet.
- **Bands** are whole powers of two tall, such as 32, 64 and 128 pixels, and start on a multiple of their own height, so mips don't bleed one into the next.
- **Leave 4 pixels of plain colour** between bands that differ a lot in tone.
- **List the bands** beside the sheet, with what each is, in pixels from the top.
- **One sheet per style.** A trim sheet at 1024 by 1024 replaces dozens of small pictures.

## Kitbashing

A kit is a family of pieces made to go together: walls, corners, arches, pillars, rails. What makes a kit cheap is that its pieces share pictures.

- **One material for a kit,** or two at most: its kit sheet, and the trim sheet. Things that share a material draw together.
- **A kit sheet** lays out what its pieces need: some repeating areas, some trims, some one-off details. Mesh UVs point into it. Repeating areas sit against an edge of the sheet that wraps.
- **Size:** 2048 by 2048 for a whole kit of large pieces, 1024 by 1024 for a small one.
- **World size stays the same across a kit.** A brick is the same size on every piece, so pieces line up. Say the pixels per unit once, for the whole kit, and hold to it.
- **Colour variants don't need new pictures.** A material with `parent` changes its numbers and tint, and shares all its pictures. A burnt or mossy version is a layer or paint over the same sheet.
- **Pieces snap by the [mesh standard](mesh-standard.md).**

## Unique mesh textures

A hero model has its own UVs and its own pictures, such as a statue or a well.

- `repeat = none`, and a set in the usual form.
- **Leave 8 pixels of padding** round each island, in its own colour, so that mips don't show what's next to it.
- Pixels per unit match what's around it, or it looks soft beside its neighbours.
- Use one only when a kit can't do it: each costs memory that a kit piece doesn't.

## Cutouts

Railings, grilles and leaves show their detail and have holes between. As they're drawn with `alpha = mask`, they sort for free.

- **Alpha is the shape,** in a clean hard edge, and one set of pictures carries it.
- **Bleed colour past the edge.** Where alpha is 0, colour carries on from the nearest solid pixel, by at least 8 pixels. Otherwise a dark fringe shows as mips soften.
- **Leave 2 pixels clear** round the whole picture.
- Soft edges, such as stained glass, use `alpha = blend`, and cost more, as they're drawn in order.

## Terrain sets

Terrain is painted from sets of ground, blended together, so each has to work beside the others. This section firms up with the terrain work on the [work list](research/work-list.md).

- **A repeating set,** at 1024 by 1024, with colour, normal and height.
- **Height matters here.** Blends follow it, so stones stick out through dirt. Give true height, not brightness.
- **Keep it plain.** Over a large area any feature that stands out shows as a pattern. Save those for decals and meshes.
- **All sets in a family share a tone and a world size,** so that they blend.
- **Eight to a sheet.** A sheet of land has up to eight layers, and a pixel reads only those it has. Each layer sets how wide its repeat is, so give sets sizes that don't line up with each other.
- **A material's fine as it is.** Land uses a material's colour, normal, height and specular, at the size of the largest one a level paints with, up to 1024. With no height, its light and dark stand in, so give a true height where the blend matters.
- **The layer of a material** can name its own `normal`, `specular` and `height`, as a surface does. Up to three of those on a material, each one read more where it's used.

## Flipbooks

- A grid of frames in one picture, left to right and then down, each the same size and a power of two.
- `repeat = none`, with alpha for its shape, and the last frame running on to the first if it loops.
- Keep what's in each frame clear of the frame's edge by 4 pixels.
- A flame needs about 16 frames at 128 by 128, in a 512 by 512 picture.

## Sky art

- **Cloud layers** are a picture with alpha, repeating both ways, as they drift. No sun or light direction painted in strongly, as sun moves.
- **Horizons** are a strip that wraps side to side, with alpha fading to nothing at its top, and nothing below the line.
- Colour only. Skies aren't lit by anything else.

## UI art

UI art isn't compiled. It's kept as PNGs at its full quality.

- **Scale.** Draw at twice the size it shows at 1080p, so that it stays sharp at 4K.
- **Frames** are nine-sliced, so that any size works. Corners stay the same and edges stretch or repeat. Say the slices in pixels, in the kit's `theme.json`, and keep edges plain enough to stretch.
- **States.** A control comes in normal, hover, pressed, focused, selected and disabled, each the same size, so that nothing moves.
- **Icons** are square, with their shape in alpha, and clear of their edge by a tenth of their size. They have to read at 32 pixels.
- **Pointers** say their hot spot in pixels.
- **Backgrounds and splashes** are 3840 by 2160, with what matters inside the middle 16 by 9, so that other screen shapes only lose the edges.
- **No text in pictures.** Words are drawn by the engine, so that they can change.

## Painted scenes

A picture shown whole, such as a title scene or a map.

- `repeat = none`, at the size it's shown.
- Layers that move or are lit differently come as separate pictures of the same size, with alpha, not flattened.
- Maps that go with it, such as depth, occlusion or regions, are the same size and line up pixel for pixel.

## Saving space

What's on disk and on the GPU comes down to a few rules.

| Picture | Compiled as | Bytes a pixel, with mips |
|---|---|---|
| Colour | BC7, sRGB | 1.33 |
| Normal | BC5 | 1.33 |
| Specular, surface maps | BC7 | 1.33 |
| UI art | Kept as it is | 5.33 |

A set of colour, normal and specular at 1024 by 1024 comes to about 4 MB.

- **Reuse before making.** A new tint, a layer or an instance of a material costs nothing. A new picture costs 4 MB.
- **Share maps.** Materials of the same stone can share a normal map.
- **Pack.** Occlusion, roughness and metal go in the three channels of one `metal_rough` picture.
- **Size by how close it's seen.** 1024 for what's walked up to, 512 for what's overhead or far off, 256 for distant things. Nothing is bigger than it's seen.
- **Identical files are held once** in a container, so copies do no harm there, and do on the GPU.
- **Kits and trims first.** A level should mostly use kit sheets and trims, with repeating sets for large surfaces, and few unique textures.

## Asking a model for art

Models can make far more than colour. Ask for each map you want, and don't derive what can be asked for. Say all of this in the request:

1. The kind of art, from the table at the top, and how it repeats.
2. Pixel size and `world_size`, and for a kit its pixels per unit.
3. The maps wanted, each lined up pixel for pixel with the colour: normal, height, specular with gloss in alpha, occlusion, roughness, metal, emissive, alpha.
4. For a set: no border, no baked light, no mirrored edges, and that edges have to meet.
5. For a sheet: where each band or area goes, and the padding.
6. That it'll be checked with `check_textures`, and what fails goes back.

`derive_maps` can work normal and specular out of colour, as a fallback for art that came with none.

Keep what was asked for. A request goes in a text file beside the source art, outside `content`, so that a set can be made again or extended in the same style.

## Checking

```bash
target/release/engine-tool content - check_textures "{\"prefix\":\"uc01\"}"
```

This lists what's wrong with each material. An empty list is a pass. A real ledge or join on a repeat line can read as a step, so look at those and decide.

## Compiling and packing

Pictures are delivered as PNGs and stay that way in the folder. For a game they're compiled, and nothing else about them changes:

```bash
target/release/engine-compile content --textures
```

That writes `compiled/<picture>.etex`: colour as BC7 in sRGB, measures as BC7, and normals as BC5, with their mips. Only what changed is redone, and a compiled picture that's behind its PNG isn't used. A normal map's blue channel is worked out from the other two, so it has to be a true unit normal.

To make one file of all content, with what only making content needs left out:

```bash
target/release/engine-compile content --pack common.pkt
```

Put `common.pkt` beside the program and it loads by itself, or run the game on it with `-content common.pkt`. See [proposal 0106](proposals/0106-packing.md).

## Decals

A decal is one picture pressed onto surfaces, so it follows different rules from a repeating set.

- **Name:** `decal_` and what it is, as in `decal_stain`, with a material of the same name.
- **Size:** 512 by 512, showing as 96 to 256 units.
- **Alpha:** the shape is in the alpha channel, and it must reach 0 by every edge.
- **Colour:** dark and neutral, so that it goes on any set. Nothing of the set's own colour.
- **Material:** `alpha = blend` and `decal = true`, which draws all of them at once.
- **Not for:** anything that has to repeat. `decal_grime`, `decal_stain` and `decal_cracks` are made from noise.
