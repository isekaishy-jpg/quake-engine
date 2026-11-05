# Proposal 0079: small fires

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) puts torches in a category of their own: fixed micro lights, done with a precompiled shader and maybe particles, not the budget of moving lights.
- The torch prefab's `prop_flame` entity spawned nothing.

## Design

- A `flame` entity compiles into crossed quads, seen from both sides, with no collision. Its material's flipbook animates it in the shader, so a flame costs no script and no snapshot traffic.
- It also records an `Emitter`, with how far its light reaches and how many sparks it gives off.
- The view takes the nearest eight within reach. Each gets a small dynamic light that flickers on its own rhythm on top of the baked light, and sparks come through the existing particles.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** crossed quads show their edges from directly above. Far flames still animate, as faces, but don't flicker or spark.
- **Acceptance criteria:** a torch prefab shows an animated flame with sparks and flicker, nothing collides with it, and a flame out of reach costs no light.
- **Future extension:** none planned.

## Review

See the increment 11 review.
