# Proposal 0113: moving lights' shadows

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

The human saw sawtooth and bands from moving lights, mostly on curved surfaces, and expected some mix of soft shadows and shadow volumes. [The note](../research/shadow-rendering.md) has what a test level found.

- Bands come from light, not shadows: each flat side of a ring faced the light by its own amount. Item 8 gave meshes normals, and moving lights weren't using them.
- Sawtooth comes from a shadow map's pixels on a surface nearly edge on. A blur narrower than a stair step can't take it out.
- Shadow volumes are exact, but sharp, and need closed geometry. Meshes don't have to be closed, and sharp isn't what was asked for.
- Variance shadow maps can be blurred ahead of time, which is cheaper to read. They need a second atlas, in floats, with light leaks to manage, for what a wider tent also gives.

## Design

- **Light follows a mesh's normal.** `live_light` uses a vertex's normal where there is one, so a curve has no bands. Shadows still go by the flat surface, as that's what they're drawn from.
- **A wider tent.** Sixteen samples in place of nine, each already blended by the sampler, and no noise, as noise on a surface reads as specks.
- **Soft with distance.** Samples spread from 1.5 shadow pixels apart near a light to 5 far off, as a real light has some size. That takes out stair steps where they're largest.
- **Looking from further off the surface.** A wider look's far samples fall below a slanted surface, which showed as fine stripes on a floor. The lift now grows with the spread.
- Taps stay inside a tile, by how far they reach.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a shadow near what casts it is no softer than before, as there's no search for what's blocking. It costs more to read: 0.45 ms at about 2560 by 1400 with a whole yard lit, or 3.33 ms in place of 2.89.
- **Acceptance criteria:** a curved mesh has no bands under a moving light, shadows are soft and have no sawtooth, and surfaces don't speckle or stripe.
- **Future extension:** none planned.

## Review

A ring of 16 sides under a moving light, with a post beside it, looked at from both sides. The hard step at each side is gone, the far floor is clean, and shadow edges are soft. A first try at a wider tent striped the far floor, which is why the lift grows.
