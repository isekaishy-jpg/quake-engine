# Proposal 0098: light in corners

Status: accepted and implemented, 2026-11-05, increment 12.

## Research

- The human saw torches leave a separate circle on each of the two walls at a corner, with nothing between them.
- A surface's light was scaled by the cosine of the angle it's struck at. Beside a light that's near a corner, each wall is brightest where it's closest, and the corner itself is struck at a shallow angle, so it falls away. That's correct for a point in empty space, but a flame isn't a point, and in a real room the walls fill each other's corner with light.
- Compilers for this kind of lighting have long softened this. [ericw-tools' `_anglescale`](https://ericwa.github.io/ericw-tools/doc/light.html) defaults to 0.5, and [its code](https://raw.githubusercontent.com/ericwa/ericw-tools/master/light/ltface.cc) uses `(1 - scale) + scale * dot`.
- It also offers `_deviance`, a sphere of lights, and `_dirt`, which darkens corners. Our `size` does the first, and the second would work against what's wanted here.

## Design

- A light's strength on a surface uses `(1 - scale) + scale * cos`, with `angle_scale` on the light, defaulting to 0.5.
- Faces turned away from a light still get nothing from it.
- The sun stays at 1, so that outdoor forms keep their modelling. Near a corner it has no source to be close to.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** lights lose some shaping on nearby surfaces. A light can set `angle_scale = 1` for the old look.
- **Acceptance criteria:** a surface struck at a shallow angle keeps more than half of the light it would have head on, at the same distance, and the sun is unchanged.
- **Future extension:** none planned.

## Review

Seen in `ember_keep` and `vesper_deep`, where the human found it.
