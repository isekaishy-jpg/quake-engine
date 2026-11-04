# Proposal 0037: how monsters move

Status: accepted, 2026-11-04, increment 7.

## Research

- `sv_move.c`: `SV_movestep` tries a step, pushing down from step height, and refuses edges with `SV_CheckBottom`. Flying and swimming monsters skip stairs, and rise or sink towards their enemy. `SV_StepDirection` turns first and only steps if it's facing the right way. `SV_NewChaseDir` tries the direct route, then either axis, then the old way, then all eight, then back.
- `SV_Physics_Step` (the second one, since the first is behind `QUAKE2`) only moves what's in the air, using `SV_FlyMove`. Everything on the ground moves by a script's `walkmove` and `movetogoal`.
- `SV_FlyMove` slides along up to four surfaces, and only a floor of the world or a brush model counts as ground.
- `PF_checkclient` keeps the PVS at the player's eyes for a tenth of a second, and says whether a monster's eyes are in it. It's what stops every monster tracing to the player every think.
- `PF_aim` bends a shot up or down at anything within `sv_aim`, 0.93, of the line of fire.
- `SV_PushMove` moves anything a pusher carries or runs into. Anything it can't move sends everything back, and a corpse is squashed flat instead.
- `SV_LinkEdict` touches triggers when anything moves, which is how monsters open doors and follow paths.
- `trace_inopen` and `trace_inwater` say what a trace passed through, so sight doesn't cross water.

## Design

- `engine_sim::walk` ports `sv_move.c`, `SV_FlyMove` and the falling half of `SV_Physics_Step`, and never calls a script. A hard landing is a new `Callback::Land`.
- Scripts pass the enemy and goal with each call, since those are game fields. `Target` holds the two, and `ideal_yaw` and `yaw_speed` are engine fields.
- `Simulation` has its own `rand`, so monsters repeat.
- `push_move` splits into the player and everything else, sharing one way back.
- `take_touches` adds triggers overlapping monsters.
- Traces report `in_open` and `in_water`, from the leaves they cross.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** triggers are checked for every monster each tick, where the original only checks on a move. It's the simplest thing that works, and a few hundred monsters stay cheap. Each trace still builds its list of brush models, which is the first thing to change if monsters ever show up in a profile.
- **Acceptance criteria:** tests cover walking, paths, plats, falling and landing, teleports and aiming. All 1888 monsters in `id1` wake, fight and die on their own maps with no script errors.
- **Future extension:** an area grid for traces and touches, shared with proposal 0034's.

## Review

See [review 0010](../reviews/0010-increment-7.md).
