# Proposal 0063: the level clock

Status: accepted, 2026-11-04, increment 10.

## Research

- `SV_SpawnServer` starts a level at `sv.time = 1.0`. The port started at 0.
- That matters. QuakeC tells a field that was never set from one that has passed by comparing it with `time`. `teleport_touch` skips a teleport that's waiting to be used with `if (self.nextthink < time)`, and at time 0 that's `0 < 0`, which is false.
- So on the first tick every waiting teleport took whatever stood in it. On `end` that sent Shub and her guards through at once, and she was telefragged, which ended the game. Monsters in ambush closets on other levels went through early the same way.
- `trigger_multiple` and others also compare against `time`, and `trap_shooter` starts with a `nextthink` that's already past, so it fires at once.

## Design

- `Simulation` starts at `LEVEL_START_TIME`, which is 1.0.
- `SV_SpawnServer` also runs two frames of 0.1 s before anyone arrives. The port doesn't, since nothing depends on it.

## Proposal

- **Selected approach:** the root cause, rather than a change to `teleport_touch`.
- **Tradeoffs:** none. Saved games keep their own time.
- **Acceptance criteria:** a waiting teleport leaves a monster in it alone until it's used, and a control with the clock at 0 fails. On `end`, Shub is alive and in place, and her teleporter is there and moves.
- **Future extension:** none planned.

## Review

See [review 0013](../reviews/0013-increment-10.md).
