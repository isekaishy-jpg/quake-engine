# Texture standard

What a texture has to be to go in the engine. Give this to whoever makes textures, a person or a model, along with the request, and check what comes back with `check_textures`.

Why it exists: the first packs came with a dark line along every picture's edge, and nothing said what a picture had to be.

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

## Asking a model for textures

Say all of this in the request:

1. The pack and kinds wanted, and how each repeats.
2. Pixel size and `world_size` for each.
3. No border, no baked light, no mirrored edges, and that edges have to meet.
4. Colour only, unless the model can make true normal and specular maps. Those can be worked out with `derive_maps`.
5. That it'll be checked with `check_textures`, and what fails goes back.

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
target/release/engine-compile content --pack game.epak
```

Run the game on it with `-content game.epak`. See [proposal 0106](proposals/0106-packing.md).

## Decals

A decal is one picture pressed onto surfaces, so it follows different rules from a repeating set.

- **Name:** `decal_` and what it is, as in `decal_stain`, with a material of the same name.
- **Size:** 512 by 512, showing as 96 to 256 units.
- **Alpha:** the shape is in the alpha channel, and it must reach 0 by every edge.
- **Colour:** dark and neutral, so that it goes on any set. Nothing of the set's own colour.
- **Material:** `alpha = blend` and `decal = true`, which draws all of them at once.
- **Not for:** anything that has to repeat. `decal_grime`, `decal_stain` and `decal_cracks` are made from noise.
