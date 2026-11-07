# Proposal 0130: going back on shots and rewind

Status: accepted and implemented, 2026-11-06, the [work list](../research/work-list.md), items 149 and 150.

## Research

An audit against what the human had said found two changes in the sprint that went against it.

**Own shots.** The human's words, in setting up projectiles, were to aim for half of the gap, as closing all of it might be too much. [Proposal 0126](0126-own-shots.md) called half a guess, and made all of it the default. It was a stated aim, not a guess.

**The rewind allowance.** Fair play has a ceiling because compensation can be abused. [Proposal 0128](0128-network-numbers.md) let a shot reach 114 ms past a compensated ping, up from 42 ms, for everyone, all the time, so that clients on bad connections could still hit. Its note said that a client asking for more than it draws "only gets to shoot at the past", and that this gives nothing away. That's wrong. Shooting at the past is what a cheat wants: it hits someone who's already behind cover.

The burst tests still need the reach. So it has to depend on something a client can't just say.

- A command's `view_time` is what the client claims, so it can't be the evidence.
- The server knows when a client said it had each snapshot. On a bad connection those stall. A client that holds them back to gain reach puts up its own measured ping, which moves it towards the ceiling, where it gets nothing.

## Design

- **Half.** `DEFAULT_SHOT_CORRECTION` is 0.5, in one place, and the cvar's default comes from it. `MOST_SLOWDOWN` stays, and `cl_shot_correction` still changes it.
- **`ViewSlack`.** Each seat keeps the longest it's waited past a tick for word of a new snapshot, halving over six seconds, as a client's view clock does. A shot reaches a compensated ping, plus 3 ticks, plus that wait, and never past `MOST_VIEW_DELAY` and a tick. It starts afresh on a new level.
- Over the ceiling a shot still reaches nothing.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a stall in commands reaching the server counts as well as one in snapshots, which gives a little more reach than the view needs for a few seconds. That errs towards a bad connection's shots landing.
- **Acceptance criteria:** a default shot closes half of the gap. A clean connection reaches what it did before proposal 0128. A connection that really stalls reaches further for a while and then stops, and shots still land in bursts.
- **Future extension:** none planned.

## Review

`by_default_a_shot_closes_half_of_the_gap` holds the first, and three tests in `fair.rs` hold the rest. `shots_land_where_they_were_aimed_at_every_ordinary_ping` still passes with bursts. With `heard` taken out, so that no stall is ever noticed, it fails, so the reach is doing what it's there for.

Whether a cheat can gain anything by delaying its acknowledgements was reasoned through, and not attacked for real.
