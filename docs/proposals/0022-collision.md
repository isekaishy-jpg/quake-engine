# Proposal 0022: collision on hull solids

Status: accepted, 2026-11-04, increment 3.

## Research

- [The pre-research](../pre-research/stack.md#physics-on-box3d) decided on `box3d` with hull solids. Each solid leaf of a BSP clipping hull is a convex region, and a ray through hull 1 equals a box sweep of the player. On 100,000 traces per map, rays matched Quake's trace on every hit once Quake's surface margin was removed.
- `box3d-rust` 0.4.0 is MIT, with no dependencies, and the `test` project carries it. `create_hull` takes points, with a limit of 128 vertices.
- `world_cast_ray_closest` ignores a ray that starts inside a shape, and a hit at fraction 0. A rough mover built on it ended up inside solid in 6 to 13% of checks, which is an [open question](../research/open-questions.md).
- `pmovetst.c` keeps movement out of solid with `DIST_EPSILON`, 1/32 of a unit: a trace stops that far short of a plane. It tests the start point against the hull tree first, and reports `startsolid` and `allsolid`.

## Design

- `box3d` goes in `vendor/box3d`, private to `engine_physics`.
- `engine_map` gains clipping nodes and each model's hull roots, with no format in them. The importer fills them.
- `engine_physics::build_solids` walks a hull tree, clipping a bounding box by each plane, and returns each solid leaf's points. It works in `f64`.
- `CollisionWorld` loads solids into a static `box3d` body. `trace` first asks the hull tree if the start is solid, which covers what rays can't see. It then casts a ray, and stops `DIST_EPSILON` short of the plane it hits, using Quake's formula.
- `contents` reads the map's tree, for water.
- Brush models aren't solid yet. Doors and lifts arrive with entities in increment 4, as kinematic bodies.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a trace does a tree walk and a ray, slightly more than Quake's single walk. The pre-research measured the ray at under a microsecond. Client and server share one `CollisionWorld` in a local game, to avoid building it twice.
- **Acceptance criteria:** tests on small maps cover solids, traces, the margin and starting in solid. On real maps, including BSP2, traces match a port of Quake's own `SV_RecursiveHullCheck` in a test, and every solid builds.
- **Future extension:** entity hulls join as kinematic bodies. Maps with real brushes can supply solids directly.

## Review

See [review 0006](../reviews/0006-increment-3.md).
