# Proposal 0001: engine architecture and the road to playing Quake

Status: accepted by the workflow, 2026-11-03, increment 0. Inputs: [DESIGN.md](../../DESIGN.md) and the [pre-research decisions](../pre-research/README.md).

## Goal

A Rust engine that plays the Quake fixture from start to finish, built so any system can later be changed or replaced. Playing Quake is how we know it works.

## Research

- **Layered engines.** Gregory's runtime architecture, as [reproduced in an architecture study](https://arxiv.org/pdf/2309.06329), puts third-party SDKs and a platform independence layer at the bottom, with systems above and game code on top. [Cross-platform practice](https://www.gamedeveloper.com/programming/simultaneous-cross-platform-game-development) isolates platform dependencies at the lowest level, so higher layers never touch platform types.
- **Coupling is the known risk.** [A study of 10 open-source engines](https://deepai.org/publication/visualising-game-engine-subsystem-coupling) found the low-level renderer, platform layer and resource manager frequently coupled to the engine core.
- **Rust workspaces.** [Large Rust Workspaces](https://matklad.github.io/2021/08/22/large-rust-workspaces.html) recommends a flat `crates/` layout, a virtual manifest, and automation in Rust through `xtask`.
- **Local precedent.** `populous-reborn` uses one crate per system, a private backend per crate, a composition root, and a permanent test that enforces the crate graph. It has worked through 113 proposals.
- **The original's own split.** Quake separates client from server, and game logic from engine, which fits the online rules in the [pre-research networking record](../pre-research/networking.md). Its shared-memory and global-state style is what we leave behind.

## Design

### Encapsulation rules

1. **One crate per system,** with one deliberate public API.
2. **Third-party libraries stay private to one crate.** No `vulkane`, `box3d`, `mlua`, `kira`, `renet`, `solapp` or `qbsp` type appears in another crate's API.
3. **The engine doesn't know Quake.** Only `fixture_quake` understands Quake's formats. Engine crates work on engine types, so a new format or game is a new importer.
4. **Dependencies point towards foundations,** never upwards.
5. **The simulation doesn't know presentation.** Server-side crates never see rendering, audio, UI or the platform.
6. **Client and server stay separate,** connected through a transport even offline.
7. **Only the composition root knows everything.**
8. **Rules are enforced by a test,** not by discipline ([proposal 0008](0008-crate-boundaries.md)).

### Systems

| Layer | Crates |
|---|---|
| Foundation | `engine_core` |
| Services | `engine_console`, `engine_assets`, `engine_platform`, `engine_net` |
| World | `engine_map`, `engine_physics`, `engine_movement` |
| Game | `engine_sim`, `engine_script`, `engine_server` |
| Presentation | `engine_client`, `engine_render`, `engine_audio`, `engine_ui` |
| Fixture | `fixture_quake`, plus the ported game scripts |
| Composition | `engine_app` |

## Proposal

**Selected approach:** the rules and systems above, delivered in increments that each end with something runnable.

| Increment | Delivers | You can |
|---:|---|---|
| 0 | Repository, workspace, checks, boundaries | Build and check it |
| 1 | Window, main loop, console, packs | Open a window and browse Quake's files |
| 2 | Map importer and world rendering | Fly around real maps |
| 3 | Collision, movement, client and server over loopback | Walk around real maps |
| 4 | Luau host, entities, doors, lifts, triggers | Open doors and change levels |
| 5 | Audio | Hear the game |
| 6 | Models, particles, effects, weapons, shootable buttons and doors, used button textures | Shoot things |
| 7 | Monsters and the rest of the QuakeC port | Fight through levels |
| 8 | HUD, menus, saves | **Play Quake from start to finish** |
| 9 | Online play | Play with others |

**Tradeoffs:** many crates exist before they have code. That's deliberate, so code can't grow in the wrong place. Crate boundaries add some API work, which is the price of being able to swap systems.

**Acceptance criteria:** the engine plays the fixture from the start of the game to the end, and every rule above is enforced by a passing test.

**Future extension:** a swapped backend changes one crate. A new map format or game is a new importer. A new crate needs a proposal and a rule.

## Review

- The first draft had `engine_client` depending on `engine_sim` for movement. Prediction only needs movement, so movement became its own crate that both sides use.
- The increment table is a plan. Each increment still takes each of its items through the workflow.
