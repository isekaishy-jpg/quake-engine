# Research plan

Each item below goes through the [workflow](../../DESIGN.md#workflow) on its own. Research covers how to do it: the technical details, sources, constraints and evidence. The [pre-research decisions](../pre-research/README.md) are inputs, not answers.

The order is a first draft. Earlier items unblock later ones.

## Increment 0: repository and build

Done, in proposals 0001 to 0009, apart from:

| Item | What research has to establish |
|---|---|
| Luau formatting | Which tool enforces `docs/style.md`, such as `StyLua` in `xtask` |
| Sol crates under the workspace lints | About 800 warnings to work through |

## Dependencies

One item each, covering fit, version, licence, how it's integrated, and how it's kept up to date: `vulkane`, `box3d`, `mlua` with Luau, `kira`, `assets_manager`, `qbsp`, `renet`, FlatBuffers through `planus`.

## Engine foundations

| Item | What research has to establish |
|---|---|
| App host integration | How the engine sits on `solapp`, and what stays free of it |
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
