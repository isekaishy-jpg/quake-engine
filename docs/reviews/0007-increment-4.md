# Review 0007: increment 4

Status: review record, 2026-11-04. Reviewed: proposals 0025 to 0028, and the changes to `engine_core`, `engine_map`, `fixture_quake`, `engine_physics`, `engine_movement`, `engine_net`, `engine_sim`, `engine_script`, `engine_server`, `engine_client`, `engine_render` and `engine_app`.

| # | Finding | Handling |
|---|---|---|
| 1 | Running every map found `cross connected doors` on `e2m1`, `e4m5`, `e4m7` and `r1m3`. `Mod_LoadSubmodels` spreads a model's bounds by a pixel, which the importer didn't do, and door travel depends on it too | The importer spreads bounds |
| 2 | `e4m5` and `r1m3` still failed. `ED_LoadFromFile` filters entities by skill and deathmatch flags, which the port missed, so doors for other modes overlapped, and items and monsters would have spawned wrongly | `spawn_filter` in `subs.luau`, with a test |
| 3 | A shader failed to compile, yet the GPU tests passed, because they treated any renderer failure as a missing device | `RenderError::is_no_device`. Any other failure now fails the test |
| 4 | `naga` 29 calls push constants `immediate` in WGSL | The shader uses that name |
| 5 | Luau's sandbox keeps script globals out of `_G`, so spawn functions weren't found and every entity was removed | The host looks them up and passes them in |
| 6 | Two tests expected the wrong thing: a door stays open with a player in its field, and a plat starts rising at once | The tests now check the real behaviour |
| 7 | `Lua::sandbox` leaves mlua's `require` in place, and that reads files from disk, so any script could load one. The sandbox test caught it | The host removes `require` and `loadstring` before sandboxing, and the test calls `require` |
| 8 | `r2m1` failed with `changelevel trigger doesn't have map`. This was first put down to mission packs having their own QuakeC, and the test excused them. The map has it, as `"map "`: `ED_ParseEdict` trims trailing spaces from keys, and the importer didn't | The importer trims keys, with a test, and the map test no longer excuses any game |

## Verified

These ran against the Steam data, read in place.

| Check | Result |
|---|---|
| Scripts on all 117 maps in `id1`, `hipnotic`, `rogue` and `mg1`, 3 s each, with nobody playing | No errors |
| Traces against a port of Quake's hull check | No disagreement on 5 maps |
| A wandering player | Never in solid, on 7 maps |
| BSP2 maps | `hub` and `mge1m1` build solids and can be walked |
| `e1m1` door | Opens when approached, in captures |
| `e1m1` exit | Walking into it loads `e1m2` |
| 12 logic tests on the real scripts | Doors, blocking, plats, buttons, teleports, level changes, messages, walls and skill flags |
| 13 script host tests | Sandbox, runaway loop, script errors and fields |
| GPU tests | 8 pass, with a brush model drawn at an offset |

`cargo xtask check` passes.

## Remaining

- Nobody has played this by hand yet.
- Key doors stay shut until items exist, and shot buttons and doors work by touch or not at all until combat exists. Some levels can't be finished without `noclip`.
- Sounds are stubs, and damage does nothing.
- Secret counters and messages print to the terminal.
- A button's alternate texture isn't shown.
- Mission packs run the original game's logic for now, so their own classes do nothing. Running without errors doesn't show they play correctly.
- Luau formatting isn't enforced by a tool.
