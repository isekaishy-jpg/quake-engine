# Proposal 0027: porting QuakeC, map logic

Status: accepted, 2026-11-04, increment 4.

## Research

- [The pre-research](../pre-research/stack.md) says to port from GPL v1.01 in `reference/quake-tools/qcc/v101qc`, map entities first.
- This part covers `subs.qc`, `doors.qc`, `plats.qc`, `buttons.qc`, `triggers.qc`, and the parts of `misc.qc`, `world.qc` and `client.qc` that maps need.
- Those files are GPL version 2 or later, so a port is a derived work and keeps that licence. The human's decision is to carry the GPL over for the whole repository, so `LICENSE` holds id's text, and the workspace says `GPL-2.0-or-later`.

## Design

- The scripts go in `crates/fixture_quake/scripts/`, one per QuakeC file, each keeping id's notice, and are embedded in `fixture_quake`.
- Function names stay the same, and `self` and `other` become arguments.
- `sound` calls stay in place as a stub, so audio only has to supply the function.
- `T_Damage` is a stub until combat arrives, so blocked doors reverse but do no harm.
- Light styles move from Rust into `world.luau`, where they belong, and reach the client by message.
- `centerprint` goes to the terminal until text can be drawn.
- Level changes skip the intermission, which needs a HUD.
- `trigger_onlyregistered` treats the game as registered, because the registered check was dropped.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** key doors stay shut until items exist, so some levels can't be finished without noclip yet.
- **Acceptance criteria:** tests run the real scripts on a small map: a door opens and closes, a plat rises when stood on, a button fires its target, a trigger teleports, and a level changes. On `e1m1`, doors open and the exit changes level.
- **Future extension:** items, weapons, monsters and combat follow in later increments.

## Review

See [review 0007](../reviews/0007-increment-4.md).
