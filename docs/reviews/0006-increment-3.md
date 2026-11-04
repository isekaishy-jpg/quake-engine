# Review 0006: increment 3

Status: review record, 2026-11-04. Reviewed: proposals 0022 to 0024, and the changes to `engine_map`, `fixture_quake`, `engine_physics`, `engine_movement`, `engine_net`, `engine_sim`, `engine_server`, `engine_client` and `engine_app`.

| # | Finding | Handling |
|---|---|---|
| 1 | Over a long random run, 2 of 21,600 ticks ended in solid on `start`, from rounding at a surface | `trace` now backs out of solid, as the end of `SV_RecursiveHullCheck` does. It's 0 on every map tested |
| 2 | A ray ignores a hit at its very start, so one starting exactly on a wall could pass through it | The hull tree catches this and finds the surface by halving, with a test |
| 3 | The first comparison with Quake's trace reported misses over 1 unit. The margin is measured from the surface, and a shallow ray covers a long way inside it, so the test was measuring the wrong thing | The test measures distance from the surface |
| 4 | `engine_movement` used `Contents` from `engine_map`, which its crate rule forbids | `engine_physics` has its own `PointContents` |
| 5 | `engine_movement`'s tests also needed `engine_map`, which the crate-graph test refused. The commit script ran on past that failure | The tests moved to `engine_sim`, which may use both, and the check passed before the last commit was amended |

## Verified

These ran against the Steam data, read in place.

| Check | Result |
|---|---|
| 100,000 traces on `start`, `e1m1`, `e1m7`, `e4m3` and `dm3`, against a port of Quake's hull check | 0 disagree on hitting, and 0 end over 0.1 units away |
| Solids | Every solid loaded, for example 1,540 on `e1m1` and 14,557 on BSP2 `hub` |
| Random player for 5 minutes each on `start`, `e1m1`, `e1m3`, `e2m1`, `e4m3`, `dm3`, `dm6` | 0 ticks in solid, over 53,000 units travelled on each |
| Random player for 1 minute on BSP2 `hub` and `mge1m1` | 0 ticks in solid |
| Jump height and stop distance | Within 1 and 1.5 units of the pre-research's 43.7 and 63.8, in tests |
| The real engine on `e1m1` | Keys walk down the stairs from the start and noclip moves exactly 200 units in 0.5 s, through client, loopback and server |

`cargo xtask check` passes.

## Remaining

- Nobody has played this by hand yet. How movement feels, and whether the render tick shows bumps when input changes, needs a human.
- Doors, lifts and other brush models aren't solid. They arrive with entities in increment 4, and nothing else in the world moves yet.
- There's no view bob, damage, falling sound or drowning.
- The map loads for both sides directly. A map change message arrives with online play.
- A stall drops time, so holding a key through a long stall can lose the press. Screenshots in debug builds stall for over a second, which should move off the main path.
- Slow exits from review 0005 didn't appear here, and their cause is still unknown.
