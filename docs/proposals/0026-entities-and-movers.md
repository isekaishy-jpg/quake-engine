# Proposal 0026: entities and moving brushes

Status: accepted, 2026-11-04, increment 4.

## Research

- [The pre-research](../pre-research/stack.md#entity-storage-research) decided on a generational slot table: lowest free slot, opaque ids, and slot order as a contract.
- `ED_Alloc` reuses a slot once it's been free for 0.5 s, and relaxes that in the first 2 s of a level, when a lot is freed and allocated.
- `SV_Physics_Pusher` and `SV_PushMove` move a pusher on its own clock, `ltime`, stop at its `nextthink`, carry what's on it or in its way, and put everything back and call `blocked` if something can't move.
- `SV_LinkEdict` grows a box by 1 unit before checking triggers. `SV_Impact` calls `touch` on whatever a move hits.
- An [open question](../research/open-questions.md) asked how doors and lifts work as `box3d` bodies. Movement has to stay a pure function that can replay on a copy, so its collision can't be state that the server changes.

## Design

- `engine_sim` gets `SlotTable`, `Entity` and pusher physics.
- `engine_core::entity::ModelInstance` says where a brush model is, whether it's drawn and whether it's solid.
- Each brush model gets its own small `box3d` world, built once at its original position. A trace to a moved model is a ray offset by the model's position. `CollisionWorld` therefore stays immutable, and `Collision` pairs it with a list of instances, which movement takes as an argument. That answers the open question: no kinematic bodies, and replay sees exactly the positions it's given.
- `player_move` reports what it hit, and `PlayerState` remembers which model it stands on, so a lift can carry the player.
- Only the player is pushed, because nothing else moves yet.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** each trace also casts against every nearby model, which a bounds check keeps small. Rotating brushes aren't covered, and Quake doesn't use them.
- **Acceptance criteria:** tests pin slot order, reuse and stale ids, and cover moved models, carrying, blocking and thinks on `ltime`.
- **Future extension:** monsters and items join the same table and are pushed the same way.

## Review

See [review 0007](../reviews/0007-increment-4.md).
