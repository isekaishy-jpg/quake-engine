# Proposal 0046: the network

Status: accepted, 2026-11-04, increment 9.

## Research

- The [networking record](../pre-research/networking.md) decided on `renet` and `renet_netcode` behind our own interface, with our own replication. It was never run here.
- A full snapshot is about 40 bytes an entity, which is around 20 KB on a busy map, 72 times a second, to each client. That's over a megabyte a second, so deltas aren't optional.
- QuakeWorld sends each command three times, so one lost packet costs nothing. Its deltas are against the last frame the client acknowledged.
- `renet_netcode` has an unsecured mode for direct play, and secure mode needs connect tokens from a backend. External servers are out of scope, so there's no backend.
- A client that connects needs the map's name before anything else, and has to load it. So does a level change.
- The record warns that every client packet is hostile. Sizes, rates and values all need limits.

## Design

- `Transport` gains `Channel::Reliable` and `Channel::Unreliable`. `Loopback` keeps both in order. `engine_net::online` holds `NetServer` and `NetClient` over `renet`, private to `engine_net`. A `NetServer` hands out a `Transport` for each client that connects, and notices those that go.
- Snapshots and moves are unreliable. Sounds and effects are unreliable, as in the original. Everything else is reliable.
- A move carries the last three commands, and the last snapshot received. The server keeps 64 recent snapshots for each client, and sends what differs from the one acknowledged, or everything if it has none.
- `ServerMessage::Level` tells a client to load a map. It answers with `ClientMessage::Ready`, and only then gets a player.
- A client that goes quiet for ten seconds is dropped, and a client's view of the server going quiet returns it to the menus.
- Commands are limited: a client's clock can't run faster than real time by more than a little, and bad messages are dropped without an error.
- The console's `host` and `connect` start each side.

## Proposal

- **Selected approach:** the above, unsecured, for direct connections.
- **Tradeoffs:** without tokens, traffic isn't encrypted and anyone who reaches the port can join. The record allows this for LAN and development. It has to change before this is used on open servers.
- **Acceptance criteria:** two clients play over real UDP in one test, through a transport that loses a fifth of its packets. Deltas are a small part of a full snapshot on a real map. Malformed messages don't hurt the server. Traffic is measured, which the record lists as unknown.
- **Future extension:** connect tokens, relay and NAT traversal with the later server work, and `renet_steam` if that's ever wanted.

## Review

See [review 0012](../reviews/0012-increment-9.md).
