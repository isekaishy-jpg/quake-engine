# Proposal 0093: what increment 11 left

Status: accepted, 2026-11-05, increment 12.

## Research

- The report on increment 11 named what wasn't done, and the human asked for it to be finished.
- Moving lights had no silhouettes of characters, paths had one size, and `mega` kept only colour. Proposals [0084](0084-live-lights.md), [0090](0090-pathing.md) and [0087](0087-mega-texture-and-seams.md) now say how each was finished.
- [The research note](../research/maps-materials-lighting.md) reserves per-pose bakes for items built into a model, as a long tool process. Meshes have no poses, but alias models do, and a capsule is the same shape whatever a body is doing, so a corpse shaded like something standing. Worse, capsules came from solid boxes, so a corpse shaded nothing.
- A pose is a few hundred points. Finding a pose's spheres takes microseconds, so the long process and the file it would write aren't needed.
- Proposal 0082 put an edge search in place of SMAA, and said a side by side showed no need for more, but that comparison never had SMAA in it. The human asked for SMAA as a setting.

## Design

- `pose_occlusion` works out each pose's spheres when a model loads: the whole of it, then three parts along its longest side. They go where capsules went, placed by the model's own matrix, with the whole far off and parts up close, as moving brush models do. Models narrower than 10 units, such as flames, shade nothing.
- Capsules come from what's drawn, not from solid boxes, so anything drawn shades, corpses included. The player keeps a capsule, as they have no model in their own view.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a pose's shade is worked out from where its points are, so a raised arm counts as much as a torso.
- **Acceptance criteria:** a pose lying down has a lower shade than one standing, parts follow a pose's longest side, and a pose that doesn't exist has none.
- **Future extension:** none planned.

## Review

See the increment 12 review.
