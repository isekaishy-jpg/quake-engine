# Proposal 0061: shootable triggers

Status: accepted, 2026-11-04, increment 10.

## Research

- `trigger_multiple` and `trigger_once` with a `health` key are solid, take damage, and fire when killed, by `multi_killed`. `multi_wait` sets them up again after their wait. A trigger with health and `notouch` is an error.
- The port left all of that out, so these fired when walked into, and couldn't be shot.
- The levels hold 29 of them, on 13 maps, including `start`, `e1m1` and the altar on `e4m6`.

## Design

- The port of `multi_trigger`, `multi_wait`, `multi_killed` and `trigger_multiple` now follows `triggers.qc`. `trigger_once` and `trigger_secret` get it through `trigger_multiple`.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** none. They block the player, as they do in the original.
- **Acceptance criteria:** a trigger with health fires when shot and not when walked into, comes back after its wait, and goes for good as a `trigger_once`. All 29 on the real maps take damage and fire.
- **Future extension:** none planned.

## Review

See [review 0013](../reviews/0013-increment-10.md).
