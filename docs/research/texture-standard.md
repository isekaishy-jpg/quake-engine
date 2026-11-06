# Texture standard and packing

Status: a direction, not a design. This hasn't been through the workflow and nothing here is approved to build. It goes through research, design and proposal later.

The human set this direction on 2026-11-06.

## Why it came up

What's in the repo is usable pictures, with no standard behind them. The dark line along every delivered picture, and a repair that made some worse, came from nothing saying what a texture has to be.

The next time assets are needed they're on a standard, above all when a model is asked for them. A written spec goes with the request, and what comes back is checked against it.

## What exists now

- 311 PNGs, 28 MB. Most are 256, and those blended by `heal_seams.py` are 232. Nothing sets a size.
- Every PNG is decoded and its mips built on the CPU at each load.
- On the GPU they're plain RGBA8, about 100 MB by estimate. At 1024 that would be about 16 times as much.
- There's no sRGB handling, so light is worked out in gamma space.
- A set is a picture, with normal and specular generated from it. Nothing holds height, gloss or occlusion per pixel.
- `.epak` is a simple archive of files.

## Chosen

- **The best colour quality we can get.** Proper sRGB, with light worked out in linear space. It changes how every level looks, so levels get tuned again, and the murky Quake look has to be kept on purpose. Done early, as later assets and lights are tuned against it.
- **Desktop only.** That settles compression on block formats used directly: BC7 for colour, BC5 for normals, BC4 or BC7 for packed maps. Basis Universal isn't needed.
- **Compile textures ahead of time.** PNGs stay as source and don't ship. Runtime textures have their mips built and are cached by content hash, so a load does none of that work.
- **One container.** `.epak` grows compression and sharing of identical content, and holds textures, shaders and meshes, as [shaders](effects-water-shaders.md) asked for.
- **Metal and roughness are supported,** for [glTF](mesh-support.md), even if they aren't the main way of working. Whether all four of metal, roughness, specular and gloss can live together is below.

## Suggested, not settled

- A material says how much of the world a tile covers, whatever its pixel size. Today pixels set that, which is why a resize made everything bigger. With this, resolution can rise without touching a level.
- Power of two sizes, and a target of 2 to 4 pixels per unit. Today's 1 pixel per unit is why surfaces look soft up close. The 232 pictures are an exception to clear when assets are redone.
- A delivery spec: sizes, how a picture repeats (both ways, sideways only, or shown whole), no border lines, and what maps come with it.
- A fixed set: colour with alpha, normal, and one packed surface map. Generated maps stay as a fallback.
- One naming pattern, and a small file per set with its world size, how it repeats and what it's made of.
- An import step that checks all of that and says what's wrong, with the seam measure from `heal_seams.py` work in it.

## All four of metal, roughness, specular and gloss

They look possible together, as they aren't four separate things. Roughness and gloss say the same thing the opposite way round, so one channel holds either. Metal and specular both say how a surface reflects, and a material could say which it means. A packed map has four channels, so what's left over can hold height and occlusion. This needs checking against the shading work, and against how glTF's model converts.

## Notes for the research

The rest is wide research. These are starting points, and the first searches were thin.

- [KTX2](https://www.phoronix.com/news/Khronox-KTX2-Format) could be the format inside `.epak`, as tools can open it. Whether it holds BC data directly needs checking: [a forum post](https://forum.babylonjs.com/t/supporting-bc5-and-bc7-device-specific-compression-formats/35045) says tools don't support it, and that's unconfirmed.
- [`intel_tex_2`](https://docs.rs/crate/intel_tex_2/latest) looks like the BC7 encoder, last updated in May 2024. [`ctt`](https://docs.rs/crate/ctt/0.3.0/source/README.md) claims BC7 and KTX2 output. Neither has been tried, and a dependency needs its own proposal.
- How packed maps survive compression hasn't been looked at. It needs testing on real pictures.
- Compression may or may not help frame rate. At 2000 by 1125 the GPU took about 4 ms a frame, where 130 FPS is 7.7 ms, so much of a frame was somewhere else when that was measured. Measure before counting on it, and treat frame rate as a question of its own.
- Moving to linear light touches the lightmaps, the post pass and every shader, so it's tied to the [shader work](effects-water-shaders.md).
- [Vulkan's texture compression notes](https://saschawillems.de/files/Vulkanised_2021_texture_compression_in_vulkan.pdf) are a place to begin.
