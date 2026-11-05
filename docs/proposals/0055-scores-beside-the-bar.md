# Proposal 0055: scores beside the bar

Status: accepted, 2026-11-04, increment 9.

## Research

- `Sbar_MiniDeathmatchOverlay` lists scores to the right of the bar in deathmatch, on screens at least 512 wide, as many as fit, centred on the player's own line.

## Design

- `Hud::draw_mini_ranking` draws them 324 units from the bar's left edge, six lines from the top of the inventory, with each player's colours, frags and name, and marks your own.
- It's drawn with the bar and with the scoreboard, as in `Sbar_Draw`.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** on a 4:3 window the canvas is under 512 wide, so there's no list, as in the original at that size.
- **Acceptance criteria:** six lines of eight players, none on a narrow screen or outside deathmatch, and your own line always among them, wherever you rank.
- **Future extension:** none planned.

## Review

See [review 0012](../reviews/0012-increment-9.md).
