# Engine systems and features

> **Pre-research.** This records decisions and early findings from before the [workflow](../../DESIGN.md#workflow) was adopted. It's an input to research, not research, and it mixes agreed decisions with suggestions. What the human has agreed is in [DESIGN.md](../../DESIGN.md).

This lists what the Quake engine actually does, by system, as opposed to [source-catalog.md](source-catalog.md), which lists the files. It describes the baseline in [baseline.md](baseline.md): `WinQuake/` 1.09, with assembly and `QUAKE2` code excluded. Paths are relative to `reference/quake/WinQuake`.

This catalog describes the original as a reference. We aren't recreating Quake, so Keep means useful to the engine, not required for parity. The engine isn't a straight port. [stack.md](stack.md) records the decisions already made: a fixed simulation tick, `box3d` physics, a `vulkane` renderer and Luau game logic. Scope labels below reflect those decisions.

The full lists of commands, cvars, switches and builtins are in [reference-tables.md](reference-tables.md).

**Scope** is a first proposal for the Rust engine:

| Scope | Meaning |
|---|---|
| Keep | Needed for the game to work and play the same |
| Later | Worth having, but not needed for a first playable version |
| Replace | The feature stays, but the implementation is new |
| Decide | Depends on a decision you haven't made yet |
| Drop | Obsolete or not wanted |

## Systems at a glance

| System | What it's responsible for | Main source |
|---|---|---|
| [Engine lifecycle](#engine-lifecycle) | Startup, frame loop, errors, shutdown | `host.c` |
| [Filesystem](#filesystem) | Pak files, search paths, game directories | `common.c` |
| [Memory](#memory) | Hunk, zone and cache allocators | `zone.c` |
| [Console and scripting](#console-and-scripting) | Commands, cvars, aliases, config files | `cmd.c`, `cvar.c`, `console.c` |
| [Input](#input) | Key bindings, mouse look, movement keys | `keys.c`, `cl_input.c` |
| [Game flow](#game-flow) | Modes, level changes, saves, cheats | `host_cmd.c`, `sv_main.c` |
| [Server simulation](#server-simulation) | World tick, entity physics, client updates | `sv_phys.c`, `sv_main.c` |
| [Collision](#collision) | Traces, hulls, entity linking, triggers | `world.c` |
| [Player movement](#player-movement) | Walking, air, water, friction | `sv_user.c` |
| [Monster movement](#monster-movement) | Step movement and chasing | `sv_move.c` |
| [Game logic and scripting](#game-logic-and-scripting) | QuakeC in the original, Luau here | `pr_exec.c`, `pr_edict.c`, `pr_cmds.c` |
| [Client](#client) | Connection, world state, interpolation | `cl_main.c`, `cl_parse.c` |
| [View](#view) | Camera, bob, kicks, screen tints | `view.c`, `chase.c` |
| [Networking](#networking) | Transport, reliability, server browsing | `net_*.c` |
| [Demos](#demos) | Recording, playback, attract loop | `cl_demo.c` |
| [World rendering](#world-rendering) | BSP, visibility, textures, lighting, sky | `r_*.c`, `gl_*.c` |
| [Models, sprites and effects](#models-sprites-and-effects) | MDL, SPR, particles, dynamic lights | `r_part.c`, `cl_tent.c` |
| [Sound](#sound) | Spatial audio, mixing, ambience | `snd_*.c` |
| [Music](#music) | CD audio | `cd_*.c` |
| [Menus](#menus) | All menu screens | `menu.c` |
| [HUD and 2D](#hud-and-2d) | Status bar, scoreboard, on-screen messages | `sbar.c`, `screen.c` |
| [Mission packs](#mission-packs) | Scourge of Armagon and Dissolution of Eternity | Flags in several files |
| [Developer tools](#developer-tools) | Profiling and debugging | Various |
| [Platform](#platform) | Window, input devices, audio output, timers | `sys_*`, `vid_*`, `in_*`, `snd_win.c` |

## Engine lifecycle

| Feature | Details | Scope |
|---|---|---|
| Startup | Initializes subsystems, then runs `exec quake.rc` from the game data, which loads configs and starts demos | Keep |
| Frame loop | Input, commands, network, server, client, draw, sound, in that order | Keep |
| Frame limiter | Skips frames faster than 72 fps, except in timedemo | Replace |
| Frame time clamp | 0.001 to 0.1 seconds, override with `host_framerate` | Replace |
| One process, client and server | Local play runs both, connected through loopback | Keep |
| Two error levels | `Host_Error` ends the current game and returns to the console. `Sys_Error` quits | Replace |
| Config on exit | Writes `config.cfg` with bindings and saved cvars | Keep |
| Dedicated server | `-dedicated` runs without video or sound, at `sys_ticrate` (0.05 s) | Later |

**The original ties physics to frame rate.** It has no fixed tick, and the 72 fps cap matters. This engine decouples them: simulation runs on a fixed tick, rendering interpolates between ticks at any frame rate, and audio runs on its own clock. Movement values will need retuning against the original's feel.

## Filesystem

| Feature | Details | Scope |
|---|---|---|
| Pak archives | `pak0.pak`, `pak1.pak` and so on, loaded in sequence until one is missing | Keep |
| Search paths | Later game directories override earlier ones. Inside a directory, paks are searched before loose files | Keep |
| Base directory | `id1`, changeable with `-basedir` | Keep |
| Mods | `-game <dir>` adds a game directory | Keep |
| `path` command | Lists the search path | Keep |
| Registered check | Looks for `gfx/pop.lmp` and sets the `registered` cvar. Shareware blocks mods and subdirectories | Drop |
| Cache directory | `-cachedir` copies files to a faster disk | Drop |

## Memory

| Feature | Details | Scope |
|---|---|---|
| Hunk | Stack allocator, cleared to a mark on map change | Replace |
| Zone | Small heap for strings and short-lived data | Replace |
| Cache | Evictable data for sounds and models, reloaded from disk on demand | Replace |
| Memory switches | `-mem`, `-heapsize`, `-zone`, `-minmemory` | Drop |
| Fixed limits | 600 edicts, 256 models and sounds, and more. See [Limits](#limits) | Drop |

## Console and scripting

| Feature | Details | Scope |
|---|---|---|
| Drop-down console | 16 KB scrollback, toggled with `~` | Keep |
| Notify lines | Recent messages over the game view, for `con_notifytime` seconds | Keep |
| Tab completion | Completes commands, then cvars | Keep |
| History | 32 lines | Keep |
| Command buffer | Queues text. `wait` delays the rest by one frame | Keep |
| Aliases | `alias name "commands"` | Keep |
| Script files | `exec file.cfg` | Keep |
| Command-line commands | `+command` arguments run at startup through `stuffcmds` | Keep |
| Cvars | Name and string value, optionally saved or announced to players | Keep |
| Command sources | Tracks whether a command came from the console or a client, so clients can't run server commands | Keep |
| Forward to server | Client sends unknown or `cmd` commands to the server | Keep |
| Chat entry | `messagemode` and `messagemode2` for team chat | Keep |
| Coloured text | The high bit of a character selects the alternate font colour | Keep |
| Log file | `-condebug` writes `qconsole.log` | Later |

## Input

| Feature | Details | Scope |
|---|---|---|
| Key bindings | `bind`, `unbind`, `unbindall`, 256 key codes | Keep |
| Input routing | Each key goes to the game, console, chat or menu | Keep |
| Button commands | `+forward`, `+attack`, `+jump` and so on, with two-key tracking so releasing one doesn't cancel the other | Keep |
| Movement speeds | `cl_forwardspeed`, `cl_sidespeed`, `cl_backspeed`, `cl_upspeed`, `+speed`, `cl_movespeedkey` | Keep |
| Keyboard look | `+left`, `+right`, `+lookup`, `+lookdown`, `centerview`. `+klook` is dropped | Keep |
| Mouse look | `+mlook`, `sensitivity`, `m_pitch`, `m_yaw`, `m_forward`, `m_side` | Keep |
| Look options | `lookspring`, `lookstrafe`, `+klook` | Drop |
| Impulse | `impulse N` sends a number to game code, used for weapon selection and cheats | Keep |
| Joystick | Advanced axis mapping in `in_win.c` | Drop |

## Game flow

| Feature | Details | Scope |
|---|---|---|
| Single-player | One client. The world stops when a menu or the console is open | Keep |
| Co-op and deathmatch | Set by `coop`, `deathmatch`, `teamplay`, up to 16 players | Keep |
| Rules | `skill` (0–3), `fraglimit`, `timelimit`, `noexit`, `samelevel`, `nomonsters`, mostly interpreted by QuakeC | Keep |
| Load a map | `map <name>` starts a new game | Keep |
| Level change | `changelevel` keeps players and carries 16 spawn parameters | Keep |
| Restart | `restart` reloads the current map | Keep |
| Connection handshake | 4 signon stages: `prespawn`, `spawn`, `begin` and ready | Keep |
| Skill filtering | Entities are removed at load based on spawnflags for skill and deathmatch | Keep |
| Save and load | Text saves, version 5, 12 menu slots. Blocked in multiplayer, in intermission, or when dead | Replace |
| Pause | `pause`, controlled by `pausable` | Keep |
| Intermission | Level-end stats screen | Keep |
| Finale and cutscene | Text screens at episode end | Keep |
| Player setup | `name`, `color` (shirt and pants) | Keep |
| Cheats | `god`, `noclip`, `fly`, `notarget`, `give`, blocked in deathmatch | Keep |
| Self-kill | `kill` | Keep |
| Server admin | `kick`, `status`, `maxplayers`, `ban`, `say`, `tell` | Later |

## Server simulation

The behaviours below stay, but movement and contact are rebuilt on `box3d`, and callbacks go to Luau.

| Feature | Details | Scope |
|---|---|---|
| Spawn | Loads the map, runs QuakeC `worldspawn`, creates entities from the BSP entity lump | Keep |
| Precache lists | Models and sounds must be registered at spawn, and are sent to clients by index | Keep |
| World tick | `StartFrame`, then each entity in order: clients first by slot, then by movetype | Keep |
| `MOVETYPE_NONE` | Runs think only | Replace |
| `WALK` | Player movement | Replace |
| `STEP` | Monsters: gravity only when airborne, moved by QuakeC | Replace |
| `FLY`, `FLYMISSILE` | No gravity, stops on impact | Replace |
| `TOSS`, `BOUNCE` | Gravity, with bounce on `BOUNCE` | Replace |
| `PUSH` | Doors, lifts and platforms that move other entities and call `blocked` | Replace |
| `NOCLIP` | Moves through everything | Replace |
| Think timers | `nextthink` runs an entity's `think` function | Replace |
| Touch | Calls `touch` on both entities in a collision | Replace |
| Water transitions | Sets `watertype` and `waterlevel`, plays the splash sound for tossed objects | Replace |
| Client updates | Sends only entities in the client's fat PVS, as deltas from a baseline | Keep |
| Static entities and sounds | Sent once at signon, then handled entirely by the client | Keep |
| Light styles | Up to 64 strings set by game code | Keep |
| Autoaim | `aim` builtin, with `sv_aim` default 0.93 | Keep |

Entity processing order is part of the behaviour. Game code can depend on it, so keep it stable.

## Collision

All of this is replaced by `box3d` queries and casts. Hull-size limits go away. See [stack.md](stack.md#physics-on-box3d) for the open questions on map collision.

| Feature | Details | Scope |
|---|---|---|
| Hull tracing | Recursive trace through BSP clipping hulls | Replace |
| 3 hulls in use | Point, player (32×32×56), large (64×64×88) | Replace |
| Hull selection | Picks a hull by entity size, so sizes outside these don't collide exactly | Replace |
| Box hulls | Non-BSP entities get a temporary box hull | Replace |
| Move types | `MOVE_NORMAL`, `MOVE_NOMONSTERS`, `MOVE_MISSILE` (which expands monster boxes) | Replace |
| Area nodes | Spatial tree of depth 4 to find nearby entities | Replace |
| Triggers | `SOLID_TRIGGER` calls `touch` on overlap without blocking | Replace |
| Solid types | `NOT`, `TRIGGER`, `BBOX`, `SLIDEBOX`, `BSP` | Replace |
| Contents | Empty, solid, water, slime, lava, sky | Replace |
| Point contents | Used for water level, damage and sound ambience | Replace |

## Player movement

Quake's movement rules are rebuilt on top of `box3d` rather than taken from it, so the behaviours and values below are the target.

These feel-critical details are in `sv_user.c` and `sv_phys.c`:

| Feature | Details | Scope |
|---|---|---|
| Ground acceleration | `sv_accelerate` 10, up to `sv_maxspeed` 320 | Replace |
| Air control | Air acceleration only up to a wish speed of 30, which is what makes air strafing work | Replace |
| Friction | `sv_friction` 4, `sv_stopspeed` 100 | Replace |
| Edge friction | Multiplied by `edgefriction` (2) when there's no floor just ahead | Replace |
| Gravity | `sv_gravity` 800 | Replace |
| Stair stepping | Up to 18 units | Replace |
| Sliding | Clips velocity against up to 5 planes | Replace |
| Wall friction | Slows you when running into walls | Replace |
| Unsticking | `SV_TryUnstick` nudges a stuck player | Replace |
| Swimming | Separate acceleration and friction, sinks when idle | Replace |
| Water jump | Jump out of water onto a ledge | Replace |
| Dead player | No movement control | Replace |
| Auto pitch | `SV_SetIdealPitch` tilts the view on slopes, with `sv_idealpitchscale` | Drop |

**Jumping wasn't in the original engine.** It's in QuakeC (`PlayerJump` in `client.qc`), along with fall damage, drowning and weapon logic. Those move to Luau, but jumping moves into the Rust movement code so it can run on the render tick.

## Monster movement

| Feature | Details | Scope |
|---|---|---|
| Step moves | `walkmove` tries a step and rejects ledges | Replace |
| Chase navigation | `movetogoal` picks directions toward a goal | Replace |
| Ground check | `SV_CheckBottom` tests for solid ground under the box | Replace |
| Flying and swimming | Vertical adjustment toward the enemy for `FL_FLY` and `FL_SWIM` | Replace |
| Line of sight help | `checkclient` picks a visible player using the PVS | Replace |

All AI decisions are in QuakeC, so they move to Luau. The engine only provides these movement tools.

## Game logic and scripting

QuakeC and `progs.dat` are dropped. `progs.dat` is the compiled game: monsters, weapons, items, doors and triggers. That logic moves to Luau through `mlua`, and the engine exposes an API to it.

| Original feature | In this engine | Scope |
|---|---|---|
| QuakeC interpreter, 66 opcodes | Luau | Drop |
| `progs.dat` format and CRC check | None | Drop |
| Flat entity memory and field offsets | Free to redesign | Drop |
| Call stack and runaway guard | Script time limit through Luau's interrupt hook | Replace |
| 65 builtins | Luau engine API, using the builtins as the starting list | Replace |
| Entry points such as `StartFrame` and `PlayerPreThink` | Luau hooks | Replace |
| Entity callbacks: `think`, `touch`, `blocked`, `use` | Luau callbacks | Replace |
| Spawn functions by `classname` | Map entity class looks up a Luau constructor | Replace |
| Entity lump parser | Same key/value parsing | Keep |
| Raw `Write*` protocol messages | Typed events, so scripts never write network bytes | Drop |
| Save support | New save format | Replace |
| Debug tools | Script profiling and inspection | Later |

**The port source** is the GPL v1.01 QuakeC: 16,804 lines in 35 files, 136 of them test code. The fixture needs it, so it gets ported to Luau, starting with the map entities listed in [stack.md](stack.md#starting-scope). `client.qc`, `items.qc`, `weapons.qc`, `doors.qc` and `ai.qc` are the largest, and there are 15 monster files. It's C-like and mostly state machines.

## Client

| Feature | Details | Scope |
|---|---|---|
| Connection state | Disconnected, connected, signon stages | Keep |
| Message parsing | 34 server message types | Replace |
| Entity interpolation | Blends between the last two server messages. The original turns it off for local play (`cl_main.c:397`), but a fixed tick needs it everywhere | Replace |
| No prediction | The original takes the player's position from the server. This engine renders it from local movement, and that code becomes prediction online | Replace |
| Scoreboard | Names, frags, colours, up to 16 | Keep |
| Stats | 32 values such as health and ammo | Keep |
| Client-side effects | Temp entities, dynamic lights and particles exist only on the client | Keep |
| Rotating items | `EF_ROTATE` model flag, spun by the client | Keep |
| Reconnect | Handles server map changes | Keep |

## View

| Feature | Details | Scope |
|---|---|---|
| Field of view | `fov`, `viewsize` | Keep |
| View bob | `cl_bob`, `cl_bobcycle`, `cl_bobup` | Keep |
| Roll | Leans on strafe with `cl_rollangle`, `cl_rollspeed` | Keep |
| Damage kicks | `v_kicktime`, `v_kickroll`, `v_kickpitch` | Keep |
| Idle sway | `v_idlescale` and related | Keep |
| Screen tints | 4 layers: contents (water, slime, lava), damage, bonus flash, powerups | Keep |
| Stair smoothing | Eases the view after steps | Keep |
| Weapon model | First-person model with its own bob | Keep |
| Chase camera | `chase_active` | Later |
| Gamma | `gamma` cvar in both renderers, plus `-gamma` at startup in GL | Replace |
| Crosshair | `crosshair` 0 or 1 | Keep |

## Networking

Deferred. Dropping QuakeC removes the requirement to keep protocol 15, so the design is open.

| Feature | Details | Scope |
|---|---|---|
| Loopback | In-memory transport for local play | Keep |
| UDP | Default port 26000 | Keep |
| Reliable messages | One in flight at a time, split into chunks, acknowledged and resent | Keep |
| Unreliable messages | Entity updates, up to 1024 bytes | Keep |
| Timeout | `net_messagetimeout` 300 seconds | Keep |
| Connection control | Connect, accept and reject packets | Keep |
| Server queries | Server info, player info and rule info | Later |
| LAN browser | `slist` broadcast search | Later |
| Bans | `ban` by IP address and mask | Later |
| `listen`, `port`, `net_stats` | Server network controls | Later |
| `test`, `test2` | Query a server's players and rules | Drop |
| Network VCR | `-record`, `-playback` for debugging | Drop |
| IPX, serial, modem, Mplayer | Legacy transports | Drop |

There's no remote console, download system, or password support in this version. Those are QuakeWorld features.

## Demos

| Feature | Details | Scope |
|---|---|---|
| Record | `record <name> [map] [cdtrack]` writes `.dem` | Replace |
| Playback | `playdemo`, `stop`, `stopdemo` | Replace |
| Benchmark | `timedemo` | Later |
| Attract loop | `startdemos`, `demos`, plays the title demos | Drop |

Stock demos store raw protocol 15 messages, so they can't play on a new protocol or stand in as regression tests. Demo recording needs a new format, and `box3d`'s recording and replay tools may help.

## World rendering

All rendering is rebuilt on `vulkane`, so nothing here is ported as code. These are the behaviours to reproduce or replace.

| Feature | Details | Scope |
|---|---|---|
| BSP traversal | Front-to-back walk of map nodes | Replace |
| PVS | Compressed per-leaf visibility | Replace |
| Frustum culling | Clips nodes and entities to the view | Replace |
| Brush submodels | Doors and lifts are BSP models with their own transform | Replace |
| Mip textures | 4 levels, palette-indexed | Replace |
| Animated textures | `+0`–`+9` frames, `+a`–`+j` alternates for switches | Replace |
| Turbulent surfaces | `*` textures warp for water, slime, lava, teleports | Replace |
| Sky | Two scrolling layers from a `sky` texture | Replace |
| Lightmaps | Up to 4 styles per surface, blended every frame | Replace |
| Animated light styles | `a`–`z` strings, stepped at 10 Hz | Replace |
| Dynamic lights | Up to 32, added to lightmaps | Replace |
| Palette | One 256-colour palette plus a `colormap` for shading | Drop |
| Player colours | Shirt and pants ranges remapped per player | Drop |
| Entity linking to leafs | Efrags sort static entities for drawing | Replace |
| Screenshots | PCX in software, TGA in GL | Replace |
| Environment map | `envmap` debug capture | Drop |

### Renderer differences

These describe the two original renderers. For the `vulkane` renderer, each row is a choice of behaviour, not a code path.

| Feature | Software | OpenGL |
|---|---|---|
| Hidden surfaces | Sorted edges, no overdraw, Z written for entities | Z-buffer |
| Colour | 8-bit palette | 16-bit or higher, or paletted with an extension |
| Texture filtering | Nearest, with mipmaps | `gl_texturemode`, bilinear by default |
| Fullbright texels | Yes, through the colormap | **No** |
| Surface lighting | Surface cache combines texture and lightmap | Separate lightmap pass or multitexture |
| Underwater warp | Whole-screen warp (`r_waterwarp`) | **None** |
| Water surfaces | Opaque | `r_wateralpha`, but stock maps lack the vis data |
| Screen tints | Palette changes | Drawn as a blended overlay |
| Dynamic light display | Lit surfaces | `gl_flashblend` glow balls by default, or lit surfaces |
| Model shadows | No | `r_shadows` |
| Mirrors | No | `r_mirroralpha`, on one texture only |
| Model rendering | Affine triangles | Strips and fans, cached in `glquake/` |
| Texture sizes | Any multiple of 16 | Must be powers of two, resampled on upload |
| Resolution | Changeable at runtime through the menu | Startup only, with separate `-conwidth` |

## Models, sprites and effects

| Feature | Details | Scope |
|---|---|---|
| MDL models | Vertex animation, single frames and frame groups | Replace |
| Skins | Single or animated groups | Replace |
| Model lighting | From the floor below plus dynamic lights, with precomputed normals | Replace |
| Frame display | Snaps between frames, with no interpolation | Drop |
| Model flags | Rocket, grenade, gib, tracer and other trails, set in the MDL file | Replace |
| Sprites | 5 orientation types, single frames and groups | Replace |
| Static entities | Up to 128, for torches and similar | Replace |
| Temp entities | 14 types, such as gunshots, explosions, lightning beams, teleports | Replace |
| Beams | Up to 24 | Replace |
| Entity effects | `EF_BRIGHTFIELD`, `MUZZLEFLASH`, `BRIGHTLIGHT`, `DIMLIGHT` | Replace |
| Particles | Up to 2048 by default, 8 behaviour types | Replace |
| Particle effects | Explosion, blob explosion, lava splash, teleport splash, entity field, 7 trail types | Replace |

## Sound

| Feature | Details | Scope |
|---|---|---|
| Audio clock | The original mixes from the frame loop. Here audio runs independently of frame rate | Replace |
| Channel layout | 128 total: 4 ambient, 8 dynamic, the rest static | Keep |
| Entity channels | A new sound replaces the one on the same entity channel | Keep |
| Spatial sound | Distance attenuation and stereo pan | Keep |
| Static sounds | Fixed-position looping sounds | Keep |
| Ambient sounds | Water, sky, slime and lava levels from the BSP leaf, faded by `ambient_fade` | Keep |
| Looping | Loop points read from WAV `cue ` chunks | Keep |
| Resampling | Converts to the output rate at load | Replace |
| Mixer | Software paint buffer | Replace |
| Controls | `volume`, `nosound`, `play`, `playvol`, `stopsound` | Keep |
| Precache | Loads on demand through the cache | Replace |

## Music

| Feature | Details | Scope |
|---|---|---|
| CD audio | Server selects a track with `svc_cdtrack`, looping | Drop |
| `cd` command | Play, stop, loop, remap | Drop |
| Music volume | `bgmvolume` | Keep |
| Music playback | Plays audio files by track | Replace |

CD playback is dropped. Music plays from audio files instead, which is new work.

## Menus

There are 18 screens, all drawn from `gfx` images:

| Group | Screens | Scope |
|---|---|---|
| Main | Main, Quit (with its message list), Help (multiple pages) | Keep |
| Single-player | New game, Load, Save | Keep |
| Multiplayer | Multiplayer, Setup (name and colours), Game options, LAN config, Search, Server list | Later |
| Options | Options, Keys, Video | Keep |
| Legacy | Serial config, Modem config, the Net type chooser | Drop |

Game options has map and episode lists.

## HUD and 2D

| Feature | Details | Scope |
|---|---|---|
| Status bar | Health, armour, ammo, face, weapons, items, keys, sigils | Keep |
| Face | Changes with health, pain, invulnerability and quad | Keep |
| Scoreboard | `+showscores` | Keep |
| Mini overlay | Small frag list in deathmatch | Keep |
| Intermission | Time, secrets and kills | Keep |
| Finale | End text | Keep |
| Centered messages | `svc_centerprint`, typed out slowly in finale | Keep |
| Loading and pause | Plaque images | Keep |
| Warning icons | `showturtle`, `showram`, network icon | Drop |
| Screen size | `viewsize`, shrinks the view, hides parts of the bar | Drop |

## Mission packs

The hardcoded HUD and menu variants are dropped. The original selects them with `-hipnotic` or `-rogue`, which also add a game directory.

| Feature | Details | Scope |
|---|---|---|
| Scourge of Armagon (`-hipnotic`) | HUD for new weapons and items, new menu lists | Drop |
| Dissolution of Eternity (`-rogue`) | Its own HUD layout, inventory and menus | Drop |
| Shared engine changes | `TE_EXPLOSION2`, `TE_BEAM` | Later |

## Developer tools

| Feature | Details | Scope |
|---|---|---|
| Performance | `r_speeds`, `host_speeds`, `timerefresh` | Later |
| Rendering debug | `r_drawflat`, `r_novis`, `r_fullbright`, `r_lightmap` | Later |
| Map leak trace | `pointfile` draws leak paths from map tools | Later |
| Model viewer | `viewmodel`, `viewframe`, `viewnext`, `viewprev` | Later |
| Asset info | `mcache`, `soundlist`, `soundinfo` | Later |
| Logging | `developer`, `cl_shownet` | Later |

## Platform

All of this gets replaced with Rust libraries. It's listed so nothing gets forgotten.

| Feature | Original | Scope |
|---|---|---|
| Window and video modes | DirectDraw, DIB, MGL, WGL, X11, GLX, SVGALib | Replace |
| Mouse and keyboard | DirectInput, Win32 messages, X11 | Replace |
| Joystick | Win32 joystick API | Replace |
| Sound output | DirectSound, waveOut, OSS | Replace |
| Timing | Platform timers | Replace |
| File I/O | Platform calls | Replace |
| Dedicated console | Windows console bridge, stdin | Later |
| DOS, Solaris, NeXT | Legacy platforms | Drop |

## Limits

These are the original hard limits, listed for reference. Most are dropped. Networking may need to set its own limits on message sizes and player counts when that's decided.

| Limit | Value |
|---|---:|
| Edicts | 600 |
| Models, sounds per map | 256 each |
| Players | 16 |
| Light styles | 64 |
| Reliable message | 8000 bytes |
| Unreliable message | 1024 bytes |
| Static entities | 128 |
| Temp entities | 64 |
| Dynamic lights | 32 |
| Beams | 24 |
| Efrags | 640 |
| Particles | 2048 |
| Sound effects | 512 |
| Sound channels | 128 |
| Lightmap blocks (GL) | 64 |
| Files per pak | 2048 |
| Save slots in menu | 12 |
| Spawn parameters | 16 |

## New in this engine

| Feature | Status |
|---|---|
| Fixed simulation tick with interpolated rendering | Decided, tick rate open |
| `box3d` physics | Decided |
| `vulkane` renderer | Decided |
| Luau game logic and scripting | Decided |
| Music from audio files | Replaces CD audio |
| Gamepad | New feature, replaces the old joystick code. Not yet scheduled |
| Ogg audio | Shortlisted with Kira |
| Theora video | Deferred |

## Not in the baseline

| Group | Features |
|---|---|
| Disabled `QUAKE2` code | See [baseline.md](baseline.md#compile-flags) |
| QuakeWorld only | Client prediction, delta entities, downloads, skins, spectators, rcon, passwords, userinfo and serverinfo strings, master servers, `cl_maxfps`, netgraph, map checksums, flood protection |
| Later GL changes in QuakeWorld | Coloured powerup glows, translucent console, partial lightmap upload |
