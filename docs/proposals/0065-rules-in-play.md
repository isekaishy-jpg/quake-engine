# Proposal 0065: rules in play

Status: accepted, 2026-11-04, increment 10.

## Research

- `StartFrame` reads `teamplay` and `skill` every frame, and `CheckRules` reads `timelimit` and `fraglimit` when it runs, so changing them takes effect at once. The port read them when a level started.
- `trigger_onlyregistered_touch` only lets a registered game through. The port let everyone through.

## Design

- `Server::set_setting` passes a change to the scripts, and `StartFrame` runs at the start of each tick. The app passes on changes to those four settings.
- `deathmatch` and `coop` stay fixed for a level, as in `SV_SpawnServer`.
- A game that isn't registered shows the trigger's message and plays `misc/talk.wav`.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** none.
- **Acceptance criteria:** a rule changed in play takes effect on the next tick and not before, `skill` stays in range, and a registered trigger stops an unregistered game with its message.
- **Future extension:** none planned.

## Review

See [review 0013](../reviews/0013-increment-10.md).
