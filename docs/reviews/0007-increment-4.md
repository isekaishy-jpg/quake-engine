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
| 9 | Running every map had nobody playing, so no trigger on a real map had fired. A new test walks into every brush model and fires every target on all 117 maps | The test stays. Findings 10 and 11 came from it |
| 10 | `end` targets `misc_teleporttrain`, which wasn't ported, and `teleport_touch` read a `mangle` only destinations have | The class is ported without its model, and an unset `mangle` is zero |
| 11 | On `hip3m4` a secret door linked to a door, as both are class `door`, and had no `pos2`. An unset vector is zero in QuakeC but nil in Luau, which stopped the chain partway | `SUB_CalcMove` takes nil as zero |
| 12 | `SV_LinkEdict` grows both boxes, but only the player's grew, so every trigger reached 1 unit short | Both grow, with a test on a door's reach |
| 13 | `door_touch` failed on a door touched before it linked | It waits for an owner, as `door_use` does |
| 14 | The client blended positions by list order, so a model replacing another in one tick was drawn between them | It only blends a model with itself, with a test |
| 15 | In play, a moving plat vibrated. The plat is drawn between ticks, but the player only moved with it on them, and the stair smoother turned that into a sawtooth on a rising plat | The view follows the ground model as drawn. The view's wobble against a plat fell from 0.65 units a frame rising and 1.39 falling to under 0.001, with a test |

A door felt less responsive in play, especially closing, so `e1m1` was measured against `doors.qc`. Reach is now 78 units past the door's face, a door starts within a tick, and it holds while the player stays in its field. It starts closing 2 to 3 s after the player leaves, because touches only renew the wait once a second. A move under 0.1 s pauses and then snaps, which is `SUB_CalcMove`'s own rule. All of this matches the original apart from finding 12, and door sounds may change how it feels.

## Verified

These ran against the Steam data, read in place.

| Check | Result |
|---|---|
| Scripts on all 117 maps in `id1`, `hipnotic`, `rogue` and `mg1`, 3 s each, with nobody playing | No errors |
| Traces against a port of Quake's hull check | No disagreement on 5 maps |
| A wandering player | Never in solid, on 7 maps |
| BSP2 maps | `hub` and `mge1m1` build solids and can be walked |
| Every touch and target on all 117 maps | No errors. Teleports aimed at an `info_null` are skipped, as touching one stops the original game too |
| `e1m1` door | Opens when approached, in captures |
| `e1m1` exit | Walking into it loads `e1m2` |
| `e1m1` sky and pillar lights | Drawn colours match the pak's textures, and by eye the original game: the sky is blue, and the pillars' amber is washed out by their white |
| 13 logic tests on the real scripts | Doors, reach, blocking, plats, buttons, teleports, level changes, messages, walls and skill flags |
| 13 script host tests | Sandbox, runaway loop, script errors and fields |
| GPU tests | 8 pass, with a brush model drawn at an offset |

`cargo xtask check` passes.

## Remaining

- Played by hand starting from `e1m1`, and it felt generally good. Which levels and features that covered isn't recorded.
- Key doors stay shut until items exist, and shot buttons and doors work by touch or not at all until combat exists. Some levels can't be finished without `noclip`.
- Sounds are stubs, and damage does nothing.
- Secret counters and messages print to the terminal.
- A used button keeps its lit texture, as the alternate frame, such as `+abasebtn`, isn't shown. This was also seen in play.
- Mission packs run the original game's logic for now, so their own classes do nothing. Running without errors doesn't show they play correctly.
- Luau formatting isn't enforced by a tool.
- A teleport doesn't hold back the player's backward movement for `teleport_time`.
- `multi_touch` checks the view's pitch, where the original uses the model's, a third of it and reversed. This only matters for a trigger facing straight up or down.
