# Baseline: which version we work from

> **Pre-research.** This records decisions and early findings from before the [workflow](../../DESIGN.md#workflow) was adopted. It's an input to research, not research, and it mixes agreed decisions with suggestions. What the human has agreed is in [DESIGN.md](../../DESIGN.md).

This records which code the Rust rewrite treats as its reference, and how each difference between versions and docs was resolved. The engine is a rebuild rather than a compatible port: see [stack.md](stack.md). Everything here was checked against the pinned source at `reference/quake` (commit `bf4ac42`), not just taken from the shipped docs.

## Recommended baseline

| Area | Baseline | Status |
|---|---|---|
| Engine | `WinQuake/`, version 1.09 | Recommended |
| Compile settings | `id386=0`, `QUAKE2` off, `BAN_TEST` on, everything else off (see [Compile flags](#compile-flags)) | Recommended |
| Game code | QuakeC ported to Luau, starting with map entities. `progs.dat` isn't used | Decided |
| Port source | v1.01 `qcc/v101qc` from [id-Software/Quake-Tools](https://github.com/id-Software/Quake-Tools) | Verified as matching this engine |
| Network protocol | Open. Protocol 15 is no longer required | Deferred |
| Renderer | Backend is `vulkane`, see [stack.md](stack.md). The reference to follow is still open, see [Software or OpenGL](#software-or-opengl) | Partly decided |

## Does the version matter?

For the engine, barely. The 1999 GPL release is the only one, so there are no competing engine source versions to choose from. All the variants live side by side in this one tree.

It matters in three other places:

1. **WinQuake or QuakeWorld.** These are different games at the protocol and VM level, so we have to pick one to start from.
2. **Game code.** The single-player QuakeC isn't in this repo, and its versions do differ.
3. **Software or OpenGL.** Both are the same engine version, but they look different.

## What's in the tree

| Variant | Version in code | Runs | Protocol | `progs` CRC |
|---|---|---|---|---:|
| WinQuake / Quake | `VERSION 1.09`, `WINQUAKE_VERSION 0.996` | Single-player, co-op, deathmatch | 15 | 5927 |
| GLQuake | Same tree with `GLQUAKE`, `GLQUAKE_VERSION 1.00` | Same | 15 | 5927 |
| Linux and X11 builds | `LINUX_VERSION 1.30`, `X11_VERSION 1.10` | Same | 15 | 5927 |
| QuakeWorld | `VERSION 2.40` | Internet deathmatch only | 28 | 54730 |

## WinQuake or QuakeWorld

**Use `WinQuake/`.** QuakeWorld is not a newer version of the same engine. It removed things single-player needs, and it can't run the same game code:

- `PROGHEADER_CRC` differs, so neither engine loads the other's `progs.dat`.
- QuakeWorld removes 5 VM globals (`coop`, `deathmatch`, `teamplay`, `idealpitch`, `punchangle`) and adds 2 (`lastruntime`, `newmis`).
- Its game code has no monsters, its client has no local server, and its menus are much smaller (about 1,090 non-blank lines against 2,675).
- It drops the builtin in slot 48 (`particle`) and adds slots 79–82: `logfrag`, `infokey`, `stof`, `multicast`.
- Its demos (`.qwd`) aren't compatible with `.dem`.

The two engines do share a lot. 46 files are identical apart from whitespace, mostly the software rasterizer and assembly.

WinQuake isn't simply the older code either. For example, `model.c` has a model cache fix (`FIX FOR CACHE_ALLOC ERRORS`) that QuakeWorld lacks.

**What to take from QuakeWorld later:**

| Item | Where | Why |
|---|---|---|
| Client prediction and shared movement | `QW/client/pmove.c`, `cl_pred.c` | The main improvement for internet play |
| Delta-compressed entities | `QW/client/cl_ents.c`, `QW/server/sv_ents.c` | Lower bandwidth |
| Network channel | `QW/client/net_chan.c` | Simpler than `net_dgrm.c` |
| VM string handles | `PR_GetString` and `PR_SetString`, `QW/server/pr_exec.c` | id's own fix for the string pointer problem: negative values index an engine string table. A useful model for Rust |
| Exact-match-first cvar completion | `QW/client/cvar.c` | Small usability fix |

## Game code

QuakeC is dropped and the game moves to Luau, so this table now answers which source to port from. This release doesn't include one:

| Source | License | Has monsters | Runs on this engine | Notes |
|---|---|---|---|---|
| `progs.dat` in the user's own `pak0.pak` | Commercial data | Yes | Yes by header: version 6, CRC 5927 | Version 1.06 per community sources. Can't go in this repo |
| v1.01 `qcc/v101qc` in Quake-Tools | GPL (35 of 36 files have the notice) | Yes | Fields and builtins verified | The clean, open source for reading and testing. `qcc` is in the same repo |
| `qw-qc/` in this repo | GPL | No | No, CRC 54730 | QuakeWorld only. Useful for reading, not for running here |
| [quake-rerelease-qc](https://github.com/id-Software/quake-rerelease-qc) | GPLv2 | Yes | No | Needs engine support for localization placeholders and a modern compiler. It also changes gameplay |

**Verified for v1.01:** all 131 fields in `progdefs.q1` appear in `defs.qc` in the same order, and its 65 builtin declarations map one-to-one onto the 65 live slots in `pr_builtin[]`. Recomputing the CRC-16 of `progdefs.q1` gives 5927, which matches the engine's check. This shows the interface matches. It hasn't been compiled or run.

**Decision:** port from v1.01. Community sources say 1.06 source was never clearly licensed, so don't use it as a code reference. A Luau port of GPL code is still GPL.

## Software or OpenGL

The two renderers share the same engine, and they differ in what's on screen. The [feature catalog](feature-catalog.md#renderer-differences) has the full list. The main ones:

- OpenGL has no fullbright texels. `gl_rmain.c:511` works around this by forcing torches to full light.
- OpenGL has no underwater screen warp (`r_waterwarp` is software only).
- OpenGL adds transparent water, shadows, mirrors and filtering.

**Recommendation:** the renderer is new work on `vulkane`, so treat GLQuake as the structural reference and software as the visual reference. GPU rendering maps onto `gl_*.c`, while the look people call "Quake" comes from the software renderer. Which look to aim for is still open.

## Compile flags

| Flag | Sites | Setting | Reason |
|---|---:|---|---|
| `QUAKE2` | 85 | **Off** | No build defines it. It's unfinished sequel work, see below |
| `id386` | 46 | **Off** | Assembly path. C equivalents exist |
| `GLQUAKE` | 18 | Renderer choice | Set by the GL project configurations |
| `BAN_TEST` | 4 | **On** | Defined at the top of `net_dgrm.c`, so it's live code: the `ban` command |
| `IDGODS` | 7 | Off | Commented out in `quakedef.h`. Back door for id's own IP range, so never port it |
| `PARANOID` | 12 | Off | Debug-only checks |
| `GLTEST` | 4 | Off | Commented out in `quakedef.h` |
| `FPS_20` | 2 | Off | Experimental timing, never defined |
| `DEBUG` | 5 | Off | Debug only |

**`QUAKE2` is the trap.** The code reads as if these features exist, but none of them is in any shipped build:

- Rotating pushers (`SV_PushRotate`) and water currents
- `MOVETYPE_BOUNCEMISSILE`, `MOVETYPE_FOLLOW`
- Per-entity `gravity`, `basevelocity`, `FL_FLASHLIGHT`
- `changelevel2` and level start spots
- `TE_IMPLOSION`, `TE_RAILTRAIL`
- Builtins `sin`, `cos`, `sqrt`, `changepitch`, `TraceToss`, `etos`, `WaterMove`

## Differences resolved

Where sources disagree, the code wins unless noted.

| # | Difference | Finding | Resolution |
|---:|---|---|---|
| 1 | Two QuakeC trees, `qw-qc/` and `QW/progs/` | Identical apart from GPL headers and one-line comments. Both hold the same `qwprogs.dat` | No conflict. Use `qw-qc/` |
| 2 | QuakeWorld friction: notes say default 6 | Code says 4, and `qw2do.txt` says "friction is still 4" | 4 |
| 3 | QuakeWorld version: notes stop at 2.30 and a 2.33 test | Code says 2.40. The 2.33 notes announce 2.4 as the next release | The tree is 2.40, and no notes cover it |
| 4 | `qwchangelog.txt` stops at 2.1 | `qwrlnote.txt` runs to 2.30 | Use `qwrlnote.txt`, and treat the changelog as history |
| 5 | QuakeWorld edicts: `qw2do.txt` marks 1024 as done | `MAX_EDICTS` is 768 | 768, and only relevant to QuakeWorld |
| 6 | GLQuake version: notes say 0.99 | Code says `GLQUAKE_VERSION 1.00` | 1.00 |
| 7 | `gl_round_down` documented in four files | Doesn't exist anywhere in either engine | Documented only. Ignore it |
| 8 | README says `-safe` includes `-nonet` | `-nonet` doesn't exist. `-safe` expands to `-stdvid -nolan -nosound -nocdaudio -nojoy -nomouse -dibonly` | Code is correct |
| 9 | Linux docs say `-nokdb` | Code checks `-nokbd` | Typo in the docs |
| 10 | `data/` docs describe v0.91 to v1.01 | Engine is 1.09 | Old docs. Fine for gameplay, wrong for hardware and versions |
| 11 | `TECHINFO.TXT` says there's no 3D accelerator support | GLQuake is in the tree | Outdated |
| 12 | `model.c` and `gl_model.c` | Same formats, different in-memory structures | One loader in Rust. Keep the shared parsing and put renderer data in the renderer |
| 13 | `r_efrag.c` and `gl_refrag.c` | GL removes the software-only top-node logic | Follow the GL version unless the software renderer is ported |
| 14 | `_windowed_mouse` default | `0` in software, `1` in GL | Platform setting, so choose fresh |
| 15 | `sv_aim` default | `0.93` in WinQuake, `2` in QuakeWorld (autoaim off) | `0.93` for compatibility |
| 16 | `MOVETYPE`, `TE_*` and `EF_*` lists | Some values are `QUAKE2` only | 11 movetypes and 14 temp entities are live |

## How far to trust each doc

| Doc | Written for | Use it for |
|---|---|---|
| `WinQuake/data/MANUAL.TXT` | Original game | Game rules and player-facing behaviour |
| `WinQuake/data/TECHINFO.TXT` | DOS version, about v1.0 | Console concepts. Skip the hardware sections |
| `WinQuake/data/README.TXT` | v0.91 to v1.01 | History only |
| `WinQuake/wqreadme.txt` | WinQuake, March 1997 | Windows behaviour. Its switches all exist in the code |
| `WinQuake/glqnotes.txt` | GLQuake 0.99, Quake 1.09 | GL features and intent. Best doc for renderer differences |
| `QW/qwrlnote.txt` | QuakeWorld up to 2.30 | The reasons behind QuakeWorld changes |
| `QW/release233_notes.txt` | QuakeWorld 2.33 test | Security issues to avoid repeating |
| `QW/client/docs.txt` | Internal notes | QuakeWorld connection process |
| `QW/qw2do.txt` | Internal to-do list | Design intent, not current state |

## Not verified

- No Quake code has been compiled or run, and no game data was used.
- `qcc/v101qc` hasn't been compiled. Only its declarations were compared.
- Differences between QuakeC 1.01 and 1.06 come from community sources, not a code comparison.
- Quake-Tools is not yet in this repo. It was checked in a temporary clone at commit `c0d1b91`.
