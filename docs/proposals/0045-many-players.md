# Proposal 0045: many players

Status: accepted, 2026-11-04, increment 9.

## Research

- `Simulation` holds one `PlayerState`, one `player_id`, and one of everything a player has: view, buttons, impulse, weapon model and frame, knock, last command and touches. That's about 110 references.
- `Server` holds one transport, and everything from a script goes down it. Some of that is for everyone, like sounds. Some is for one player, like `sprint`, `centerprint`, `stuffcmd`, damage and a change of view.
- Scripts mostly pass `self`, so `the_player` only appears in five places. `checkclient` picks a target for monsters. `PF_checkclient` cycles through clients, checking one a tenth of a second.
- `sv_user.c` runs each client's commands as they arrive, and `SV_Physics` runs each client's `PlayerPreThink` and `PlayerPostThink` around them. Players collide with each other as solid boxes.
- `SV_SaveSpawnparms` keeps parms for each client, and clients carry them through a level change.

## Design

- `Simulation` holds `Vec<Player>`, a struct of everything that was one of. What took no argument takes the player's id. `Server` keeps one client for each player, with its own transport.
- Other players are solid boxes to a player's move, and traces and monsters see them as they see the one player now.
- `Game` keeps a table for each player. `the_player` becomes `next_client`, which cycles as `PF_checkclient` does. `connect_player` and `disconnect_player` run `ClientConnect` and `ClientDisconnect`.
- `Outbox` items for one player say who. Everything else goes to everyone, with `own` worked out for each listener.
- A level change hands every client's transport and parms to the next `Server`.
- Saving stays for one player, as the original blocks it with more.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** every method that touched the player takes an id, so a lot of lines change at once. The alternative of giving the one player a special case would have left two paths through everything.
- **Acceptance criteria:** with two clients on loopback, each moves on their own commands, blocks the other, gets their own messages, and hurts the other. All single player tests still pass, and saves still go on identically.
- **Future extension:** spectators, and bots, which are clients with no transport.

## Review

See [review 0012](../reviews/0012-increment-9.md).
