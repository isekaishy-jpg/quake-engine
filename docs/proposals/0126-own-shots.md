# Proposal 0126: how your own shots ease

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

Your own rocket has to do two things that pull apart. It has to leave the moment you fire, and it should hit what you see it hit. Your commands run a round trip ahead of the server, and what's on your screen is behind it, so those are two times, and the `gap` between them grows with ping.

[Proposal 0052](0052-projectiles.md) drew a shot at your commands' time less 8 ms, then eased it over 0.4 s back by half the gap. Both numbers were guesses, left for testing in play.

Feel needs a person, but what's behind it can be worked out. Easing back makes a shot look slower than it is, by the gap closed over the time taken. With 50 ms of interpolation, and the old fixed half:

| Ping | Gap | Half the gap, so how far off a hit looks | Rocket looks slower by |
|---|---|---|---|
| 30 ms | 80 ms | 40 ms, or 40 units | 8% |
| 80 ms | 130 ms | 65 ms, or 65 units | 14% |
| 150 ms | 200 ms | 100 ms, or 100 units | 23% |
| 300 ms | 350 ms | 175 ms, or 175 units | 42% |

So a half is the wrong shape at both ends. At a good ping it leaves a rocket 40 units off for no reason, when closing all of it would barely show. At a bad ping it visibly drags, and a rocket that leaves fast and then slows reads as a fault. People notice a change in speed from about a tenth to a fifth, and a rocket has only a moment to be looked at.

The 8 ms stays: it's under a tick of 13.9 ms, and well under a frame at 60 Hz.

## Design

- **Close all of the gap, as far as a fifth's slowdown allows.** `correction` is 1 now, and `MOST_SLOWDOWN` caps what's closed at 88 ms over the 0.4 s.
- **`cl_shot_correction`** changes the share from 0 to 1, without rebuilding, for whoever plays it. It only changes what's drawn.

| Ping | Gap | How far off a hit looks | Rocket looks slower by |
|---|---|---|---|
| 30 ms | 80 ms | None | 18% |
| 80 ms | 130 ms | 42 ms | 20% |
| 150 ms | 200 ms | 112 ms | 20% |
| 300 ms | 350 ms | 262 ms | 20% |

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** at a high ping a hit looks further off than it did, in exchange for a rocket that doesn't drag. Nothing about a hit changes, only where it's drawn.
- **Acceptance criteria:** a shot leaves within 8 ms at any ping, ends up with what you see on a good connection, and never looks more than a fifth slower.
- **Future extension:** none planned.

## Review

Tests hold each of those at pings from 0 to 500 ms. **Whether a fifth is the right number needs a person.** Those numbers come from what can be worked out, and `cl_shot_correction` is there to change it without a build.
