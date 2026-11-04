# Proposal 0024: client and server over loopback

Status: accepted, 2026-11-04, increment 3.

## Research

- [The pre-research](../pre-research/stack.md#online-feel) sets the rules: client and server stay separate even offline, input is numbered commands kept in a history, server state carries tick numbers, and prediction uses the same code as the render tick.
- `cl_pred.c` starts from the last server state and replays every unacknowledged command.
- `renet` arrives with online play in increment 9, so this needs a transport interface now and a loopback behind it.
- Treating every client packet as hostile is a [standing rule](../pre-research/networking.md#security), so decoding has to refuse bad input, not panic.

## Design

- `engine_net` holds a `Transport` trait, `Loopback` pairs, and `ClientMessage` and `ServerMessage` with byte encoding. Messages go through bytes even locally, so there's one code path.
- `engine_sim::Simulation` owns the player and runs commands. `engine_server::Server` owns the simulation, reads commands and sends a snapshot each tick, with the last command it ran.
- `engine_client::Client` numbers commands, keeps those not yet acknowledged, and replays them on each snapshot.
- Each frame, `Client::render_state` copies the predicted state and runs movement for the time left over since the last tick, with the latest input. The copy is thrown away. This replaces interpolation for the local player.
- `noclip` and `setpos` travel as `ClientMessage::Console`, so the server stays in charge.
- `engine_app` loads the map for both sides. A map change message arrives with online play.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a local game encodes and decodes messages it could pass directly, which costs a few microseconds a tick, and keeps the path honest.
- **Acceptance criteria:** tests cover encoding, refusal of bad bytes, command history, replay after delayed snapshots, correction after a server change, and that the render tick leaves state untouched. On a real map you can walk, jump and swim.
- **Future extension:** `renet` becomes a second `Transport`. Entities, delta snapshots and rewind build on the snapshot.

## Review

See [review 0006](../reviews/0006-increment-3.md).
