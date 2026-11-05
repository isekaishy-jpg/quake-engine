# Proposal 0090: pathing

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) decides the main items: waypoint splines for predefined movement, shown in the editor as a floating line with bubbles, and a regular navmesh good enough to keep enemies and NPCs clear of walls. The baseline is that they pass through doorways and nav tunnels without trying to go through walls. Upgrades such as jumping down come later. Bevy is out.
- `recast-rs` and `polyanya` find paths on polygon meshes, and generating those meshes is the hard part. `oxidized_navigation` generates them but is a Bevy plugin.
- Our clipping hulls already say exactly where a player-sized thing fits, which is the problem Recast's voxelising solves. A grid of standing places read straight from the hull needs no polygons, and can't disagree with collision. Stairs and stacked floors fall out of it. A grid's paths wander, so corners are cut where a straight walk stays on the grid.

## Design

- Baking walks a 16 unit grid over the world, finds each floor in each column from a clipping hull, and links neighbours a step apart. There's a grid for each hull, as [Unreal bakes one for each agent size](https://forums.unrealengine.com/t/how-do-i-set-up-navmeshes-for-characters-of-different-sizes/280867/2), and `Map::nav_for` picks by width as collision picks a hull. Diagonals need both sides open, so a path can't cut a wall's corner. Lava and slime aren't places to stand. It goes in the bake.
- `NavGrid::find_path` finds the nearest places, runs A*, and cuts corners where a straight line stays on linked places. Doors aren't in the hull, so paths go through them, as a doorway needs.
- `engine.find_path` gives scripts the points, for a width that defaults to a player's, and `engine.walk_to` uses the entity's own and steps an entity towards the next one, for a monster or NPC to call in its think.
- A map's `spline` block names a line of points, which can be `closed`. `Spline::point_at` is Catmull-Rom, so it passes through its points. `engine.spline_point` gives scripts a point on one.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** two sizes, because there are two hulls, and a bake holds a grid for each. A tunnel has to be a little wider than what walks it, as places sit on a 16 unit grid and need room round them.
- **Acceptance criteria:** a path goes through a doorway and round a wall and never through one, a sealed room has no path, an open room's path is straight, stairs are climbed, a stacked floor keeps its own places, lava is avoided, a doorway too narrow for a large monster has no way for one, and a spline passes through its points and wraps when closed.
- **Future extension:** jumps down, water and lifts, which the human set for later.

## Review

See the increment 11 review.
