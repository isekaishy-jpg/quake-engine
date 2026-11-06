# Proposal 0106: packing

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

The [texture note](../research/texture-standard.md) asks for compiled textures, block compression and one container. The human said that packing seemed reasonable, since frame rates were low.

- Every picture was a PNG, decoded at each load, with mips made then, in gamma space, and held at four bytes a pixel on the GPU.
- `.epak` existed (proposal 0068), but no tool wrote one, it stored files as they were, and it held everything in the folder.
- BC7 is the best quality for colour and measures, at one byte a pixel, and every desktop GPU has it. BC5 keeps a normal map's two channels at the same size, with the third worked out from them.
- `rusty_dds` encodes BC7 and BC5 in Rust. All 311 of our pictures compile in about a second, and the result is hard to tell from the source, so there's no need for a C encoder.
- `miniz_oxide` was already in the build, through `png`, so squeezing packs adds nothing new to trust. Files are read once a load, so a slower but smaller method than LZ4 costs little.

## Design

- **Compiled pictures.** `engine-compile <content> --textures` writes `compiled/<picture>.etex` for each picture a material names: every mip, in blocks, with a hash of its source. Colour is BC7 in sRGB, measures are BC7, and normals are BC5. Mips are averaged in linear light, wrap as a repeating picture does, and normals are made unit length again. It takes about a second, and only redoes what changed.
- **A mip says what it is.** `MipLevel::form` is `Rgba`, `Bc7Srgb`, `Bc7` or `Bc5`. The renderer takes its format from that.
- **A game reads them, a bake doesn't.** `MaterialLibrary::for_play` uses a compiled picture if there is one and its source hasn't moved on. Nothing compiled, or something stale, falls back to the PNG, so a fresh checkout plays. `MaterialLibrary::new` always gives pixels.
- **What reads pixels asks for pixels.** Stamps, stencils, stencil marks and clouds use `TextureSource::pixels`. Layers are mixed into their base at load, so a material with layers reads PNGs, and a light's cookie goes through `image_at`.
- **Two jobs, no compiling.** A picture used as colour in one material and a measure in another can't have one format, so it stays a PNG.
- **One loading path.** `play_map` is how a level is read for play, and packing uses it too, so what it finds is what a game does.
- **Packs.** `engine-compile <content> --pack <file>` compiles the pictures and writes one `.epak`. Files that are the same are held once, and each is squeezed if that helps. A pack leaves out generators, text maps with a binary form beside them, and any PNG that's compiled and that no level reads as pixels. It finds those by reading every map as a game would.
- A pack's version is now 2. No pack had been made, so version 1 isn't read.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a picture is held twice in a working folder, as a PNG and compiled, which is why `compiled/` is ignored by git. BC7 costs a little quality, measured below. A GPU without BC formats can't run the engine, which only rules out what we don't target.
- **Acceptance criteria:** a game reads compiled pictures and looks the same, stale ones aren't used, what reads pixels gets them, a pack holds what a game needs and plays, and damaged packs are refused.
- **Future extension:** none planned.

## Review

Measured on Vesper Deep, with everything compiled.

| | PNGs | Compiled |
|---|---|---|
| Pictures on the GPU | 102 MB | 26 MB |
| Content on disk | 475 MB in a folder | 68 MB in one pack |
| Load | 1.5 s | 1.4 s from a pack |
| Square, about 2560 by 1400 | 248 FPS | 248 |
| Market | 190 to 200 | 190 to 200 |
| Quay | 198 to 213 | 199 to 214 |
| Terrace | 198 to 211 | 200 to 212 |

- Frame rate didn't move. Reading pictures isn't what limits us on this GPU, so the human's hope of speed from packing wasn't met, and what it gives is memory and size. Frames were dealt with in items 1 and 4.
- Load hardly moved either, so decoding PNGs wasn't where its time went. Item 2 had already taken it from 12.5 s to 1.5 s.
- Against PNGs, two captures differed by 0.3 to 0.5 of a byte on average. Side by side they can't be told apart.
- The first attempt lost the device. Stencils and clouds were compressed blocks sent up as if they were pixels, so the copy read past its bytes. That's what `pixels` is for, and `a_compressed_texture_draws_its_colour` covers the format.
