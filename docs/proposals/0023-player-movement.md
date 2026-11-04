# Proposal 0023: player movement

Status: accepted, 2026-11-04, increment 3.

## Research

- [The pre-research](../pre-research/stack.md#fixed-tick) requires movement to be a pure Rust function of state, input, time and collision, with jumping inside it, and small state with no hidden globals. Prediction and the render tick both depend on that.
- `pmove.c` from QuakeWorld is that function already: friction, ground, air and water movement, stair stepping, jumping, water jumps and nudging out of solid.
- WinQuake's values differ from `pmove.c`'s commented defaults. Friction is 4 in `sv_phys.c`, and these are the fixture's numbers.
- The pre-research's own model gives targets at 72 Hz: a jump height of about 43.7 units and a stop from 320 in about 63.8 units.
- `gl_rmain.c`'s neighbour `view.c` smooths the view after a stair step, at 80 units a second, up to 12 units. Without it, stairs jump.

## Design

- `engine_core::player` holds `PlayerState` and `UserCommand` as plain values, because `engine_net` carries them and can't see `engine_movement`.
- `engine_movement::player_move` ports `PlayerMove`. All of its inputs are arguments: the state, the command, `MoveVars` and the `CollisionWorld`.
- `noclip` is a state flag, and it replaces `FlyCamera`.
- Commands always run: a held key asks for 400 units a second, which clamps to `maxspeed`, and `+speed` walks at half that. Moving at walking pace by default is from before always-run was the norm.
- `engine_client::StepSmoother` ports the view smoothing.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** `pmove.c`'s edge friction trace is used, not WinQuake's point trace, which keeps movement to one hull.
- **Acceptance criteria:** tests on small maps cover the jump height, stop distance, sliding, stairs, air control limits, water and noclip. On real maps, random input for a long run never leaves the player in solid, which closes the open question.
- **Future extension:** per-player `maxspeed` and gravity join `MoveVars` when scripts need them.

## Review

See [review 0006](../reviews/0006-increment-3.md).
