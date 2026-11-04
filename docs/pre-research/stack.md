# Technology choices

> **Pre-research.** This records decisions and early findings from before the [workflow](../../DESIGN.md#workflow) was adopted. It's an input to research, not research, and it mixes agreed decisions with suggestions. What the human has agreed is in [DESIGN.md](../../DESIGN.md).

This tracks what the engine is built on and what's still open. Choices get made while walking the [feature catalog](feature-catalog.md).

This isn't a recreation of Quake. It's a new engine on modern foundations, with the original acting mostly as a fixture: its maps and map entities give us something real to build and test against. The first goal is a solid baseline, and larger changes come after it.

## Starting scope

QuakeC gets ported to Luau, because the fixture needs it. The port starts with the map side of the game, enough to walk through real maps and have them work:

| Group | Entities (proposed) |
|---|---|
| Movers | `func_door`, `func_door_secret`, `func_plat`, `func_train`, `func_button` |
| Static brushes | `func_wall`, `func_illusionary` |
| Triggers | `trigger_multiple`, `trigger_once`, `trigger_relay`, `trigger_counter`, `trigger_teleport`, `trigger_push`, `trigger_hurt`, `trigger_changelevel` |
| Markers | `info_player_start`, `info_teleport_destination`, `path_corner`, `info_null` |
| World | `worldspawn`, `light` and its variants, `ambient_*` |

Monsters, weapons and items follow in later passes. The port source is the GPL v1.01 QuakeC.

## Decided

| Item | Choice | Notes |
|---|---|---|
| Language | Rust 1.98.1 | Pinned in `rust-toolchain.toml` |
| App host | `solapp` | Engine crate in `crates/solapp`. Windows x64 only, so engine logic shouldn't depend on it directly |
| Timing | Fixed simulation tick | 72 Hz. Rendering and audio are independent of it, and the local player uses a render tick. See [Fixed tick](#fixed-tick) |
| Physics | `box3d` (`box3d-rust`) | Pure Rust, MIT. Quake's movement rules are rebuilt on top. Not in this repo yet |
| Renderer | `vulkane` | 0.16.0, MIT or Apache-2.0. Rendering features are remade on it. Not in this repo yet |
| Scripting and game logic | Luau through `mlua` | Replaces QuakeC and `progs.dat` |
| Audio | `kira` | 0.12.6, MIT or Apache-2.0. Runs on its own audio thread, so sound is independent of frame rate. Not in this repo yet |
| Collision | Hull solids in `box3d` | Rebuilt from BSP hulls by the map importer. See [Physics on box3d](#physics-on-box3d) |
| Maps | BSP through an importer | `qbsp` 0.17.0 reads BSP29, BSP2 and `.lit` into an engine-owned map type, so formats can change. See [maps.md](maps.md) |
| Server entities | Generational slot table | Small and ours, built to be extended. See [Entity storage research](#entity-storage-research) |

`solworker` and `solcache` are also in `crates/` and can be edited here. Neither is used by engine code yet.

**`vulkane` source:** `test` carries a patched copy that adds array images, explicit mip levels and mipmapped samplers (`SOLSTICE-CHANGES.md` in that copy). Mip textures need the same things, so that copy is the better starting point. Keep the `fetch-spec` feature off: `test/UPGRADE_ADVISORY.md` documents a TLS issue in the build-time download path it uses.

**`box3d` source:** `test` carries version 0.4.0 from upstream commit `1685214`, and `src/collision.rs` there is a working example of its mover API.

## Shortlisted

| Item | Candidate for | What we know |
|---|---|---|
| `shipyard` | Client or render data | Possible later. Not used for server entities |
| `assets_manager` | Asset loading and caching | 0.13.10, MIT or Apache-2.0. See [Assets](#assets). `assets_manager-kira` connects it to Kira |

## Deferred

| Item | Status |
|---|---|
| Networking | `renet` chosen, the rest under investigation. See [networking.md](networking.md). External servers are out of scope for now |
| Asset management | Core decided: `assets_manager`, a text id table and FlatBuffers pack indexes. See [Assets](#assets). Details get settled as we build |
| Theora video | Deferred. Kira doesn't cover video, and no mature pure-Rust decoder turned up |

## Open questions

### Fixed tick

**Decision: 72 Hz.** It's the original's frame cap, so it's the rate Quake's movement was tuned at, and `box3d`'s `world.step` accepts any time step.

Quake's movement code run at fixed rates, using its constants and update order:

| Hz | Jump height | Stop distance from 320 | Speed after 1 s of air strafing |
|---:|---:|---:|---:|
| 20 | 39.00 | 54.0 | 347 |
| 30 | 41.11 | 58.5 | 360 |
| 60 | 43.33 | 63.0 | 397 |
| **72** | **43.70** | **63.8** | **409** |
| 77 | 43.81 | 64.0 | 414 |
| 100 | 44.22 | 64.8 | 440 |
| 144 | 44.63 | 65.6 | 417 |
| 240 | 45.00 | 66.4 | 381 |

- **Jumping and stopping** change slowly with rate. This fits reports that [physics changes above 72 fps in Quakespasm](https://sourceforge.net/p/quakespasm/discussion/1092022/thread/a180cafc/?limit=25), and it's why [vkQuake](https://github.com/Novum/vkQuake/issues/149) separates its renderer from physics.
- **Air strafing** is the sensitive one. Air acceleration is capped per tick, so the gain peaks near 100 Hz and falls after that. After one second, 60 Hz gains about 13% less speed than 72 Hz.

The strafing figures use an idealised input, and the real game's frame time wasn't perfectly constant, so treat them as a comparison between rates.

**Why 72 and not 77:** 77 belongs to QuakeWorld, not this baseline.

- WinQuake caps at 1/72 s in `Host_FilterTime` and uses the real frame time as a float.
- Stock QuakeWorld 2.40 also clamps to 72 (`min(cl_maxfps, 72)`), so 77 isn't in the original either. It's a [later convention](https://www.quakeworld.nu/forum/topic/2356/qw-physics-vs-nq-physics-everything-you-ever-wanted-to-know), and [most QuakeWorld servers run at it](https://ezquake.com/docs/independent-physics.html).
- QuakeWorld sends move time as whole milliseconds, truncated in `CL_FinishMove` (`ms = host_frametime * 1000`). A 13.89 ms frame at 72 fps is sent as 13 ms, so about 6% of real time goes missing, and 77 fps is about 13 ms exactly. That's likely why 77 caught on, but it's my reading of the code. A forum post mentions an "msec rounding bug", and nothing I found explains it in full.
- [QuakeSpasm Spiked](https://fte.triptohell.info/moodles/qss/) keeps physics and networking at 72 with uncapped rendering, which is the same design as here.

**Latency:** plain interpolation costs more than one tick, so it's not the plan for the local player.

| | Added delay at 72 Hz |
|---|---|
| Interpolated display | A constant 13.9 ms ([one physics update](https://docs.unity.com/en-us/engine/current/manual/physics-section/physics-overview/rigidbody/rigidbody-interpolation)) |
| Waiting for the next tick to read input | 0–13.9 ms |
| Movement input to screen | 13.9–27.8 ms |

**Decision: a render tick for the local player.** Each frame, copy the player's state, run movement for the time left over since the last tick using the latest input, and draw that. The copy is thrown away, and the real simulation stays fixed at 72 Hz. This is the [render tick technique](https://jakubtomsu.github.io/posts/fixed_timestep_without_interpolation/) applied to the player only, and it's the same idea as QuakeWorld's client prediction.

Compared in a simplified 2D model of Quake's movement (running, jumping, reversing, a wall), with a 72 Hz tick and 240 Hz rendering:

| Method | Mean distance behind | Typical frame-to-frame bump | Largest bump |
|---|---:|---:|---:|
| Interpolation | 3.7 units | 0.26 | 0.7 |
| Render tick | 0.03 units | 0.6 | 3.3 |
| No smoothing | 1.8 units | 5.5 | 5.7 |

The render tick removes nearly all the lag. Its cost is a small visible bump when input changes, because the next real tick doesn't exactly match the prediction. If that shows up in play, a short blend on the correction would hide it. This model has no real map collision, so it needs confirming in the engine.

**What this requires:**

- **Movement is a pure Rust function** of state, input, time and collision, so it can run on a copy. That rules out running it through Luau.
- **Jumping moves into that function.** In the original it's QuakeC (`PlayerJump`). QuakeWorld made the same move for prediction (`JumpButton` in `pmove.c`). Fall damage and sounds can stay in Luau.
- **Other entities stay interpolated**, one tick behind. Their velocity changes too often for extrapolation, which [overshoots](https://docs.unity.com/en-us/engine/current/manual/physics-section/physics-overview/rigidbody/rigidbody-interpolation).
- **Firing is processed on the tick but timestamped within it,** so shots aren't rounded to tick boundaries. See [networking.md](networking.md#trailing-hitboxes).

**Loop design**, following [Fix Your Timestep](https://gafferongames.com/post/fix_your_timestep/):

- An accumulator runs fixed ticks with the latest input.
- Frame time is clamped so a stall can't trigger a catch-up spiral.
- Mouse look applies at render rate, so it has no tick delay.
- `solapp` provides frames and services but no simulation step, so we write this. Use `RendererManaged` pacing, since `Stock` rounds to whole milliseconds. The loop must keep ticking when the window is minimized, which still needs checking.

### Online feel

The render tick fixes offline play. Online, a client without prediction waits a full round trip to see its own movement, which is why the original feels sluggish over the internet. Networking is still deferred, but these requirements affect how the offline engine is built.

Where the delay comes from at 50 ms ping (my arithmetic, using a 72 Hz server):

| What the player sees | No prediction (WinQuake) | With prediction |
|---|---|---|
| Own movement and looking | About 65–95 ms | About 0 |
| Own shot registering | About 65–95 ms | Same, unless effects are predicted |
| Other players' positions | Behind by half the ping plus buffering | Same |
| Rocket knockback, doors, teleports | About 65–95 ms | Corrected when the server replies |

**Prediction uses the same code as the render tick.** QuakeWorld's [`CL_PredictMove`](https://fabiensanglard.net/quakeSource/quakeSourcePrediction.php) starts from the last server state and replays every unacknowledged command, then blends a fraction of the last frame. That's [standard reconciliation](https://www.gabrielgambetta.com/client-side-prediction-server-reconciliation.html). Our render tick is that loop with no commands outstanding, so offline can be treated as online with zero ping.

**Things prediction can't fix:**

- **Hitting other players.** QuakeWorld has no lag compensation, so [players had to lead instant-hit shots](https://www.quakeworld.nu/blog/257/antilag-explained). Later servers added rewind for hitscan weapons, which needs the server to keep position history.
- **Peeker's advantage.** [Riot's analysis](https://www.riotgames.com/en/news/peeking-valorants-netcode) counts buffering in frames, about 2 on the server and 3 on the client. Higher tick and frame rates shrink it, and going from 60 to 144 fps took its example from about 100 ms to about 71 ms.
- **Server-driven movement.** Knockback arrives a round trip late unless it's predicted.

**What the offline engine must do so this stays possible:**

1. **Keep the client and server separate**, talking through loopback even offline, as the original does. One code path, not two.
2. **Use numbered input commands**, each with its duration, kept in a history. Offline only ever has the latest one.
3. **Keep movement state small and self-contained**: position, velocity and flags, with no hidden globals. Anything scripts change that affects movement, such as speed or gravity, has to be part of that state. QuakeWorld [added per-player `maxspeed` and `gravity` for this reason](../../reference/quake/QW/qwrlnote.txt).
4. **Run the same collision on the client.** The client builds the same map solids, and has a list of nearby moving objects to collide with, like QuakeWorld's `physents`.
5. **Run game scripts on the server only.** The client runs movement and presentation, not game rules.
6. **Stamp server state with tick numbers,** and render other entities from a time-based buffer, not a fixed "last two ticks". A later network layer can then change how far behind they run.
7. **Keep effects separate from results,** so muzzle flashes and sounds can later play straight away on the client while the server decides what was hit.
8. **Keep per-tick position history on the server** and stamp shots with exact view times, so lag compensation can be exact.

**72 Hz is a good server rate for this.** Buffering is counted in ticks, so 72 Hz behaves much like the 64-tick servers common in shooters. See [networking.md](networking.md) for transport and replication.

### Physics on box3d

**Decision: rebuild convex solids from the BSP collision hulls when a map loads, and load them into `box3d` as hull shapes.** The map importer does this, so physics only ever sees solids. Each solid leaf of a BSP tree is a convex region, so it can be recovered by clipping the map's bounds down the tree. No new map format is needed.

Tested on all 51 maps in `id1` (data read locally, nothing copied into this repo):

| Check | Result |
|---|---|
| Largest map | 2,771 solids in one hull |
| Largest solid | 36 vertices. `box3d`'s limit is 128 |
| Build time | About 12 ms per hull on the larger tested maps |
| Hull shape failures | 0 on the 5 maps loaded into `box3d` |

Then 100,000 random traces per map on `e1m1`, `e1m7`, `e4m3`, `dm3` and `start`, compared with a port of Quake's own `SV_RecursiveHullCheck`:

| Method | Hits disagree | Within 0.1 unit | Over 1 unit off | Time per trace |
|---|---:|---:|---:|---:|
| Quake's trace | n/a | n/a | n/a | 0.25–0.48 µs |
| `box3d` ray vs hull 1, against Quake's trace with its epsilon removed | 0 | 100% | 0 | 0.58–0.86 µs |
| `box3d` ray vs hull 1, against unmodified Quake | 0–3 | 90–93% | About 0.1% | Same |
| `box3d` box sweep vs hull 0, against unmodified Quake | 5–118 | 92–95% | 0.1–1.3% | 1.4–3.4 µs |

- **The geometry is exact.** The first `box3d` row matched on every trace. The small differences in the next row are Quake's own back-off from surfaces (`DIST_EPSILON` and its retry loop), which our movement code can reproduce.
- **A ray through hull 1 is a box sweep.** The compiler already expanded hull 1 by the player's box, so tracing a point through it gives the same result as sweeping that box. This keeps the box player decision and includes `clip` brushes.
- **Sweeping a box through hull 0 is worse.** It's slower, and almost all large errors let the player pass further than Quake would. `clip` brushes only exist in hulls 1 and 2 (`QBSP/BRUSH.C:725`), which is the likely cause.
- **Sizes.** Rays work for Quake's 3 hull sizes, the same limit as the original. Other sizes use a box sweep through hull 0 and lose `clip` brushes.

**Still to do or test:**

- Doors and lifts: each brush model has its own hulls, so they'd be kinematic bodies. Not tested.
- Sweeps against other entities: boxes need expanding as Quake does in `SV_HullForBox`, or a separate shape cast. Not tested.
- Actually walking on this. These tests only compare traces.
- `BSP2`: 2 of the 81 re-release maps use it, and it needs a wider parser.
- Length units: Quake units are about an inch, so rigid bodies will want `box3d`'s length scale set. Rays don't care.
- Newly compiled maps can carry real brushes through [`-wrbrushes`](http://ericwa.github.io/ericw-tools/doc/qbsp.html), which would avoid the rebuild and allow any size.
- **Rule ownership.** Stepping, friction, air control and water movement are all ours. `box3d` supplies queries and casts.
- **Determinism.** Its README claims bit-exact results against the C reference, and it has recording and replay. That's useful for tests and possibly networking, but hasn't been tried here.

### Luau

- **API shape.** The 65 builtins are the starting list, not the final API.
- **Where scripts run.** The server simulation at minimum. UI and mods are separate questions.

### Assets

`assets_manager` handles loading, caching and hot reload. It has filesystem, embedded, zip and tar sources, and its default build pulls in only `hashbrown`, `log` and `foldhash`.

**File ids work through a custom `Source`.** Its ids are strings with no built-in numeric form, but a throwaway prototype confirmed this approach:

- A table maps an assigned number to a file. Loading `"1042"` resolves through it, and repeat loads come from the cache.
- An asset type can accept several extensions, so one id can point at `.wav` today and `.ogg` later without callers changing.
- Unknown ids and ids of the wrong type return errors.

**What we'd still own or need to decide:**

- **Pack format.** Reading Quake paks needs our own `Source`. The prototype used in-memory data, not real pak files.
- **Paths and ids together.** Each id string is cached separately, so loading one file by both a path and a number loads it twice. Pick one canonical form inside the engine.
- **Id rules.** `/`, `\`, `..` and leading or trailing dots are rejected, so a path like `sound/ambience/water1` needs converting first.
- **Per-level unloading.** The cache has no remove call. `make_child()` creates a child cache, which looks like the way to get per-level lifetime. Not yet tested.
- **Hot reload.** A custom `Source` has to implement it itself.

**File ids and indexes:** see [Asset id research](#asset-id-research) for the evidence behind these choices.

- **Assigned `u32` ids.** They survive renames and format changes, and a `u32` is cheap to store and compare.
- **Text id table as the source of truth.** A `id;path` file in the repo, reviewed and merged like code. Binary files can't be diffed or merged, and assigned ids are exactly where merges go wrong.
- **FlatBuffers index for packs, generated with `planus`.** New packs embed it. The table and pak tool produce a sidecar for paks that don't have one, so original paks are never modified.
- **Index contents.** A format version, the pack's SHA-256, and entries of id, path, offset and length, sorted by id. Use explicit field ids in the schema so it can evolve.
- **Pak tool.** It adds new files to the table with the next free id, keeps existing ids by path, and never reuses a removed id.
- **Missing indexes.** The engine reports an error pointing at the tool. It doesn't invent ids at load time, because those wouldn't be stable.
- **Mods are unsolved.** Fixed ranges work for our own content, but third-party authors can't coordinate them. Scoping ids by pack, or letting mods use a wider id, are the likely answers. Not needed for the baseline.

### Asset id research

**How other systems identify assets:**

| System | Identity | Where it's stored | Weak point |
|---|---|---|---|
| [Unity](https://docs.unity.com/en-us/engine/current/manual/assets-and-media/import-assets/asset-metadata) | Unique id | Text `.meta` beside each asset | Losing a `.meta` file breaks references |
| [Godot 4.4](https://godotengine.org/article/uid-changes-coming-to-godot-4-4/) | `uid://` | Sidecar `.uid` files | Moving files outside the editor needs care |
| [Unreal](https://dev.epicgames.com/documentation/en-us/unreal-engine/asset-redirectors-in-unreal-engine) | Path, with redirectors left behind on moves | In packages | Redirectors need cleanup and can block deletion |
| [WoW community listfile](https://github.com/wowdev/wow-listfile) | Numeric file id | CSV files | The closest match: names bound to ids for files that have none |

[Names vs GUIDs](https://www.gamedeveloper.com/programming/referencing-objects-names-vs-guids) recommends stable ids by default and names only where late binding helps.

**Path hashes were rejected.** They change on rename, and 32-bit hashes collide quickly. The birthday bound gives a 4.6% chance at 20,000 files and 69% at 100,000. `populous-reborn` is small enough for this to be fine, but a growing engine isn't.

**Binary format comparison:**

| Format | Schema evolution | Rust advisories | Notes |
|---|---|---|---|
| FlatBuffers | [Yes, with rules](https://flatbuffers.dev/evolution/) | [3, all old](https://github.com/rustsec/advisory-db/tree/main/crates/flatbuffers) | Needs `flatc`. Generated code had a serious soundness advisory, patched in 22.9.29 |
| FlatBuffers via [`planus`](https://docs.rs/crate/planus/latest/source/README.md) | Same format | None listed | Pure Rust, validates on access. Doesn't support `file_identifier` or fixed arrays |
| Cap'n Proto | Yes | 2 | Needs its own compiler. Slowest access of the three in the benchmark |
| rkyv | [No](https://rkyv.org/feature-comparison.html) | [6, five from 2026](https://github.com/rustsec/advisory-db/tree/main/crates/rkyv) | Fastest in the [benchmark](https://github.com/djkoloski/rust_serialization_benchmark), but several advisories involve crafted archives |

**Speed doesn't decide this.** Parsing a text table took 184 µs for 2,048 rows, 8 ms for 100,000 and 132 ms for 1,000,000. A Quake pak holds at most 2,048 files. `planus` opened a 100,000-entry index and read one entry in 500 ns, which only starts to matter for much larger packs.

**`unsafe` caveat.** Both generators emit it: 10 lines from `planus` and 13 from `flatc` for the same test schema. Generated code goes in its own module with the workspace's lint relaxed there.

Tests ran once on this machine in a throwaway project. Corruption handling wasn't properly tested, so it still needs a real fuzz test.

### Entity storage research

The original runs entities in slot order and reuses the lowest free slot once it's been free for 0.5 seconds (`ED_Alloc` in `pr_edict.c`). That order is behaviour we want to keep.

Delete one entity from six, spawn another, then iterate:

| Storage | Order | Keeps slot order |
|---|---|---|
| Original Quake | 0, new, 2, 3, 4, 5 (after the reuse delay) | Yes |
| `slotmap` 1.1.1 | 0, new, 2, 3, 4, 5 | Yes |
| `shipyard` 0.11.5 | 0, 5, 2, 3, 4, new | No |
| `hecs` 0.11.2 | 0, 5, 2, 3, 4, new | No |

`shipyard`'s [guide](https://leudz.github.io/shipyard/guide/master/fundamentals/iterators.html) says not to expect any specific order from its iterators. It can sort a storage on demand, which restored order in the test, but that's work on every change.

An ECS also buys little here. Quake has hundreds to a few thousand entities, so cache-friendly iteration isn't the bottleneck. Game state will live in Luau, and [existing Lua-to-ECS bridges](https://docs.rs/suon_lua) need per-component bindings or reflection, a non-`Send` VM workaround, or both.

**Decision:** a generational slot table for server entities, written as a small type of our own rather than using `slotmap`. `slotmap` keeps iteration order, but it reuses the most recently freed slot immediately, while Quake takes the lowest free slot and holds freed slots for 0.5 seconds. That delay stops new entities from inheriting stale references.

**How it stays extensible:**

- **Opaque ids.** The rest of the engine only sees `EntityId` (index and generation) and a small API, so the storage behind it can change.
- **Side tables.** New per-entity data goes in separate tables keyed by `EntityId`, the same idea as `slotmap`'s `SecondaryMap`. Physics bodies, render state and networking state can each have their own without touching the core.
- **Ordered iteration as a contract.** A test pins slot order and reuse, so a later replacement has to match it or change it on purpose.
- **Room for an ECS.** Engine data lives in plain Rust and game fields in a Luau table per entity. Side tables are the same shape as ECS components, so `shipyard` or similar could replace them later without changing the ids scripts hold.

### Audio research

| Crate | Version | Last release | Recent downloads | Notes |
|---|---|---|---|---|
| `kira` | 0.12.6 | 2026-10 | 273,000 | Game focused: mixer, clocks, spatial tracks, tweens |
| `rodio` | 0.23.0 | 2026-10 | 3,858,000 | Most used, but its maintainers call the API limiting beyond simple playback |
| `firewheel` | 0.14.0 | 2026-09 | 26,000 | Audio graph, heading towards Bevy as its default engine |
| `oddio` | 0.7.4 | 2023-10 | 8,000 | Good spatial design, but no release in three years |
| `fyrox-sound` | 1.0.1 | 2026-03 | 6,000 | Part of the Fyrox engine |

No independent latency or spatial comparison turned up, so this rests on features and a direct test.

**Kira test results:**

- An 8-bit, 11,025 Hz mono WAV with a `cue ` chunk decoded correctly.
- `loop_region` accepts a loop start, but Kira doesn't read `cue ` chunks, so a small parser of ours supplies the position.
- Default capacity is 128 sub-tracks, and it's configurable. Spatial sound works per track, so moving sources each need one.
- `attenuation_function`, `distances` and `spatialization_strength` are available for matching Quake's falloff.

Actual playback, latency and channel behaviour weren't tested, because that needs a running audio device and a real game loop.

**Decision:** Kira. `assets_manager-kira` connects it to asset loading.

### Regression testing

Stock demos can't serve as tests any more, because they need protocol 15. Exact parity with Quake isn't the goal, so tests can use our own recorded scenes and check that maps load and doors, lifts and triggers work.
