# Review 0014: the whole application

Status: review record, 2026-11-05. Reviewed: every engine crate, `fixture_quake`, its scripts and `xtask`. Not reviewed: `solapp`, `solworker` and `solcache`.

The human asked for two circular reviews across the application, one for bugs and one for code quality, each repeated until nothing remained, with Fable giving one second opinion on each.

## How it ran

| Pass | By | Covered |
|---|---|---|
| Bugs 1 | Claude | Changes since increment 4, by eight angles |
| Quality 1 | Claude | Shared values and duplicated constants |
| Bugs, second opinion | Fable, at `b713afe` | All the QuakeC against the Luau, line by line apart from monster frame tables, and the server, simulation, network and application code |
| Quality, second opinion | Fable, at `17c7e7d` | Everything but the Sol crates, `engine_audio`, `engine_ui`, `engine_platform`, part of `engine_render`, the file parsers, the scripts and the bodies of tests |
| Bugs and quality 2 | Claude | What both second opinions didn't reach: the file parsers, `solids.rs`, `engine_audio`, effects, particles, bots, the conditioned link, the HUD, the menus, `engine_ui`, `engine_console`, `engine_assets`, input, and the renderer's data. Effects and particles were read against what's remembered of `r_part.c` and `cl_tent.c` |
| Check of the changes | Claude | Every change here read again for what it could break, then a real host and guest run on `dm3` |

The last pass found nothing new. That's the only claim this makes: reading finds what it finds, and earlier reports here have been wrong about being finished.

## Bugs found and fixed

| # | Found by | Finding |
|---|---|---|
| 1 | Claude | A player's team was never set, so friendly fire rules couldn't apply |
| 2 | Claude | Saved rules were overwritten by the console on load |
| 3 | Claude | Saves from before `no_backing` couldn't be read |
| 4 | Claude | Renames didn't reach the scripts, and names could hold control characters |
| 5 | Claude | Held weapons looped their frames with no ammo |
| 6 | Claude | Grenades and anything else in flight didn't touch triggers |
| 7 | Claude | A launch heard before its first snapshot was forgotten |
| 8 | Claude | A host's name was never tidied. A long or accented one left guests unable to read the scores |
| 9 | Fable | Trap, Scrag, Death Knight and Chthon shots were announced before their speed was set, so clients drew them up to three times too fast |
| 10 | Fable | `SUB_CheckRefire` was called without `self`: a script error on nightmare, and no second burst |
| 11 | Fable | Firing or invisible players made pain sounds |
| 12 | Fable | A player killed while invisible stayed as eyes |
| 13 | Fable | `vectoyaw` and `vectoangles` rounded down where C cuts towards zero |
| 14 | Fable | `spawn_tdeath` was only used in deathmatch and co-op |
| 15 | Fable | Monsters standing still touched triggers every tick. `force_retouch` is ported with this |
| 16 | Fable | A facing trigger used a player's view, not their angles |
| 17 | Fable | Monsters hunted a player who'd left at the world's origin |
| 18 | Fable | Commands lasting no time ran scripts for free |
| 19 | Fable | Things that fly thought after moving, not before |
| 20 | Fable | Noclipping players didn't touch triggers |
| 21 | Claude | `qbsp` calls sky leaves solid. Only `e1m5` and `e4m7` keep any, and there rockets burst on the sky |
| 22 | Claude | Sizes in a WAD or picture could overflow |
| 23 | Claude | The menus could only type lower case, with no punctuation |
| 24 | Claude | Sky gave no tint, and no splash, where the original treats it as past water |

One second opinion finding was wrong: it said a monster's landing thud should use a fixed speed. `sv_phys.c` scales it by gravity, as this does.

## Quality found and fixed

| Area | Change |
|---|---|
| Duplicates | Tick length, player box, view height, view axes, gravity, punch recovery, short key id, buttons, fair play defaults, stats and item bits, public key size, skin and sprite upload, and four copies of the test room each have one home |
| Encapsulation | `fixture_quake::layout` holds paths, startup names, rules and `load_level`. A game's `GameRules` and `SameTeam` replace Quake's rules in the server. Numbers shared with scripts come from one table. Scripts name their number fields and player defaults. Movement's numbers are in `MoveVars` |
| Structure | `engine_app`'s `app` and the Vulkan backend are split into modules. A seat follows the fairness zone itself. `Server::kick` says who it put out with an enum |
| Speed | Traces look at brush models only. A tick builds what's shown once. Prediction replays without copying. Broadcasts encode once. Console lookups skip lower-casing. The renderer keeps one fence |
| Dead code | `Bindings::press`, the `server` cvar flag, empty dev tool and testing feature checks, and an unused bound |
| Tests | Pusher tests in `engine_sim`, the grenade spread pinned between client and script, and a GPU test that can't run says so |
| Docs | Stale lines corrected |

## Findings not acted on

| Finding | Why |
|---|---|
| Split `World` into session and presentation | It would touch every console command and buy no new boundary. The module split gave the readability |
| Merge `Contents`, `Liquid` and `PointContents` | Each is a narrower view for a crate that can't see the wider one. The conversion is in one place |
| `ED_Alloc`'s delays, the item link margin and the default `setmodel` box as settings | They're how this engine works, each named once, and nothing wants them otherwise |
| Reuse a command buffer each frame | The vendored wrapper can't reset one. Allocating from a pool is cheap, and patching the vendor for it isn't worth it |
| The small maps the client builds each frame | A few hundred entries at most |
| Public accessors only tests use | Tests live in `tests/`, as the style guide requires, so they need them |
| Wall clock in the online tests | They run real sockets |
| `effects.rs` knows Quake's names | Proposal 0036 keeps them in that one file on purpose |
| A client holding back every answer to raise its ping | It can't be told from a slow connection. Fairness's ceiling bounds it, and the doc now says so |

## Not reviewed

The Sol crates, at about 31,000 lines of source and as much again of tests, weren't read. Proposal 0009 exempts them from the workspace lints, and their own tests pass. Monster frame tables were spot checked, not compared in full. The Vulkan backend's recording and pipeline code was moved but not read line by line, and the second opinion on quality saw only part of it.

## Found in play

The human played episode 1 after these reviews, and heard what no reading had found.

| Finding | Cause | Change |
|---|---|---|
| Falling into water beside a wall could screech, with many copies of a sound | `PlayerPostThink` overwrote `jump_flag` on every move, where the original clears it on landing and only reads it in the air. Water brakes slowly, so a player stood on the bottom still moving fast, and `h2ojump.wav` restarted up to 20 times in one fall | Ported as written, with `W_WeaponFrame` back at the top. A real map test on `e1m2` fails at 20 without this |
| Lava fireballs are far too loud, and heard through walls | `SV_Physics_Toss` returns on a clear move, before `SV_CheckWaterTransition`, so what flies through a surface is silent. The port looked for water on every move, so each fireball splashed at full volume twice. Reading only the call's volume, the first answer here called this correct. The human's ears were right | The check now runs on a hit alone. A real map test pairs a tossed thing with a stepping one through the same surface, and `e1m6`'s fireballs fall silent |

## Validation

`cargo xtask check`, the real map tests (26) and the netplay tests (24) pass. A real host and guest played `dm3` over a password, with chat, a suicide reaching the scores, and screenshots from both.
