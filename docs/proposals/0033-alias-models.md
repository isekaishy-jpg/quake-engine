# Proposal 0033: alias models

Status: accepted, 2026-11-04, increment 6.

## Research

- `modelgen.h` defines MDL: a header, skins, texture coordinates with a seam flag, triangles, then frames of byte positions with an index into 162 fixed normals. The original paks hold 79 models, version 6.
- Frames and skins can be groups that cycle by time. `flame.mdl` is one frame group.
- `R_AliasPreparePoints` moves a coordinate on a seam by half the skin's width for a triangle that faces back.
- `R_LightPoint` traces 2048 units down through the tree, and samples the lightmap of the first face it meets. This needs each node's faces.
- `R_DrawEntitiesOnList` uses that as both ambient and directional light. Ambient stops at 128, and the two together at 192, with light coming along -x.
- Skin indices from 224 up are fullbright, as on the world.
- A model's `flags` ask for a trail, or for it to spin, as items do at 100 degrees a second.
- `V_CalcBob` and `V_CalcRefdef` place the weapon, which draws in front of everything, and bobs with speed.
- `vulkane` can bind one buffer at two offsets, so two poses can be blended by the vertex shader.

## Design

- `engine_map::AliasModel` holds skins as RGBA, corners with their coordinates, poses, and frames as runs of poses. `fixture_quake::parse_mdl` makes one, with the normals table from `anorms.h`.
- `Map::light_at` ports `R_LightPoint`, so `Node` keeps its faces.
- `engine_render` keeps each model's poses per corner, 8 bytes each: a byte position and a signed normal. A vertex takes two poses and blends them, with the transform and light as push constants.
- `engine_app` loads a model when its name arrives. `engine_client` turns entities into draws, with frames blending over 0.1 s, since Quake's animate at 10 Hz.
- The weapon is one more draw, its depth squeezed so it stays in front.

## Proposal

- **Selected approach:** the above.
- **Direct upgrades:** blended animation in place of stepped frames.
- **Tradeoffs:** poses per corner repeat data, about 1.4 MB for the player, for the sake of a simple draw.
- **Acceptance criteria:** tests cover parsing, seams, groups and bad files, and every model in the game parses. A GPU test draws a model where it should be, lit. On a real map torches burn, items spin, and the weapon shows.
- **Future extension:** per-pixel lighting, shadows and `.lit` colour on models.

## Review

See [review 0009](../reviews/0009-increment-6.md).
