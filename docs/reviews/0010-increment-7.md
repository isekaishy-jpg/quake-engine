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
| 13 | Asked to check how weapons display. Models were drawn with no culling, where the original culls. A weapon is close enough for the near plane to open it, so its inside showed, as a jagged plate and a red slash across the nailgun | Models cull their backs, with a test, and the nailgun's artifacts are gone in captures |
| 14 | Measuring the item boxes before trusting culling on them found they wind correctly, but every normal pointed inwards, so they've been lit from the wrong side since increment 6 | Fixed, and all six boxes measured have every triangle wound and facing outwards |
| 15 | The gun was lit at the player's body, where `R_DrawViewModel` uses the gun's own position | It's lit where it is |
| 16 | The Ring of Shadows left the gun on screen, where the original hides it | The script hides it, with a test |

## Verified

These ran against the Steam data, read in place.

| Check | Result |
|---|---|
| Every monster on every `id1` map woken, hurt and killed, half of them gibbed | 1888 monsters, no script errors |
| Every touch and target on all 117 maps | No script errors |
| Every class on every map | All 88 have a function |
| Every sound scripts ask for | None missing |
| A soldier in `e1m1`, captured over a few seconds | It sees the player and comes on, and dies to the shotgun, and its backpack can be picked up |
| Every shootable button and door in `id1` | 73 can be hit by a line and answer. One waits behind a door, and `e4m5` has a door that's only a switch, with no room to move |
| A real shot at the switch on `e1m2`, and pressing a button on `e1m1`, captured | Both change to their used textures |
| All eight weapons, captured one at a time | Each shows its own model |
| 60 logic tests | Sight, walls, attacks, deaths, gibs, backpacks, paths, plats, falling, teleports, aiming, every weapon on a shot switch, and blast damage on one |

`cargo xtask check` passes.

## Remaining

- Nobody has played this yet, so how monsters feel is still to judge.
- Most monsters have only been run by tests. Only the soldier and a dog have been looked at.
- Weapons were checked against the source and the data, not side by side with the original.
- Obituaries, the intermission, and the end of the game's text screen wait for increment 8. `finale` prints for now, and killing Shub-Niggurath moves the player but can't yet hold them still.
- Mission pack monsters aren't in.
