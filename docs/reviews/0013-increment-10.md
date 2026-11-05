# Review 0013: increment 10

Status: review record, 2026-11-04. Reviewed: proposals 0061 to 0065, and the changes to the scripts, `engine_sim`, `engine_movement`, `engine_net`, `engine_server`, `engine_client`, `engine_script`, `engine_app` and the tests.

The human asked whether all of the QuakeC had been ported. It hadn't, and earlier reports had made it sound as if it had. Tests that a map runs with no script errors can't show that behaviour is missing. This increment compares the port with the source.

## How it was compared

- Every function in the 31 files `progs.src` compiles, by name, against what the port defines.
- Every entity class in all 51 maps against a function in the port.
- Every string, sound, model and field in the QuakeC against the port.
- For each function with a namesake, what it calls, what it sets and the numbers in it. Everything flagged was read side by side.

This finds what's missing, and numbers that differ. It doesn't prove two functions behave the same, so a difference that uses the same names and numbers could still be there.

## Findings

| # | Finding | Handling |
|---|---|---|
| 1 | The level clock started at 0, not 1.0, so every waiting teleport fired on the first tick. On `end` that telefragged Shub and ended the game | Proposal 0063 |
| 2 | 29 triggers on 13 maps fired on touch and couldn't be shot | Proposal 0061 |
| 3 | `e1m8` had full gravity | Proposal 0062 |
| 4 | `misc_teleporttrain` had no model | It has now. Finding 1 had been removing it |
| 5 | Nobody was stopped from backing up after a teleport | Proposal 0064 |
| 6 | No bubbles from drowning, no telefrag sound, no hop when dying, and a gibbed player wasn't dead at once | Proposal 0064 |
| 7 | Powerups didn't come back in deathmatch 2 | Proposal 0064 |
| 8 | `trigger_onlyregistered` let everyone through | Proposal 0065 |
| 9 | `teamplay`, `skill`, `timelimit` and `fraglimit` only changed with the level | Proposal 0065 |
| 10 | Nail guesses from proposal 0057 never showed, since their models weren't named at the start | `worldspawn` names them |
| 11 | A test expected `trap_shooter` to wait, where the original fires at once | The test was wrong |
| 12 | A test of the dying hop failed on a near-zero first random number | It draws a few first |
| 13 | `sv_gravity` was saved to the config at first, so quitting on `e1m8` would have carried its gravity into the next game | It isn't saved, as in the original |

## Left out, on purpose

| What | Why |
|---|---|
| `T_BeamDamage`, `SpawnChunk`, `CheckSpawnPoint`, `trigger_reactivate`, `plat_outside_touch`, `SUB_CalcAngleMove`, `SUB_CalcMoveEnt` | Nothing calls them |
| `amtest.qc`, `jctest.qc`, `testplayerstart`, `DumpScore`, `PrintClientScore` | Test code, or only called from code commented out |
| `W_Precache`, `main`, `makestatic` | Nothing to do here |
| The 400 upward knock in `LightningDamage` | It needs `other` to be a player. The engine sets `other` to the world before every think, so it can't happen |
| `self.speed == 1000` in `misc_fireball` | A typo in the original that does nothing |
| The 0.8 drag in `WaterMove` | QuakeWorld's QuakeC drops it too, and movement here follows QuakeWorld's |

## Verified

| Check | Result |
|---|---|
| `cargo xtask check` | Passes |
| A waiting teleport | Leaves a monster alone until used. With the clock back at 0 the test fails |
| `end` | Shub alive and in place, teleporter on the level with its model, and it moves when used. With the clock at 0 the test fails |
| Shootable triggers on real maps | All 29 fire |
| Gravity | A jump goes 552 units on `e1m8`, 40 on `e1m1`. A client at 60 ms stays 0.00 units from the server there. A saved game keeps it |
| Real maps, bots and networks | As before |

## Not verified

- Nobody has played these levels through, so whether anything else stops a level being finished isn't known.
- The comparison can't see every kind of difference.

## Limitations

- `SV_SpawnServer`'s two settling frames aren't run.
- Mission pack QuakeC isn't in the sources here, so `hipnotic` and `rogue` have only been run, not ported.
