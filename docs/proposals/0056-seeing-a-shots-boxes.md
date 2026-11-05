# Proposal 0056: seeing what a shot was judged against

Status: accepted, 2026-11-04, increment 9.

## Research

- The [networking record](../pre-research/networking.md) asks for a debug view showing the client's box beside the server's rewound box, as Unlagged's `cg_drawBBox` did, because trailing is hard to diagnose without it.

## Design

- With `sv_unlag_debug` set on the server, a rewound shot sends `Effect::Outline` for each other player's box, as the shot sees it. The client marks its edges in red for two seconds.
- With the same cvar set on the shooter's own client, each shot marks the boxes on their screen in green. Two boxes that sit together mean rewind is finding what was shown.
- It's particles, so it needs nothing new from the renderer.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** every client sees the red boxes, which is fine for something only a host turns on.
- **Acceptance criteria:** with 60 ms of ping and a target at full speed, the judged box is within two ticks of running of the box on the screen, the same measure against a later view shows a real difference, and nothing is sent when it's off.
- **Future extension:** none planned.

## Review

See [review 0012](../reviews/0012-increment-9.md).
