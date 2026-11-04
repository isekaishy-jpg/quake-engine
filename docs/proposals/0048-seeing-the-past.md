# Proposal 0048: interpolation and rewind

Status: accepted, 2026-11-04, increment 9.

## Research

- `draw_entities` blends between the last two snapshots, and assumes one arrives every tick. Online, they arrive unevenly, and some don't.
- The [networking record](../pre-research/networking.md) recommends about two ticks of buffer, tunable, and hit detection that rewinds to the client's exact view time, built in from the start.
- `G_TimeShiftClient` in Unlagged interpolates between the two stored positions around the shot time. Its documented weaknesses are shots around corners, and that movers don't rewind. The record plans a cap on rewind, and rewinding movers.
- A client has to say what time it was looking at, or the server has to guess from ping.

## Design

- The client keeps recent snapshots by server time, and draws others at a render time that runs `cl_interp` behind the newest, default two ticks. It's stretched gently to keep the buffer level, rather than jumping.
- The local player stays predicted.
- `UserCommand` carries the snapshot time the client was drawing. The server keeps each player's box for 250 ms.
- While a hitscan weapon fires, `Simulation::rewind` moves other players to where that client saw them, interpolating, and back afterwards. It's capped at 250 ms, so old views can't reach far.
- Only `traceline` from a player's weapon sees it, through `engine.rewound`.

## Proposal

- **Selected approach:** the above, for players.
- **Tradeoffs:** a player can be hit just after reaching cover, the known price of favouring the shooter. Doors and platforms don't rewind yet. Monsters don't either, since co-op doesn't need it as much.
- **Acceptance criteria:** tests show the buffer holding steady through gaps and jitter, and a shot at where a target was drawn 100 ms ago hitting, one 400 ms ago missing, and the target being back in place afterwards.
- **Future extension:** the fair play zone, weapon sub-tick, half tick, launch events for projectiles and predicted rocket jumps. Those need tuning with real players, and wait in the plan.

## Review

See [review 0012](../reviews/0012-increment-9.md).
