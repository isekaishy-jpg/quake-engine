# Review 0010: increment 7

Status: review record, 2026-11-04. Reviewed: proposals 0037 and 0038, and the changes to `engine_physics`, `engine_sim`, `engine_script`, `fixture_quake` and the tests.

| # | Finding | Handling |
|---|---|---|
| 1 | `trace_inopen` and `trace_inwater` are floats in QuakeC and booleans here, so a translated `~= 0` was always true and every sight check failed. Monsters never saw anything, and a test that only looked for script errors passed, because nothing ever attacked | Fixed in the translation. Tests on the real scripts now cover seeing and attacking, and the run over every map went on to find findings 3 and 4 |
| 2 | `sight_entity` is empty when a level starts, which QuakeC reads as the world | `ent` stands in for it |
| 3 | A wizard stores `v_angle`, which only the player had | Entities hold one, and it reads as zero otherwise, which `FireBullets` needs for monsters |
| 4 | `ogre_smash12` calls `ai_charge` with no argument | It passes 0 |
| 5 | Laser traps and the enforcer share `LaunchLaser`, and `newmis` has to be set by whatever fires | Both set it |
| 6 | Pushers could only move the player, so a monster on a lift would have stayed behind | `push_move` moves everything, and puts it all back if something's stuck |
| 7 | Triggers only noticed the player, so monsters couldn't open doors, follow paths or use teleports | Triggers overlapping monsters get touches |
| 8 | A dropped backpack landing on a map with no world entity passed nil to its touch | Callbacks use `ent` |
| 9 | The ported `teleport_touch` took anything with health, and left out telefrags and clearing the ground flag | It matches the original |
| 10 | Crucified zombies can't be hurt, which looked like a failure | They never call `walkmonster_start`, as in the original, and the test allows for it |
| 11 | Telefragging Shub-Niggurath on `end` stopped with `no info_intermission`. A class with no function is removed, and the original gives spawn points empty ones so they can be found | They have them too, and the test of classes now allows no exceptions |
| 12 | `finale_1` runs as Shub-Niggurath's death, and writes to a global `other` left over from whatever ran last | It sets the player's angles and view directly |

## Verified

These ran against the Steam data, read in place.

| Check | Result |
|---|---|
| Every monster on every `id1` map woken, hurt and killed, half of them gibbed | 1888 monsters, no script errors |
| Every touch and target on all 117 maps | No script errors |
| Every class on every map | All 88 have a function |
| Every sound scripts ask for | None missing |
| A soldier in `e1m1`, captured over a few seconds | It sees the player and comes on, and dies to the shotgun, and its backpack can be picked up |
| 56 logic tests | Sight, walls, attacks, deaths, gibs, backpacks, paths, plats, falling, teleports and aiming |

`cargo xtask check` passes.

## Remaining

- Nobody has played this yet, so how monsters feel is still to judge.
- Most monsters have only been run by tests. Only the soldier has been watched.
- Obituaries, the intermission, and the end of the game's text screen wait for increment 8. `finale` prints for now, and killing Shub-Niggurath moves the player but can't yet hold them still.
- Mission pack monsters aren't in.
