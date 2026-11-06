# Proposal 0122: paths that drop, jump, swim and ride

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

[Proposal 0090](0090-pathing.md) left jumps down, water and lifts for later. A path could only follow links between places a step apart, so a ledge, a gap, a pool or a lift ended the world as far as anything walking was concerned.

- Recast and Unreal call these off-mesh links: one-way connections, added beside the mesh, each with a cost and a kind of movement. That's the shape here too, as it leaves the grid and its links alone.
- A walk link can be found from next to a place. A leap needs a look further out, so it comes after the grid is built.
- Every place along a ledge could drop. Keeping them all would mean thousands of links that say the same thing.
- Quake's own numbers set the limits: a fall hurts from about 264 units, a jump reaches about 45 up and about 200 across at a run, and something can climb out of water onto about 48.

## Design

- **`NavLeap`**: a way from one place to another with a `NavWay`, one of drop, jump, swim or lift. A grid holds them apart from its places, in order of where they start, and a grid in chunks holds all of its own too, as they're few.
- **Search** treats a leap as a link with a cost added: 32 for a drop, 96 for a jump, 192 for water and 256 for a lift. A walk that isn't much longer wins.
- **Drops** go off an edge, up to 224 units. **Jumps** cross up to 160 units, landing up to 40 higher, and the arc has to be clear.
- **Water.** A place too deep to stand in has no place on its bed. It has one afloat on top instead, and those link to each other. What's afloat only joins dry land by a leap, so swimming costs what it should, even at a low edge.
- **Lifts.** The hull doesn't hold a `func_plat`, so its shaft's floor and the ledge at its top are both places, and a leap joins them, going up. Going down is an ordinary drop once it's down.
- One leap of a kind is kept for each square of 4 cells.
- `find_route` says how each point is reached, and `find_path` still gives the points. Corners are only cut along a walk.
- **`engine.walk_to` does all of it.** Off an edge or over a gap, it throws the entity in an arc that goes up first and lands a little past the place, as a place can be right at an edge. Afloat, it moves in a straight line. At a lift, it asks `engine.call_lift`, which the game sets, and waits.

Trains aren't part of this. They run to their own timetable and something can't call one, so whether one's there depends on when you ask, and a path can't promise that.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a route is worked out as if nothing's in the way, and doors and lifts are moving parts, so `walk_to` works it out again each time it's called.
- **Acceptance criteria:** something really goes off an edge, over a gap, through a pool and up a lift to reach a goal, a way back up a drop doesn't exist, a gap too wide isn't jumped, nobody walks along the bottom of deep water, and a dry way round beats a swim that saves little.
- **Future extension:** none planned.

## Review

Each of the four is tested with a real walker in a real server, not just with a route. Those tests found what route tests couldn't:

- A short drop aimed straight at somewhere lower pressed the walker into its own ledge. The arc goes up first now.
- Walking won't go right to an edge, so a leap starts from close to it, and landing right on a place at an edge left it hanging, so it lands past it.
- A corner landing exactly on the seam between a lift and a ledge finds no ground and refuses the step, as Quake's `SV_CheckBottom` does. `walk_to` tries a shorter step.

**Three real faults came up on the way, and are fixed.**

- **The nearest place could be on the roof.** A search for the nearest place stopped at the first ring with any place in it, however far above or below. A walker at the edge of a pool got a place on top of the level, and so no way anywhere. It now goes on looking until it finds one within 48 units.
- **A player touched what's at the middle of the world.** What's stored for a player's entity stays at 0 0 0, and a level asking everything to touch again counted that. Joining a level set off a lift from 1,400 units away, and any trigger at the middle of a world would do the same. Players touch triggers as they move, so that pass now skips them. `a_player_far_away_does_not_touch_what_is_at_the_middle_of_the_world` fails without the fix.
- **`find_path` and `walk_to` used to agree on walking only.** Everything that called `walk_to` now gets leaps with nothing to change.

The baked places outside a level, such as on its roof, are still there. They're unreachable, so they cost a little space and nothing else.
