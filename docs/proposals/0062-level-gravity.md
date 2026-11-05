# Proposal 0062: level gravity

Status: accepted, 2026-11-04, increment 10.

## Research

- `worldspawn` sets `sv_gravity` to 100 on `e1m8` and 800 everywhere else. The port had 800 written into the server, the client's paths and the particles, so Ziggurat Vertigo played at full gravity.
- Prediction runs the same movement on the client, so a client has to know the gravity or every jump there would be wrong.

## Design

- `engine.set_gravity` sets it on the simulation, which uses it for players, monsters and what flies.
- The server tells clients with `ServerMessage::Gravity` when it changes, and tells newcomers. Client paths and particles use it.
- A saved game keeps it. An older save without it gets 800.
- `sv_gravity` shows a level's gravity, and changing it at the console takes effect, as in the original. The next level sets it again.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** none.
- **Acceptance criteria:** a jump goes far higher on `e1m8` than on `e1m1`, clients are told, prediction stays with the server there, and a saved game keeps it.
- **Future extension:** none planned.

## Review

See [review 0013](../reviews/0013-increment-10.md).
