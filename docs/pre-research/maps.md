# Maps

> **Pre-research.** This records decisions and early findings from before the [workflow](../../DESIGN.md#workflow) was adopted. It's an input to research, not research, and it mixes agreed decisions with suggestions. What the human has agreed is in [DESIGN.md](../../DESIGN.md).

This covers what a Quake map contains, what the engine does with each part, and which format and tools to use. Collision is covered in [stack.md](stack.md#physics-on-box3d).

Figures come from a survey of a local Steam install of Quake, read in place. No game data is in this repo.

## Approach

Maps will change quickly. For now the goal is to make the maps we have work with the systems we're building, without tying those systems to BSP.

| Question | Choice | Status |
|---|---|---|
| Runtime map format | Load BSP directly: BSP29, BSP2 and `.lit`. No converted format yet | Decided for now |
| Parsing | The `qbsp` crate, inside the importer only | Decided for now |
| Authoring | TrenchBroom, then `ericw-tools`, as external tools | Decided for now |
| Visibility | Keep the PVS, behind a query | Decided for now |
| Entities | Spawn through the Luau class table | Follows from Luau |

**The importer is the boundary.** BSP loading is one importer that produces an engine-owned map type. Everything else uses that type, so a new format or a richer map only means a new importer.

| The importer produces | Used by | Why it survives a format change |
|---|---|---|
| Static meshes grouped by material | Renderer | No faces, edges or texinfo outside the importer |
| Materials and textures | Renderer | Texture name conventions become material flags here |
| Lightmap atlas and light styles | Renderer | Another lighting method can replace it without touching the rest |
| Convex collision solids | Physics | Solids can come from BSP hulls, `-wrbrushes` or anything else |
| Submodels | Renderer, physics, Luau | Doors and lifts are just meshes plus solids |
| Entities as key/value lists | Luau | Scripts never see a file format |
| A visibility query | Renderer, server | The PVS can be swapped without changing callers |
| Contents and ambient queries | Server, sound | Water and ambience don't depend on BSP nodes |

Rules: no `qbsp` or BSP types outside the importer, and no system reads map files itself.

## What's in a map

A `.bsp` is a compiled map with everything baked in. Sizes across the survey:

| Content | Maps | Largest file | BSP2 | Most faces | Most entities | Models |
|---|---:|---:|---:|---:|---:|---:|
| `id1` original | 51 | 1.5 MB | 0 | 7,078 | 861 | 131 |
| Scourge of Armagon | 18 | 2.2 MB | 0 | 10,769 | 976 | 187 |
| `id1` re-release | 81 | 6.3 MB | 2 | 22,524 | 946 | 131 |
| Re-release `mg1` | 25 | 32.9 MB | 22 | 170,801 | 1,224 | 285 |

Stock maps are small by today's standards, but modern ones are over 20 times larger. The largest stock map builds to 19,464 triangles, and the largest `mg1` map to 464,569. BSP2 exists because those maps overflow BSP29's 16-bit fields. The largest clipnode count in the survey is 241,568, against BSP29's limit of 32,767. Supporting BSP2 from the start avoids a limit we'd hit immediately.

None of these maps carry BSPX lumps. The re-release uses `.lit` sidecar files for colour, along with `.png` textures, `.md5mesh` models and `.nav` files for bots.

## Where each part goes

| Map data | What it is | Used by | In this engine |
|---|---|---|---|
| Entities | Text key/value blocks | Luau | Parsed at load, spawned by class |
| Vertices, edges, faces, texinfo | World geometry | Renderer | Built into static GPU meshes at load |
| Textures | 8-bit palette mip textures, embedded | Renderer | Expanded to RGBA at load |
| Lighting | 8-bit lightmaps, up to 4 styles per face | Renderer | Packed into an atlas, blended in the shader |
| Visibility | Compressed PVS per leaf | Renderer, server | Kept |
| Nodes, leafs, marksurfaces | Render BSP tree | Renderer | Kept for PVS lookups and contents |
| Clipnodes | Collision hulls 1 and 2 | Physics | Rebuilt into `box3d` solids |
| Models | World plus brush entities | All | Each brush model becomes its own mesh and collision body |
| Leaf ambient levels | Water, sky, slime, lava | Sound | Kept |

## Format

**Load BSP directly for the baseline.** It already holds everything, and loading is not a bottleneck. Using `qbsp` on every map:

| Content | Maps parsed | Failed | Slowest parse | Slowest mesh build |
|---|---:|---:|---:|---:|
| `id1` original | 51 | 0 | 2.6 ms | 2.5 ms |
| Scourge of Armagon | 18 | 0 | 4.0 ms | 4.2 ms |
| `id1` re-release | 81 | 0 | 12.8 ms | 7.2 ms |
| Dimension of the Past | 13 | 0 | 3.7 ms | 4.3 ms |
| `mg1` | 25 | 0 | 60.9 ms | 65.5 ms |
| `mg3` | 22 | 0 | 65.8 ms | 45.1 ms |

All 210 maps loaded, including BSP2 and `.lit` files. A converted format would add a build step for no load-time gain, so it can wait until something actually needs it.

**Why not load `.map` source directly?** [`bevy_trenchbroom`](https://docs.rs/bevy_trenchbroom/latest/bevy_trenchbroom/index.html) supports both `.map` and compiled BSP, and `func_godot` loads `.map`. Skipping BSP gives faster iteration, but we'd lose baked lighting, visibility and collision hulls, and the original maps ship only as BSP.

## Parsing with qbsp

[`qbsp`](https://github.com/Noxmore/qbsp) 0.17.0 is MIT or Apache-2.0 and actively developed, and it brings in about 35 crates with default features. It reads BSP29, BSP2, `.lit` and [BSPX](https://github.com/fte-team/fteqw/blob/1f9f3635f0aef3b2eed6b40e35fcf6223c6ad533/specs/bspx.txt), builds meshes, packs lightmap atlases, and exposes clipnodes, which we need for collision.

Downsides: it has few users (about 1,650 recent downloads) and it pins a `glam` version. It can't write BSP, which we don't need. Since it's a small parser, we could copy it into `crates/` if we need to change it.

Its lightmap atlas wasn't tested. Neither was rebuilding collision solids for BSP2 maps, because the earlier collision test only read BSP29.

## Authoring

The established pipeline works unchanged:

1. **[TrenchBroom](https://trenchbroom.github.io/manual/latest/)** edits `.map` files. It takes a custom game config and an FGD file listing entity classes. We can generate the FGD from our Luau entity definitions, as `bevy_trenchbroom` does from Rust.
2. **[`ericw-tools`](https://ericwa.github.io/ericw-tools/)** compiles to BSP and bakes lighting and visibility. It's GPLv2+, used as a separate program.

`ericw-tools` also gives us a path for improvements later: [`-wrbrushes`](http://ericwa.github.io/ericw-tools/doc/qbsp.html) for real brush data, which allows collision at any size, plus RGB lighting in the BSP, per-surface lightmap scale, and bounce lighting.

## Rendering on vulkane

- **World geometry.** Faces are triangulated into static buffers once per map, grouped by texture.
- **Visibility.** Keep the PVS. It's already in the map, it's cheap, and the server needs it anyway to decide what to send clients. A [small Vulkan BSP renderer](https://github.com/kondrak/quake_bsp_vulkan) reports 0.38 ms with PVS and frustum culling against 1.01 ms without, though that's a hobby project's own number. I found nothing on how vkQuake handles this.
- **Lightmaps.** One atlas, with light styles blended in the shader, so animated lighting costs no CPU uploads. Dynamic lights are a separate shader pass, not a lightmap rewrite.
- **Texture kinds.** Names select behaviour: `sky*` for two-layer sky, `*` for warped liquids, `+` for animated frames. `clip`, `skip`, `trigger` and `hint` are never drawn. In `id1` about 19% of texture entries are animation frames.
- **Palette.** The 8-bit pipeline is dropped, but textures are still stored as palette indices, so they're expanded at load.

## Entities

`id1` uses 88 entity classes:

| Group | Placed | Handling |
|---|---:|---|
| `light*` | 8,178 | Plain `light` removes itself at spawn unless it's switchable. Most lighting is already baked |
| `item*`, `weapon*` | 2,485 | Luau |
| `monster*` | 1,888 | Luau |
| `func*` | 1,334 | Luau, with brush models |
| `trigger*` | 1,167 | Luau, with brush models |
| `info*`, `path*` | 2,057 | Luau, mostly markers |
| `ambient*`, `trap*`, `misc*`, `worldspawn`, other | 464 | Luau |

Those 88 classes are the checklist for the Luau port, starting with the map entities in [stack.md](stack.md#starting-scope). Mission packs and modern maps add more: Scourge of Armagon alone has 135.

## File ids

- **The map** gets a file id like any other file.
- **Embedded textures** have no ids, because they live inside the BSP.
- **Entity keys** such as `model`, `noise` and `map` are path strings. These resolve to ids at load, so maps stay unchanged.
- **Replacement textures**, such as the re-release's `.png` files, are a later asset question.

## Open questions

- **Lighting look.** Software Quake's overbright and fullbright texels both affect how close we get to the original. Fullbright can be a mask built from the palette at load.
- **Content for a standalone release.** The engine runs on maps the player owns. Shipping a game needs original maps.
- **`.nav` and `.md5mesh`.** These are re-release additions, not part of the baseline.
