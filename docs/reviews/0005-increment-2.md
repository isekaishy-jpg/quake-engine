# Review 0005: increment 2

Status: review record, 2026-11-03. Reviewed: proposals 0016 to 0019, and the changes to `engine_core`, `engine_map`, `fixture_quake`, `engine_render`, `engine_client`, `engine_platform` and `engine_app`.

| # | Finding | Handling |
|---|---|---|
| 1 | The picture was upside down. `naga` flips y for Vulkan by default, and the camera matrix flipped it too. The human caught it, after screenshots had been misread as correct | The matrix now outputs WGSL's y-up space. A GPU test checks which half of the picture shows what's above the view, and it fails when the flip is put back |
| 2 | `r1m1` from Dissolution of Eternity failed to import, because `qbsp` reads stray bytes after the last lump as a broken BSPX directory | `without_stray_tail` removes them, with tests |
| 3 | The engine crashed on exit. Fields dropped in declaration order, so the command pool went before its buffer | Reordered, with the rule noted on the struct |
| 4 | `assets_manager` and file ids were expected in this increment | A map and a palette give a cache nothing to do. Moved to when models and sounds arrive, recorded in proposal 0017 |
| 5 | Reduced mode draws nothing yet, because a minimized window has no surface | Left open, as below |

## Verified

These ran against the Steam data, read in place.

| Check | Result |
|---|---|
| Import of every map in `id1`, `hipnotic`, `rogue` and the re-release folders | All pass |
| `e1m1`, `start`, `e1m3`, `e1m7`, `e2m1` | Textures, lightmaps, liquids, lava and sky draw correctly in captures |
| `hub` from `mg1` | BSP2 with 464,277 triangles draws with coloured light, 146,685 visible at the start |
| PVS on `e1m1` | 1,636 of 15,322 triangles at the start |
| The real window | A screen grab matches the capture |
| Resize, minimize, restore | No crash, and frames resume at the new size |
| Flying | `W` for 1 s moved exactly 400 units along the view, `D` moved right and space moved up, with keys sent to the window |
| Exit | Code 0 |
| GPU tests | 6 pass on a GTX 1070, and they skip with no Vulkan device |

`cargo xtask check` passes.

## Remaining

- Mouse look on a real map hasn't been tested with a real mouse. Raw motion reached the view in review 0004, and it needs window focus, so it's for the human to try.
- Entities aren't drawn, and doors and lifts sit at rest. Light styles start as on, so lights that should start off are lit.
- The picture may be darker than people remember, because it follows the software renderer's sums with no gamma setting. `gamma` is unknown to the console.
- Rendering in reduced mode, and getting it to the preview, is still an [open question](../research/open-questions.md).
- Debug builds ran at around 160 frames a second. Release speed wasn't measured.
- Increment 3 is next: collision, movement, and client and server over loopback.
