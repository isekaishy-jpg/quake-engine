# Proposal 0032: entities on the client

Status: accepted, 2026-11-04, increment 6.

## Research

- Only brush models reach the client today. Everything else in Quake is an entity with a model, a frame, a skin, angles and effects.
- `SV_SendClientDatagram` sends them by difference from a baseline, and the client's `CL_RelinkEntities` interpolates between the last two messages.
- `PF_setmodel` gives a non-brush model a box of 32 units each way, until `setsize` changes it, which ported code always does.
- `makestatic` hands an entity to the client and frees it, which only saved bandwidth in 1996.
- Model files are named by text. The server sends a list once, and numbers from then on.
- The feature catalogue names the v1.01 QuakeC as the port source. Increment 4 read `qw-qc`, but its doors, plats and triggers differ from v1.01 in licence headers and networking calls, not logic, so they stand. New ports read `qcc/v101qc`.

## Design

- `engine_core::entity::EntityState` holds an id, a model number, frame, skin, effects, origin and angles.
- `Simulation` keeps a table of model names, and `Entity` gains those fields, which scripts reach through the prelude.
- `Snapshot` lists every entity with a model, in full each tick. `ServerMessage::ModelName` gives a number its name, once.
- `Client` keeps the last two lists, matches them by id, and interpolates position and angles. A jump of over 200 units is a teleport, and isn't blended.
- `makestatic` stays an ordinary entity for now.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** full lists each tick cost more than deltas, which doesn't matter on loopback and has to change for online play. Every entity is sent, with no check of what the player can see.
- **Acceptance criteria:** tests cover the messages, model numbers, interpolation by id, and teleports. On a real map, torches and items appear.
- **Future extension:** delta encoding and visibility culling, with online play.

## Review

See [review 0009](../reviews/0009-increment-6.md).
