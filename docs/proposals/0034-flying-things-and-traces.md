# Proposal 0034: flying things and traces

Status: accepted, 2026-11-04, increment 6.

## Research

- `SV_Move` clips against the world, then every solid entity, with rules on who ignores whom: an entity never hits itself, its owner, or what it owns.
- `SV_HullForEntity` picks a hull by size: the point hull under 3 units, the player's up to 32, and the large one above. Entities are boxes grown by the mover's size.
- The player's own collision uses `box3d` solids of hull 1. Rockets are points, so they need hull 0, the drawing tree itself, and only `Contents::Solid` stops them. Sky doesn't, so `T_MissileTouch` checks for it.
- `SV_RecursiveHullCheck` is short, and the tests already hold a port of it as a reference.
- `SV_Physics_Toss` serves `TOSS`, `BOUNCE`, `FLY` and `FLYMISSILE`: think, gravity, turn, `SV_PushEntity`, clip velocity, stop on floors, and splash on crossing water. `MOVE_MISSILE` grows monsters to 15 units each way.
- `SV_Impact` runs both entities' `touch`. A hit on the world gives `other` the world.

## Design

- `engine_physics::hull` ports `SV_RecursiveHullCheck` over either kind of tree. `Collision::move_box` picks a hull by size, and checks the world and brush instances.
- `Model` keeps the root of its drawing tree.
- `Simulation::trace_move` adds entity boxes and the ignoring rules, and names what it hit. A brush instance maps back to its entity.
- `engine_sim::flight` ports the toss physics, returning touches as callbacks like everything else.
- `Solid` gains `Bbox` and `SlideBox`, and `MoveType` the flying types. Entities gain an owner, flags and angular velocity.
- `Callback::Touch` takes an optional other, where none is the world.
- The player's moves stay on `box3d`, and gain solid boxes, so a player can't walk through a crate.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** every trace checks every solid entity, where Quake used area nodes. Hundreds of entities won't notice, and a grid can come later.
- **Acceptance criteria:** tests cover each hull, the ignoring rules, bouncing, stopping on floors and water. A real map's traces match the reference.
- **Future extension:** an area grid, and `MOVETYPE_STEP` with monsters.

## Review

See [review 0009](../reviews/0009-increment-6.md).
