# Proposal 0050: weapon timing

Status: accepted, 2026-11-04, increment 9.

## Research

- The [networking record](../pre-research/networking.md) decided on exact weapon timing inside the tick, a half tick, and sending fire commands on the click. Together they take fire rates to their true values and save about 10 ms.
- Nails and lightning repeat every 0.1 s, which rounds up to 8 ticks and 9 shots a second. `SV_RunThink` sets `time` to a think's exact time, which the port didn't do. That alone rounded everything that repeats.
- Quake processes clients in slot order, so a low slot wins a trade on the same tick.

## Design

- `Callback::Think` carries the think's exact time, and scripts see it, as the original's do.
- `refire` counts from when a held weapon was due, not the tick that noticed.
- `ClientMessage::Fire` goes out from the input event itself, in its own small packet, with the view, the view time, a byte for where in the tick it fell, and a seed for randomness.
- `Server::half_tick` runs midway between ticks. It reads waiting shots and runs them, and nothing else moves. What else arrives waits for the tick.
- Shots run in the order they were fired, not by seat. A client can claim the start of a tick and no earlier, so the most it gains is one tick.
- The command behind a shot finds the weapon busy, so nothing fires twice. If the packet's lost, the command fires it instead.
- A shot's rewind uses the shooter's own exact position too.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** two shots in one instant don't both land: the first still wins, in time order rather than seat order. Weapon code can run between ticks, so it has to use `time()`.
- **Acceptance criteria:** nails reach ten a second. A shot lands in the half tick, before the next tick. Fire can't beat the refire time or fire twice. The earlier shot of two wins, whichever seat it's from.
- **Future extension:** both of two simultaneous shots landing.

## Review

See [review 0012](../reviews/0012-increment-9.md).
