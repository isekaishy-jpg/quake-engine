# Proposal 0093: what increment 11 left

Status: accepted, 2026-11-05, increment 12.

## Research

- The report on increment 11 named what wasn't done, and the human asked for it to be finished.
- Moving lights had no silhouettes of characters, paths had one size, and `mega` kept only colour. Proposals [0084](0084-live-lights.md), [0090](0090-pathing.md) and [0087](0087-mega-texture-and-seams.md) now say how each was finished.
- [The research note](../research/maps-materials-lighting.md) reserves per-pose bakes for items built into a model, as a long tool process. Meshes have no poses, but alias models do, and a capsule is the same shape whatever a body is doing, so a corpse shaded like something standing. Worse, capsules came from solid boxes, so a corpse shaded nothing.
- A pose is a few hundred points. Finding a pose's spheres takes microseconds, so the long process and the file it would write aren't needed.
- Proposal 0082 put an edge search in place of SMAA, and said a side by side showed no need for more, but that comparison never had SMAA in it. The human asked for SMAA as a setting.
- SMAA's tables only save working out how much of each pixel a line covers. That sum is a few lines per edge pixel, so it can be done in the shader, and nothing has to be generated or shipped. Its low and medium presets leave out diagonal and corner passes.
- Built and put side by side on `showcase`, at three times size on a thin line and a shallow edge, the edge search turned a shallow edge into a sawtooth, worse than none. SMAA was smooth, and close to four samples of MSAA. The quick blend softens lines and leaves the stairs. So the search goes, and [`r_aa 2` is SMAA](0082-antialiasing-and-alpha.md).

## Design

- `pose_occlusion` works out each pose's spheres when a model loads: the whole of it, then three parts along its longest side. They go where capsules went, placed by the model's own matrix, with the whole far off and parts up close, as moving brush models do. Models narrower than 10 units, such as flames, shade nothing.
- Capsules come from what's drawn, not from solid boxes, so anything drawn shades, corpses included. The player keeps a capsule, as they have no model in their own view.
- SMAA is three steps. A pass finds edges by luma, leaving those a stronger neighbour hides. A second follows each edge up to 32 pixels each way, reads which way its ends turn, and works out how much each side of the line takes from the other. The final pass reads part way into a neighbour by that much. Both images are made with the scene target, and the passes only run when `r_aa` is 2.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** a pose's shade is worked out from where its points are, so a raised arm counts as much as a torso. SMAA has no diagonal pass, so edges near 45 degrees gain less, and it costs two small images and two passes. MSAA stays the cleanest on geometry.
- **Acceptance criteria:** a pose lying down has a lower shade than one standing, parts follow a pose's longest side, and a pose that doesn't exist has none. SMAA leaves flat pixels alone, puts shades between an edge's two sides, and leaves nothing behind when it's turned off.
- **Future extension:** none planned.

## Review

See the increment 12 review.
