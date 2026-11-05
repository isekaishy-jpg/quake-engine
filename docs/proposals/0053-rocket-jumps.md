# Proposal 0053: predicted rocket jumps

Status: accepted, 2026-11-04, increment 9.

## Research

- The [networking record](../pre-research/networking.md) decided a rocket jump should be predicted, because otherwise the knock arrives a round trip late.
- `T_RadiusDamage` and `T_Damage` give the knock: 120 less half the distance, halved for your own rocket, times 8 by direction, from the blast point to your centre.
- `SV_ClipToLinks` doesn't clip a player against what they own. The port did, so a rocket a player had just fired counted as solid for them.

## Design

- A rocket's path says how many ticks it flies. The command it was fired on plus that many is the command it lands on. Counting commands is exact, where working from estimated times wasn't.
- Only your own rockets are predicted. The client checks reach and a clear line, then stores the knock against the command.
- The game supplies the numbers as a `BlastRule`, which the fixture fills in as `ROCKET_BLAST`, so nothing about Quake's rockets is in the engine. A client without one doesn't predict blasts.
- A knock is added before its command runs. On the server the blast comes after the command before it, so a snapshot answering that earlier command already holds the knock, and the client drops it then.
- The server's numbers always win. This only decides what's shown until they arrive.
- `solid_boxes` leaves out what the player owns, on both sides.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** other people's rockets aren't predicted, because nothing says when they were fired. A host's own rockets are only known from the server, which is exact with no network in the way.
- **Acceptance criteria:** with prediction, a rocket jump stays within a unit of the server at 40 and 80 ms, and at most half the error without it.
- **Future extension:** none planned. Quad damage and held fire followed in [proposal 0057](0057-the-feel-of-a-shot.md).

## Review

See [review 0012](../reviews/0012-increment-9.md).
