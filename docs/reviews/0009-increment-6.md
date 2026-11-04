# Review 0009: increment 6

Status: review record, 2026-11-04. Reviewed: proposals 0032 to 0036, and the changes to every crate but `engine_console`, `engine_platform`, `engine_assets` and `engine_ui`.

| # | Finding | Handling |
|---|---|---|
| 1 | Luau cached globals a chunk doesn't assign, so scripts never saw later changes. `world` was nil everywhere but its own script, so `worldtype` read as medieval on every map, since increment 4. Base and metal maps would have had the wrong door sounds and key names | Scripts compile at optimization level 0, and a test covers it |
| 2 | `__newindex` only fires for absent keys, so setting `model` or `owner` a second time wasn't seen | They live in hidden fields, so every write reaches the engine |
| 3 | Map text reached engine fields, so a key like `size` or `think` could overwrite one with a string | Keys are converted by type, and functions and other engine fields are skipped |
| 4 | `engine_script` may not depend on `engine_physics`, but traces need collision | The simulation holds the world and offers traces |
| 5 | Item boxes are small BSP files, not models | Each texture becomes a one-pose model, drawn like any other |
| 6 | `R_RocketTrail` moves 1 unit for each 3 it counts, so trails bunch up and change with the frame rate. At our frame rates they'd have been three times as dense | Trails carry their distance, with a test from 20 to 2000 steps |
| 7 | The original's 2048 particles run out after two rockets | 16384 |
| 8 | The player has no model on the server, so it was never among the entities, and neither was its muzzle flash | Snapshots carry its entity and effects, and a test checks a flash lasts one snapshot |
| 9 | The first capture of lightning showed the muzzle light's edge as a hard circle. The original's is blurred by its 16 unit samples | The edge fades over 16 units |
| 10 | Firing everything on every map found laser traps on `e2m1` and `e4m1` calling `LaunchLaser`, which lives in `enforcer.qc` | Ported with the weapons, along with `Laser_Touch` |
| 11 | `bubble_bob` goes on using a bubble after `bubble_split` removes it, which QuakeC allows and Luau doesn't | `bubble_split` says whether it's gone |
| 12 | Drawing and capturing recorded the same calls twice | One `Recorder` does both |

## Verified

These ran against the Steam data, read in place.

| Check | Result |
|---|---|
| Every touch and target on all 117 maps | No script errors |
| Every map's scripts, run for a while | No script errors |
| Every class on every map | 88 classes. Those without a function are spawn points, and the monsters, `event_lightning` and `trigger_monsterjump` of increment 7 |
| Every model and sprite in the game | 79 models and 3 sprites parse |
| Every sound scripts ask for | 72 different sounds, none missing |
| `e1m1`'s used button textures | `+0basebtn` and `+abasebtn` link to each other |
| Captures in `e1m1` | Explosions with their sprite, particles and light, lightning from the gun, and shotgun puffs |
| 14 tests on a real device | Sprites, particles, lights and alternate textures among them |
| 44 logic tests | The real scripts: combat, barrels, traps, bubbles, fireballs, the view's kick and used buttons |

`cargo xtask check` passes.

## Remaining

- Nobody has played the effects yet, so how particles, lights and the view's kick feel is still to judge.
- `aim` looks straight ahead until there are monsters to aim at.
- Screen tints, the bonus flash and the ammo display wait for the status bar.
- Mission pack effects, `TE_EXPLOSION2` and `TE_BEAM`, aren't in.
