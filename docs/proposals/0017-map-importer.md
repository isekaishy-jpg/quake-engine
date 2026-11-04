# Proposal 0017: the map type and BSP importer

Status: accepted, 2026-11-03, increment 2.

## Research

- [The pre-research on maps](../pre-research/maps.md) parsed all 210 surveyed maps with `qbsp` 0.17.0, including BSP2 and `.lit`, and chose to load BSP directly through an importer.
- `qbsp` gives raw lumps through `BspData`: faces, texinfo, nodes, leaves, marksurfaces, visibility, lighting and mip textures. Its default `meshing` feature adds a texture packer, which isn't needed, so it stays off.
- From `gl_model.c`, `gl_rsurf.c` and `gl_warp.c`:
  - A face's lightmap size comes from its texture-space extents, in 16-unit steps. Each extra style follows the first in the lighting lump.
  - PVS data is run-length encoded, with zero bytes followed by a count. Bit `n` is leaf `n + 1`, and leaf 0 is the solid outside.
  - Texture names choose behaviour: `sky`, `*`, `+`, and `{` in later tools.
- `world.qc` sets the light style strings. A style letter maps to a brightness, with `m` as normal.
- `assets_manager` adds caching and typed loading. A map is one file loaded once, with one palette beside it, so there's nothing to cache yet. Review 0003 expected it in this increment, but it now waits for models and sounds, along with file ids.

## Design

`engine_map` holds a `Map` that knows no file format:

- Textures as RGBA mip chains with a `TextureKind`: solid, sky, liquid, cutout or hidden.
- Faces as point ranges with texture axes and optional lighting.
- RGB light samples, for any lightmap source.
- A BSP tree with leaf contents, and visibility queries: `leaf_at` and `mark_visible`.
- Models with placement and visibility, and entities as key-value lists.

`fixture_quake::import_map` builds a `Map` from BSP bytes, an optional `.lit` and a palette. All of Quake's naming rules stay here, so the engine sees kinds, not prefixes. It also holds the standard light style table, until the QuakeC port owns it.

`qbsp` and its `glam` types stay inside the importer. `engine_core` gets a small `Vec3`.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** textures are expanded to RGBA at load, which uses more memory than palette indices, and the 8-bit pipeline was dropped already.
- **Acceptance criteria:** tests cover texture kinds, palette expansion, fullbright marking, animation chains, light styles, tree lookups and PVS decoding, with no game data. Every map in the local install imports.
- **Future extension:** a new format is a new importer. Collision reads the same `Map` in increment 3.

## Review

See [review 0005](../reviews/0005-increment-2.md).
