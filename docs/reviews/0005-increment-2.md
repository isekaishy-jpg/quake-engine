# Review 0005: increment 2

Status: review record, 2026-11-03. Reviewed: proposals 0016 to 0021, and the changes to `engine_core`, `engine_map`, `fixture_quake`, `engine_render`, `engine_client`, `engine_platform` and `engine_app`.

| # | Finding | Handling |
|---|---|---|
| 1 | The picture was upside down. `naga` flips y for Vulkan by default, and the camera matrix flipped it too. The human caught it, after screenshots had been misread as correct | The matrix now outputs WGSL's y-up space. A GPU test checks which half of the picture shows what's above the view, and it fails when the flip is put back |
| 2 | `r1m1` from Dissolution of Eternity failed to import, because `qbsp` reads stray bytes after the last lump as a broken BSPX directory | `without_stray_tail` removes them, with tests |
| 3 | The engine crashed on exit. Fields dropped in declaration order, so the command pool went before its buffer | Reordered, with the rule noted on the struct |
| 4 | `assets_manager` and file ids were expected in this increment | A map and a palette give a cache nothing to do. Moved to when models and sounds arrive, recorded in proposal 0017 |
| 5 | Moving looked uneven, which the human noticed while strafing. Frames read a fraction that only changed when ticks ran, so the camera stepped at 72 Hz, and 65% of frames showed no movement. On the 144 Hz display each position showed for two refreshes | Frames now work out their own fraction, with tests, and the measurement was repeated |
| 6 | Reduced mode draws nothing yet, because a minimized window has no surface | Left open, as below |

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
| Borderless default | The client area fills the 2560 by 1440 monitor with no caption, and a screen grab shows the map at full size |
| `-window` | A captioned 1280 by 720 window |
| Hands-on test by the human | Flying and mouse look worked with no glaring issues, and the Alt-Tab and taskbar previews worked, including while minimized |
| FPS counter | Shows top right in a capture. Over four rounds of 4 s it ran 203.8 frames a second on and 203.3 off, so its cost doesn't show |
| Camera motion while moving | Frames with no movement went from 65% to 0%, and speed per frame went from 400 with a deviation of 548 to 400 with none |
| `vid_vsync` | 144 frames a second with it on, 205 with it off, and it switches while running |
| Exit | Code 0 |
| GPU tests | 6 pass on a GTX 1070, and they skip with no Vulkan device |

`cargo xtask check` passes.

## Remaining

- Entities aren't drawn, and doors and lifts sit at rest. Light styles start as on, so lights that should start off are lit.
- The picture may be darker than people remember, because it follows the software renderer's sums with no gamma setting. `gamma` is unknown to the console.
- Reduced mode still draws nothing. The previews work without it, so it stays as it is unless a preview falls short.
- Exit was slow three times, over 8 s twice and 5.2 s once, all soon after builds. Five later runs took 150 to 470 ms, and shutdown stages measured 105 ms. The cause isn't known.
- Motion is settled for now, by the human's decision. Strafing and mouse look are substantially better. What remains may be the panel or texture filtering, and a filtering setting is later rendering work.
- Vsync stays off by default, by the human's decision, because it adds input lag.
- Debug builds ran at around 160 frames a second. Release speed wasn't measured.
- Increment 3 is next: collision, movement, and client and server over loopback.
