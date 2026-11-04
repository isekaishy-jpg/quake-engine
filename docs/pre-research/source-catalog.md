# Quake source catalog

> **Pre-research.** This records decisions and early findings from before the [workflow](../../DESIGN.md#workflow) was adopted. It's an input to research, not research, and it mixes agreed decisions with suggestions. What the human has agreed is in [DESIGN.md](../../DESIGN.md).

This is a file-level inventory of id Software's original Quake source, written to plan a rewrite in Rust. It covers what the code contains, how the parts fit together, and which parts depend on C.

See also [baseline.md](baseline.md) for which version we work from, [feature-catalog.md](feature-catalog.md) for what the engine does, and [reference-tables.md](reference-tables.md) for full command and cvar lists.

The source is pinned as a submodule at [`reference/quake`](../../reference/quake), commit `bf4ac42` of [id-Software/Quake](https://github.com/id-Software/Quake). Paths below are relative to `reference/quake/WinQuake` unless they say otherwise. Line counts are raw `\n` counts, so they include comments and blanks.

## What's in the release

| Tree | Files | Contents |
|---|---:|---|
| `WinQuake/` | 295 | The original engine, version 1.09: WinQuake (software) and GLQuake (OpenGL), plus DOS, Linux and Solaris ports |
| `QW/` | 308 | QuakeWorld: a separate client and server, forked from the same engine and rebuilt for internet play |
| `qw-qc/` | 21 | QuakeC game code for QuakeWorld |

**The single-player game code is not here.** `WinQuake/` has no QuakeC source and no `progs.dat`, so monsters, weapons, items and triggers aren't in this tree. `qw-qc/` has no monsters and won't load on this engine. The GPL single-player source is v1.01 in id's Quake-Tools repo, covered in [baseline.md](baseline.md#game-code).

This catalog treats `WinQuake/` as the primary target and covers `QW/` as a set of differences from it.

## Size by subsystem

`WinQuake/` has 216 `.c`, `.h` and `.s` files and 108,088 lines. The tables also count `winquake.rc`, `progdefs.q1` and `progdefs.q2`.

| Group | Lines | Share | Meaning for the rewrite |
|---|---:|---:|---|
| Core | 35,468 | 33% | Engine logic to carry over |
| Renderer | 29,259 | 27% | Pick one approach, or replace both |
| Platform | 15,399 | 14% | Replace with the target language's libraries |
| Legacy | 28,394 | 26% | Drop |

The groups are a first proposal, not a decision. Only about a third of the code is core engine logic, and about 40% is platform code or legacy that a rewrite replaces or drops.

| Subsystem | Group | Files | Lines |
|---|---|---:|---:|
| Host and main loop | Core | 3 | 3,218 |
| Foundation | Core | 15 | 5,305 |
| Server | Core | 7 | 5,170 |
| QuakeC VM | Core | 8 | 4,347 |
| Client | Core | 9 | 4,544 |
| UI and 2D | Core | 8 | 6,218 |
| Shared render data | Renderer | 8 | 1,829 |
| Software renderer: scene (`r_`) | Renderer | 21 | 11,719 |
| Software renderer: rasterizer (`d_`) | Renderer | 17 | 5,180 |
| OpenGL renderer | Renderer | 15 | 10,531 |
| Sound | Core | 4 | 1,933 |
| Network: core and UDP | Core | 16 | 4,733 |
| Platform backends | Platform | 26 | 15,399 |
| Legacy platforms (DOS, Solaris, NeXT) | Legacy | 20 | 11,042 |
| Legacy network transports | Legacy | 16 | 6,951 |
| x86 assembly | Legacy | 26 | 10,401 |

## How the engine works

**One process, client and server.** Even single-player runs a full server and a full client, which talk through the in-process loopback driver (`net_loop.c`). The client never touches server state directly.

**Game logic is bytecode.** The engine has no idea what a shotgun or an ogre is. It loads `progs.dat`, runs QuakeC through the VM (`pr_exec.c`), and exposes itself through builtins (`pr_cmds.c`). The server calls into QuakeC for spawning, thinking, touching and player events.

**Frame order** (`_Host_Frame` in `host.c`):

1. `Host_FilterTime` decides the frame time, or skips the frame if it would exceed 72 fps.
2. Read key events and input, then run buffered console commands.
3. Poll the network. If a local server is active, send the client's move now.
4. Run `Host_ServerFrame`: read client messages, run physics and QuakeC, send updates.
5. If the server is remote, send the client's move now.
6. Read server messages with `CL_ReadFromServer`.
7. Draw with `SCR_UpdateScreen`, then update sound and CD audio.

**The timestep is variable.** Physics advances by `host_frametime`, clamped to 0.001–0.1 seconds. There is no fixed tick, so movement behaviour varies slightly with frame rate.

**Key movement defaults:** `sv_gravity` 800, `sv_friction` 4, `sv_stopspeed` 100, `sv_maxspeed` 320, `sv_accelerate` 10, `sv_maxvelocity` 2000.

**Entity movement** is selected per entity: 11 live `MOVETYPE_*` values (`NONE`, `ANGLENOCLIP`, `ANGLECLIP`, `WALK`, `STEP`, `FLY`, `TOSS`, `PUSH`, `NOCLIP`, `FLYMISSILE`, `BOUNCE`) and 5 `SOLID_*` values (`NOT`, `TRIGGER`, `BBOX`, `SLIDEBOX`, `BSP`). `BOUNCEMISSILE` and `FOLLOW` also appear in the code, but only under the disabled `QUAKE2` flag.

**Collision** runs against precomputed BSP clipping hulls. The format allows 4 hulls per model, and entity collision is limited to sizes that match a hull.

**Console.** Almost everything is driven by commands and cvars: 148 commands and 211 cvars, counted across all platform backends.

## File formats

| Format | Identifier | Defined in | Notes |
|---|---|---|---|
| PAK archive | `PACK` | `common.c` | Header gives directory offset and length. Entries are a 56-byte name, position and length. Up to 2048 files per pak |
| WAD2 | `WAD2` | `wad.h` | Used for `gfx.wad`. Lump types include palette, qpic, sound and miptex |
| BSP map | version 29 | `bspfile.h` | 15 lumps: entities, planes, textures, vertices, visibility, nodes, texinfo, faces, lighting, clipnodes, leafs, marksurfaces, edges, surfedges, models |
| MDL model | `IDPO` v6 | `modelgen.h` | Vertex-animated, with compressed vertices and normals indexed into `anorms.h` |
| SPR sprite | `IDSP` v1 | `spritegn.h` | Billboard sprites |
| QuakeC program | version 6 | `pr_comp.h` | Rejected unless its CRC equals `PROGHEADER_CRC` (5927) |
| Sound | RIFF WAVE | `snd_mem.c` | Reads `cue ` chunks to find loop points |
| Demo | none | `cl_demo.c` | A text line with the CD track, then repeating records: 4-byte length, three float view angles, raw server message |
| Save game | version 5 | `host_cmd.c` | Text file |
| Config | text | `cmd.c` | Console commands, usually `config.cfg` and `autoexec.cfg` |

All multi-byte values are little-endian.

## Network protocol

| Property | Value |
|---|---|
| Protocol version | 15 |
| Default port | UDP 26000 |
| Max reliable message | 8000 bytes (`MAX_MSGLEN`) |
| Max unreliable datagram | 1024 bytes (`MAX_DATAGRAM`) |
| Server to client messages | 34 `svc_*` types (`protocol.h`) |
| Client to server messages | 5 `clc_*` types |
| Temporary entity types | 14 live `TE_*` values |
| Signon stages | 4 |
| Limits | 600 edicts, 256 models, 256 sounds, 16 players, 64 light styles |

Packets carry flags for data, ack, nak, end-of-message, unreliable and control (`NETFLAG_*` in `net.h`). Entity updates are bit-masked deltas against a per-entity baseline (`U_*`), and player state uses a separate mask (`SU_*`).

## QuakeC VM contract

The engine is moving game logic to Luau, so this isn't a target any more. It's kept as a description of the original. Running existing `progs.dat` files would need all of this preserved exactly:

- **Opcodes:** 66 instructions, listed in `pr_comp.h`.
- **Types:** void, string, float, vector, entity, field, function, pointer.
- **Calling convention:** return value at global offset 1, and parameters at offsets 4, 7, 10 and so on up to 25. Each slot is 3 wide so it can hold a vector. Reserved space ends at offset 28.
- **Layout:** 131 globals and entity fields in `progdefs.q1`, in that exact order.
- **Builtins:** 79 numbered slots in `pr_builtin[]`. 65 are live and 14 are `PF_Fixme` placeholders. The numbers are the ABI, so they can't be renumbered.

## What won't translate directly

These are the places where the code relies on C itself, so a mechanical translation won't work. With QuakeC dropped, the string and entity memory rows no longer apply.

| Issue | Where | Why it matters |
|---|---|---|
| Strings stored as pointer differences | `pr_cmds.c:253`, `pr_cmds.c:934` | Engine strings are stored as `ptr - pr_strings` in 32-bit slots. This needs a real string table, and it also breaks on 64-bit C |
| Entities addressed by byte offset | `progs.h:94-97` | `EDICT_TO_PROG` and `NEXT_EDICT` do byte arithmetic, because edict size is set at load time by `progs.dat` |
| Entity fields are untyped memory | `pr_exec.c`, `progs.h` | QuakeC reads the same 32-bit slots as float, int, or offset. Fields should live in a flat word array, not a typed struct |
| Non-local error handling | `host.c:111`, `host.c:149`, `host.c:640` | `Host_Error` longjmps out of anything, mid-frame. This has to become exceptions or error returns |
| Custom memory lifetimes | `zone.c` | Hunk memory is freed by rewinding to a mark at map change, and cache entries can vanish under low memory. This maps to per-level arenas and asset handles |
| Files read by casting | `model.c`, `gl_model.c`, `wad.c` | Loaders cast raw bytes onto structs and byte-swap in place. These need explicit parsers |
| Global state everywhere | `cl`, `cls`, `sv`, `svs`, `host_client` | Most functions read and write globals, with ordering assumptions that aren't written down |
| Two parallel model loaders | `model.c`, `gl_model.c` | Same formats, different in-memory structures. Only one should survive |
| Float precision | `Sys_HighFPPrecision` and related, 15 call sites | Physics uses 32-bit floats. Using 64-bit changes results slightly, so stick to 32-bit for matching behaviour |
| Self-modifying code | `r_main.c:234`, `d_modech.c:45` | Only serves the assembly paths, so it disappears with them |

The assembly can be dropped safely. `id386` in `quakedef.h` switches to C equivalents for everything, and Carmack's readme says the cost is close to half the speed of the software renderer on the hardware of the time.

## File inventory

**Build** shows which configuration of `WinQuake.dsp` compiles the file: `both`, `SW` (software only), `GL` (OpenGL only), or `none` (not in the Windows project, so it's used by another port or not at all). Headers have no build value.

### Host and main loop

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `host.c` | 958 | both | Engine init and shutdown, `_Host_Frame`, `Host_Error` longjmp abort path |
| `host_cmd.c` | 1,925 | both | Game console commands: map, changelevel, save/load, connect, god, noclip, status |
| `quakedef.h` | 335 |  | Master include; version, limits, stat and item constants, `id386` switch |

### Foundation

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `common.c` | 1,827 | both | Byte order, sizebuf, MSG read/write, token parser, argv, pak filesystem and search paths |
| `common.h` | 183 |  | Declarations for the above, `link_t` lists |
| `cmd.c` | 705 | both | Command buffer, aliases, tokenizer, dispatch, forwarding to server |
| `cmd.h` | 121 |  | Command interface |
| `cvar.c` | 224 | both | Console variables, archive and server-notify flags |
| `cvar.h` | 97 |  | Cvar interface |
| `zone.c` | 935 | both | Memory: hunk (stack), zone (heap), cache (evictable) |
| `zone.h` | 131 |  | Allocator interface |
| `mathlib.c` | 585 | both | Vector math, angle helpers, `BoxOnPlaneSide`, fixed-point helpers |
| `mathlib.h` | 89 |  | Vector types and macros |
| `crc.c` | 80 | both | CRC-16 CCITT |
| `crc.h` | 24 |  | CRC interface |
| `wad.c` | 158 | both | WAD2 reader for `gfx.wad` |
| `wad.h` | 75 |  | WAD2 structures and lump types |
| `sys.h` | 71 |  | Platform layer interface (files, time, errors) |

### Server

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `sv_main.c` | 1,200 | both | Server init, spawn, signon, per-client entity and datagram building |
| `sv_phys.c` | 1,617 | both | Per-tick entity physics for each MOVETYPE, pushers, stepping, bouncing |
| `sv_user.c` | 629 | both | Player movement (friction, accel, water), reads client messages |
| `sv_move.c` | 427 | both | Monster walking, step checks, chase navigation |
| `world.c` | 962 | both | Area-node entity linking, hull traces, `SV_Move`, point contents |
| `world.h` | 78 |  | Trace and move interface |
| `server.h` | 257 |  | `server_t`, `client_t`, MOVETYPE, SOLID and damage constants |

### QuakeC VM

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `pr_exec.c` | 668 | both | Bytecode interpreter, call stack, profiler, runtime errors |
| `pr_edict.c` | 1,106 | both | Edict allocation, `progs.dat` load, entity-lump parser, save/load of edicts |
| `pr_cmds.c` | 1,934 | both | Builtin table: the engine API that game code calls |
| `pr_comp.h` | 180 |  | Opcodes and on-disk `progs.dat` structures |
| `progs.h` | 134 |  | Edict struct, VM globals, access macros |
| `progdefs.h` | 24 |  | Selects `progdefs.q1` or `.q2` |
| `progdefs.q1` | 143 |  | Generated layout of globals and entity fields, plus `PROGHEADER_CRC` |
| `progdefs.q2` | 158 |  | Same, for the unreleased `QUAKE2` define |

### Client

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `cl_main.c` | 757 | both | Connection state, entity relink and interpolation, dynamic lights |
| `cl_parse.c` | 963 | both | Parses all `svc_*` server messages |
| `cl_input.c` | 448 | both | `+action` button state, builds and sends `clc_move` |
| `cl_demo.c` | 367 | both | Demo record, playback, timedemo |
| `cl_tent.c` | 394 | both | Temporary entities: beams, explosions, impact effects |
| `chase.c` | 92 | both | Chase camera |
| `view.c` | 1,113 | both | View bob and roll, damage kicks, palette shifts, builds `refdef` |
| `view.h` | 35 |  | View interface |
| `client.h` | 375 |  | `client_state_t`, `client_static_t`, dlights, beams, efrags |

### UI and 2D

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `menu.c` | 3,231 | both | All menus, including save, multiplayer, options and video |
| `menu.h` | 38 |  | Menu interface |
| `sbar.c` | 1,323 | both | Status bar, scoreboard, intermission |
| `sbar.h` | 39 |  | Status bar interface |
| `console.c` | 649 | both | Console text buffer, notify lines, drawing |
| `console.h` | 46 |  | Console interface |
| `keys.c` | 759 | both | Key bindings, console line editing, key event routing |
| `keys.h` | 133 |  | Key codes and destinations |

### Shared render data

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `bspfile.h` | 324 |  | BSP29 on-disk structures, lumps, map limits |
| `modelgen.h` | 134 |  | MDL on-disk structures |
| `spritegn.h` | 110 |  | SPR on-disk structures |
| `render.h` | 158 |  | Refresh interface: `entity_t`, `refdef_t`, `R_*` entry points |
| `vid.h` | 85 |  | Video interface, `viddef_t` |
| `r_part.c` | 800 | both | Particles; the one `r_` file built into both renderers |
| `anorms.h` | 181 |  | 162 precomputed vertex normals for MDL |
| `anorm_dots.h` | 37 |  | Precomputed shading dot products for MDL |

### Software renderer: scene (`r_`)

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `model.c` | 1,874 | SW | Model loader, software build: BSP, MDL, SPR |
| `model.h` | 382 |  | In-memory model structures, software build |
| `screen.c` | 991 | SW | Screen update, loading plaque, screenshots, software build |
| `screen.h` | 57 |  | Screen interface |
| `draw.c` | 902 | SW | 2D pictures and characters, software build |
| `draw.h` | 40 |  | 2D drawing interface |
| `r_main.c` | 1,085 | SW | Frame setup, PVS marking, view frustum, entity loop |
| `r_bsp.c` | 674 | SW | BSP traversal, submodel clipping |
| `r_edge.c` | 774 | SW | Active edge list: scans edges into spans (zero overdraw) |
| `r_draw.c` | 908 | SW | Clips and emits edges for world polygons |
| `r_surf.c` | 678 | SW | Surface cache: blends lightmaps into textures |
| `r_alias.c` | 753 | SW | MDL setup, transform, lighting |
| `r_aclip.c` | 350 | SW | MDL triangle clipping |
| `r_sprite.c` | 401 | SW | Sprite setup and clipping |
| `r_sky.c` | 280 | SW | Sky scrolling |
| `r_light.c` | 260 | SW | Dynamic light marking, light sampling |
| `r_efrag.c` | 276 | SW | Links static entities into BSP leafs |
| `r_misc.c` | 523 | SW | Viewport setup, timing, debug graphs |
| `r_vars.c` | 39 | SW | Refresh globals (C build only) |
| `r_local.h` | 315 |  | Private renderer declarations |
| `r_shared.h` | 157 |  | Shared with `d_` layer |

### Software renderer: rasterizer (`d_`)

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `d_init.c` | 173 | SW | Rasterizer init, mipmap thresholds |
| `d_edge.c` | 331 | SW | Draws span lists |
| `d_scan.c` | 449 | SW | Textured span and turbulent water drawing |
| `d_polyse.c` | 1,111 | SW | MDL triangle rasterizer |
| `d_sprite.c` | 442 | SW | Sprite spans |
| `d_part.c` | 207 | SW | Particle drawing |
| `d_sky.c` | 138 | SW | Sky spans |
| `d_surf.c` | 335 | SW | Surface cache allocator |
| `d_modech.c` | 107 | SW | Reacts to video mode changes |
| `d_fill.c` | 88 | SW | Solid rect fill |
| `d_zpoint.c` | 47 | SW | Z-buffer point plot |
| `d_vars.c` | 50 | SW | Rasterizer globals (C build only) |
| `d_iface.h` | 229 |  | Interface between `r_` and `d_` |
| `d_local.h` | 111 |  | Private rasterizer declarations |
| `adivtab.h` | 1,077 |  | Integer division lookup table |
| `block8.h` | 143 |  | 8-bit surface block macros |
| `block16.h` | 142 |  | 16-bit surface block macros |

### OpenGL renderer

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `gl_rmain.c` | 1,159 | GL | Frame setup, entities, MDL drawing |
| `gl_rsurf.c` | 1,694 | GL | World surfaces, lightmap upload, texture chains |
| `gl_model.c` | 1,840 | GL | Model loader, GL build (near-copy of `model.c`) |
| `gl_model.h` | 430 |  | In-memory model structures, GL build |
| `gl_mesh.c` | 360 | GL | Builds MDL triangle strips and fans, with a disk cache |
| `gl_draw.c` | 1,297 | GL | 2D drawing, texture upload and resampling |
| `gl_screen.c` | 923 | GL | Screen update, GL build |
| `gl_warp.c` | 1,092 | GL | Water warp, sky, surface subdivision |
| `gl_warp_sin.h` | 51 |  | Warp sine table |
| `gl_rlight.c` | 354 | GL | Dynamic lights, light sampling |
| `gl_refrag.c` | 234 | GL | Static entity linking (near-copy of `r_efrag.c`) |
| `gl_rmisc.c` | 455 | GL | Renderer init, player skin translation, `timerefresh` |
| `gl_test.c` | 182 | GL | Debug test code, empty unless `GLTEST` is set |
| `glquake.h` | 251 |  | GL declarations |
| `glquake2.h` | 209 |  | `QUAKE2` variant of `glquake.h` |

### Sound

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `snd_dma.c` | 1,017 | both | Channels, spatialization, ambient sounds, precache |
| `snd_mem.c` | 341 | both | WAV parse, resample to output rate |
| `snd_mix.c` | 398 | both | Paint buffer mixer |
| `sound.h` | 177 |  | Sound interface and `dma_t` |

### Network: core and UDP

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `net_main.c` | 997 | both | Socket abstraction, driver dispatch, host cache |
| `net_dgrm.c` | 1,390 | both | Reliable datagram protocol, connection and server query control messages |
| `net_loop.c` | 245 | both | In-process loopback for local play |
| `net_vcr.c` | 167 | both | Records and replays network traffic for debugging |
| `protocol.h` | 167 |  | Game protocol: version, `svc_*`, `clc_*`, `TE_*`, update bit masks |
| `net.h` | 337 |  | Network interface, packet flags, driver tables |
| `net_dgrm.h` | 34 |  | Datagram driver interface |
| `net_loop.h` | 33 |  | Loopback interface |
| `net_vcr.h` | 37 |  | VCR interface |
| `net_win.c` | 120 | both | Driver table, Windows |
| `net_bsd.c` | 93 | none | Driver table, Unix |
| `net_none.c` | 46 | none | Driver table, loopback only |
| `net_udp.c` | 414 | none | UDP on BSD sockets |
| `net_udp.h` | 39 |  | UDP interface |
| `net_wins.c` | 575 | both | UDP on Winsock |
| `net_wins.h` | 39 |  | Winsock interface |

### Platform backends

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `sys_win.c` | 893 | both | Windows entry, timer, file I/O |
| `sys_wind.c` | 325 | none | Windows dedicated server variant |
| `sys_linux.c` | 457 | none | Linux entry, timer, file I/O |
| `sys_null.c` | 232 | none | Stub template for a new port |
| `conproc.c` | 365 | both | Windows dedicated server console bridge |
| `conproc.h` | 37 |  | Bridge interface |
| `winquake.h` | 114 |  | Shared Windows declarations |
| `winquake.rc` | 131 | both | Windows resources |
| `resource.h` | 20 |  | Resource IDs |
| `vid_win.c` | 3,343 | SW | Software video on Windows (DirectDraw, DIB, SciTech MGL) |
| `gl_vidnt.c` | 1,949 | GL | GL video on Windows |
| `gl_vidlinux.c` | 866 | none | GL video on Linux (SVGALib) |
| `gl_vidlinuxglx.c` | 997 | none | GL video on Linux (GLX) |
| `vid_x.c` | 1,199 | none | Software video on X11 |
| `vid_svgalib.c` | 1,003 | none | Software video on SVGALib |
| `vid_null.c` | 87 | none | Stub video |
| `in_win.c` | 1,239 | both | DirectInput mouse, joystick |
| `in_null.c` | 39 | none | Stub input |
| `input.h` | 34 |  | Input interface |
| `snd_win.c` | 729 | both | DirectSound and waveOut output |
| `snd_linux.c` | 269 | none | OSS output |
| `snd_null.c` | 97 | none | Stub sound |
| `cd_win.c` | 477 | both | CD audio on Windows |
| `cd_linux.c` | 416 | none | CD audio on Linux |
| `cd_null.c` | 54 | none | Stub CD audio |
| `cdaudio.h` | 27 |  | CD interface |

### Legacy platforms (DOS, Solaris, NeXT)

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `sys_dos.c` | 953 | none | DOS entry |
| `sys_sun.c` | 352 | none | Solaris entry |
| `dos_v2.c` | 257 | none | DJGPP helpers |
| `dosisms.h` | 100 |  | DOS declarations |
| `vid_dos.c` | 778 | none | DOS video manager |
| `vid_dos.h` | 83 |  | DOS video interface |
| `vid_ext.c` | 795 | none | VESA modes |
| `vid_vga.c` | 478 | none | VGA modes |
| `vgamodes.h` | 599 |  | VGA register tables |
| `vregset.c` | 81 | none | VGA register script interpreter |
| `vregset.h` | 56 |  | Its interface |
| `vid_sunx.c` | 1,257 | none | Solaris X video |
| `vid_sunxil.c` | 1,288 | none | Solaris XIL video |
| `in_dos.c` | 615 | none | DOS input |
| `in_sun.c` | 245 | none | Solaris input |
| `cd_audio.c` | 886 | none | DOS CD audio |
| `snd_dos.c` | 653 | none | DOS sound |
| `snd_gus.c` | 1,293 | none | Gravis Ultrasound |
| `snd_sun.c` | 218 | none | Solaris sound |
| `snd_next.c` | 55 | none | NeXT sound stub |

### Legacy network transports

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `net_wipx.c` | 432 | both | IPX on Winsock |
| `net_wipx.h` | 39 |  | Its interface |
| `net_ipx.c` | 706 | none | IPX on DOS |
| `net_ipx.h` | 39 |  | Its interface |
| `net_ser.c` | 951 | none | Serial and modem protocol |
| `net_ser.h` | 33 |  | Its interface |
| `net_comx.c` | 1,285 | none | DOS serial port driver |
| `net_bw.c` | 763 | none | Beame and Whiteside TCP for DOS |
| `net_bw.h` | 39 |  | Its interface |
| `net_dos.c` | 162 | none | Driver table, DOS |
| `net_mp.c` | 443 | none | Mplayer service |
| `net_mp.h` | 39 |  | Its interface |
| `mplib.c` | 222 | none | Mplayer DOS support |
| `mplpc.c` | 1,001 | none | Mplayer DOS sockets |
| `mpdosock.h` | 797 |  | Mplayer socket header |
| `net_wso.c` | 0 | none | Empty file (0 bytes) |

### x86 assembly

| File | Lines | Build | Purpose |
|---|---:|---|---|
| `d_draw.s` | 1,037 | SW | 8-bit span drawing |
| `d_draw16.s` | 974 | SW | 16-pixel span drawing |
| `d_parta.s` | 477 | SW | Particles |
| `d_polysa.s` | 1,744 | SW | MDL triangles |
| `d_scana.s` | 89 | SW | Turbulent spans |
| `d_spr8.s` | 900 | SW | Sprite spans |
| `d_varsa.s` | 213 | SW | Rasterizer globals |
| `d_copy.s` | 149 | none | Screen copy (DOS) |
| `r_aclipa.s` | 216 | SW | MDL clipping |
| `r_aliasa.s` | 237 | SW | MDL transform |
| `r_drawa.s` | 838 | SW | Edge emission |
| `r_edgea.s` | 750 | SW | Edge list |
| `r_varsa.s` | 64 | SW | Refresh globals |
| `surf8.s` | 783 | SW | 8-bit surface blocks |
| `surf16.s` | 172 | SW | 16-bit surface blocks |
| `snd_mixa.s` | 218 | both | Mixer |
| `math.s` | 418 | both | Math helpers |
| `worlda.s` | 144 | both | Hull point contents |
| `sys_wina.s` | 116 | both | FPU control, Windows |
| `sys_dosa.s` | 95 | none | FPU and timer, DOS |
| `dosasm.s` | 77 | none | DOS helpers |
| `nonintel.c` | 64 | none | C stubs for the `id386=0` build |
| `asm_draw.h` | 151 |  | Asm struct offsets |
| `asm_i386.h` | 97 |  | Asm macros |
| `d_ifacea.h` | 98 |  | Asm mirror of `d_iface.h` |
| `quakeasm.h` | 280 |  | Asm includes |

## QuakeWorld differences

`QW/` is about 96,000 lines across client and server, and much of that duplicates `WinQuake/`. What it adds:

| Area | Change | Files (`QW/client/` unless noted) |
|---|---|---|
| Split binaries | Separate client and dedicated server, with no local server in the client | `QW/server/` |
| Shared movement | Player movement moved out of the server so both sides can run it | `pmove.c`, `pmovetst.c`, `pmove.h` |
| Client prediction | The client runs movement ahead of the server to hide latency | `cl_pred.c` |
| Delta entities | Packet entities are delta-compressed, with 64 frames of history (`UPDATE_BACKUP`) and 64 entities per packet | `cl_ents.c`, `QW/server/sv_ents.c` |
| Network channel | New reliable and unreliable channel | `net_chan.c`, `QW/server/sv_nchan.c` |
| Protocol | Version 28, not compatible with version 15. Server port 27500, client 27001, master 27000. Up to 32 clients | `protocol.h` |
| Other | Spectator camera, player skins, MD4 checksums | `cl_cam.c`, `skin.c`, `md4.c` |
| Tools | `qwfwd`, a small packet forwarder | `QW/qwfwd/` |

Prediction and delta compression are what made Quake good on the internet, so they're worth considering even if the rewrite starts from `WinQuake/`.

`qw-qc/` is 10,912 lines of QuakeC. `QW/progs/` holds a second copy, and the two are the same code. They differ only by the GPL headers added to `qw-qc/` and a few one-line comments, and both contain an identical `qwprogs.dat`.

## Not engine code

| Path | What it is | Handling |
|---|---|---|
| `dxsdk/` | Microsoft DirectX headers and libraries | Third-party, don't carry over |
| `scitech/` | SciTech MGL headers and library, used by `vid_win.c` | Third-party, don't carry over |
| `kit/` | Prebuilt `GLQUAKE.EXE`, `OPENGL32.DLL`, readmes | Don't carry over |
| `cwsdpmi.exe` | DOS extender | Don't carry over |
| `gas2masm/` | Tool that converts the assembly to MASM syntax | Not needed once the assembly is gone |
| `data/` | Text only: manual, order form, license and tech info | Reference only |
| `docs/` | Install notes and readmes for GLQuake and Linux | Reference only |
| `*.dsp`, `Makefile.*`, `*.bat`, `*.spec.sh` | Visual C++ 6 projects and build scripts | Useful as a record of what each port compiled |

## License

The code is GPL version 2 or later, per the file headers and `gnu.txt`. A translation into another language is a derivative work, so plan on the new engine being GPL too.

The game data isn't covered. Carmack's readme says it stays copyrighted under its original terms, so `pak0.pak` and friends must stay out of this repo. A standalone release needs its own assets.

Seven files have no GPL header, but they ship in the same GPL release: `net_wso.c` (empty), `sys_linux.c`, `mpdosock.h`, `resource.h`, `d_copy.s`, `dosasm.s` and `sys_dosa.s`. None of them is in the Core group.

## Open decisions

The target language is Rust, and the engine is a rebuild rather than a compatible port. Still open:

1. **Assets.** How much of the original data formats to keep.
2. **Networking.** Which model to build on, now that protocol 15 isn't required.

The current list of choices is in [stack.md](stack.md).
