# Proposal 0049: bots and bad networks

Status: accepted, 2026-11-04, increment 9.

## Research

- The human's direction: find or create bots to simulate players, and add latency to test.
- The [networking record](../pre-research/networking.md) says its numbers come from a simplified model and need tuning with real players. Nothing online has been measured under latency.
- QuakeC bots, such as the Reaper and Frikbot, live on the server, as entities that skip the network. That wouldn't test the network at all, and they aren't under this project's control.
- A proxy on real sockets runs in real time, so a minute of play takes a minute, and can't repeat a run.

## Design

- `engine_net::Conditioned` wraps any transport in delay, jitter and loss. Time only passes through `update`, so tests run many minutes of network in seconds, and the same seed gives the same losses. Reliable messages keep their order.
- `engine_client::Bot` is a real `Client` with a simple brain. It knows only what a player's client knows, through the same messages, and aims at what it draws. So bots exercise prediction, deltas, the view clock and rewind.
- `engine_app/tests/netplay.rs` plays bots on real maps over simulated networks and measures them.
- `addbot [ping]` and `removebots` put bots in a real game, so one person can test alone. `net_fake_latency`, `net_fake_jitter` and `net_fake_loss` apply to a real `connect`.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the bots are test partners and sparring, not good opponents. They wander and have no idea of the map.
- **Acceptance criteria:** a target running at about 320 u/s is hit by aimed shots at every ordinary ping with rewind, and the same run without it misses, so the result can't be an accident of the test. Measured results are in [review 0012](../reviews/0012-increment-9.md).
- **Future extension:** bots that know the map, and loss in bursts, which real networks have.

## Review

See [review 0012](../reviews/0012-increment-9.md).
