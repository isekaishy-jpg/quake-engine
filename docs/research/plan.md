# Research plan

Each item below goes through the [workflow](../../DESIGN.md#workflow) on its own. Research covers how to do it: the technical details, sources, constraints and evidence. The [pre-research decisions](../pre-research/README.md) are inputs, not answers.

The order is a first draft. Earlier items unblock later ones.

## Increment 0: repository and build

Done, in proposals 0001 to 0009, apart from:

| Item | What research has to establish |
|---|---|
| `engine_host`, `engine_worker` and `engine_cache` under the workspace lints | About 800 warnings to work through |

## Dependencies

One item each, covering fit, version, licence, how it's integrated, and how it's kept up to date: `vulkane`, `box3d`, `mlua` with Luau, `kira`, `assets_manager`, `qbsp`, `renet`, FlatBuffers through `planus`.

## Engine foundations

| Item | What research has to establish |
|---|---|
| App host integration | How the engine sits on `engine_host`, and what stays free of it |
| Main loop | Accumulator, frame and tick order, minimized behaviour, stalls |
| Console, commands and cvars | Structure, threading and persistence |
| Filesystem and packs | Search paths, pak reading and the `assets_manager` source |
| File ids | The text table, pack indexes and the pak tool |

## Simulation

| Item | What research has to establish |
|---|---|
| Entity storage | The slot table's exact design and its extension path |
| Map importer | The engine map type, and how each part is built from BSP |
| Collision | Rebuilt solids, brush models, entity sweeps and BSP2 |
| Player movement | A pure movement function, surface margins and stuck handling |
| Luau host | The API, sandbox, budgets and how entities map onto it |
| QuakeC port | How to port it, in what order, and how to verify it |
| Saves | The format and what it covers |

## Presentation

| Item | What research has to establish |
|---|---|
| Renderer | The `vulkane` architecture, world, models, lighting, sky and water |
| The look | Palette, fullbright and overbright |
| Particles and effects | How they're represented and drawn |
| Audio | Kira integration, spatial sound, loop points and music |
| Menus, HUD and 2D | How they're built and scripted |

## Networking

| Item | What research has to establish |
|---|---|
| Transport | `renet` in practice, loopback, loss and latency behaviour |
| Replication | Snapshot format, delta encoding and baselines |
| Prediction | Command history, replay, the render tick and corrections |
| Lag compensation | Position history, view times and movers |
| Fair play | The zone, delay queues and server settings |
| Weapon timing | Sub-tick timestamps and the half tick |
| Projectiles | Launch events, shared paths, display and easing |
| Rocket jumps | Predicted self-knockback |
| Client clock and jitter | Time sync and buffers |

## Later

External servers, relay and NAT traversal, Linux dedicated servers, Theora video.

### Real sound features

What increment 5 does to sound is a port with fixes on top, not an audio design. Volume falloff, stereo, ambient levels and mixing should become real features, researched properly against what's normal now, which should cover and go past these stopgaps.

| Today's stopgap | What should replace it |
|---|---|
| A straight line of volume over distance, in `spatial.rs` | Real attenuation curves, per kind of sound. `kira` has spatial tracks, with listeners, distances and attenuation functions |
| Stereo worked out by hand | The same spatial tracks, and whatever surround or binaural the output allows |
| Sounds stay where they started | Sounds that follow what made them, once entities reach the client |
| Water and wind measured to a box around each face, at one `ambient_level` | Sound from the surfaces themselves, with a level set per kind of sound |
| Sounds pass through walls | Occlusion, and reverb that suits the room |
| One track for effects, with a cap on each ear and no limiter | A proper mix, with buses, ducking and a limiter or compressor on the output |
| Fixed levels from 1996 | Settings the player can change, per bus |

### Entity culling

Every client is sent every entity, so a cheating client can see through walls, and traffic is higher than it has to be. `SV_WriteEntitiesToClient` in WinQuake and `SV_WritePacketEntities` in QuakeWorld send only what's in the client's fat PVS. To add to an increment, in this order:

| Step | What it covers |
|---|---|
| Research | How `SV_FatPVS` and `SV_LinkEdict`'s leaf lists work, and what's worth changing. Measure today's traffic on a real map first |
| Which leaves an entity is in | A box's leaves from the tree in `engine_map`, kept up to date as things move, rather than searched every tick |
| What a client can see | The union of the visible sets of leaves near the view, using `Tree::visibility`. A map with no visibility data shows everything |
| Filter each seat's snapshot | `Server::tick` builds what's shown once, and `snapshot` copies it to every seat. Each seat gets its own share, and the player always sees themself |
| Prediction's boxes | `solid_boxes` gives away positions too, so it needs the same filter, without taking away anything the player can touch |
| Movers | Decide if doors and lifts are culled at all. Prediction collides with `instances`, so one that's dropped mustn't leave the client walking through it |
| Leaving and coming back | Check what `delta_from` and the client do when something leaves the set and returns: no false removals, no sliding in from an old position, and projectiles on shared paths still drawn |
| Tests | Something behind a wall isn't sent, with a control in plain view. Nothing pops in at corners. Lag compensation and bots still work. Traffic is measured again, and netplay passes at 2% loss |
