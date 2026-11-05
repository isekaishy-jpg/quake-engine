# Proposal 0051: the fair play zone

Status: accepted, 2026-11-04, increment 9.

## Research

- The [networking record](../pre-research/networking.md) sets the rules: a zone from a 20 ms floor to the lobby's mean plus 20 ms, delay only up to the floor, half the excess compensated up to a 90 ms ceiling, nothing above it, outliers left out of the mean, and all of it server settings.
- It matters most with player hosting, since a host has no ping at all.
- Ping has to be measured by the server. `renet`'s own figure wouldn't cover a simulated network, or a transport without one.

## Design

- `engine_server::fair` holds `FairSettings` and `Zone`, as plain arithmetic, tested against the record's tables.
- A seat notes when each snapshot went, and how long its answer took, less the tick that waiting for a command costs. The client's own say counts for nothing, and it can only make itself look worse.
- A level's first measure of a player's ping is the lowest of eight answers, because a client that's just finished loading can stall, and that's no measure of its network.
- Every `sv_fair_interval` seconds the zone is worked out again. Delay and compensation then move at 2 ms a second, so timing doesn't jump in a fight, except that a newcomer starts where they should be, and losing help at the ceiling is immediate.
- Added delay holds a seat's commands and snapshots, half each way.
- A shooter's rewind is limited to what's compensated, plus the interpolation buffer and a tick. Over the ceiling there's no rewind.
- `status` shows the rules, the zone and everyone's standing, and a player is told when they cross the ceiling.
- Rewind also takes doors, lifts and monsters back, which the record planned and proposal 0048 left out.

## Proposal

- **Selected approach:** the above, with the record's defaults.
- **Tradeoffs:** a host with others connected plays with 20 ms added.
- **Acceptance criteria:** the record's tables hold, the ceiling doesn't flicker, measured pings land near the truth, and bots show shots landing within the zone and missing above the ceiling.
- **Future extension:** `sv_unlag_proj`, and tuning all of this with real players.

## Review

See [review 0012](../reviews/0012-increment-9.md).
