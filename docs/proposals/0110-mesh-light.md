# Proposal 0110: mesh light

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

[The note](../research/mesh-support.md) has the human's choice: generate light map UVs, so artists don't supply a second set. Small props use probes, by the author's choice. Each triangle being its own baked face doesn't suit a model of thousands, and glTF has only triangles, so flat polygons have to come back together.

- With a light map to each triangle, a generated well of 288 triangles showed every triangle as a stripe, since each was flat, and took three quarters of the map's samples.
- Tools that unwrap a mesh find charts: groups of triangles that can lie flat without folding, then pack them. A chart's edge is a seam, but seams hardly show if both sides are lit by the same surface.
- Light on a curve has to use the curve's normal, or the chart looks flat however good its UVs are.
- The engine's atlas, tiles, styles and bounce all work on rectangles of samples with texel axes. A chart is one of those. So UVs can be two axes per chart, with no second set of coordinates to store, and everything downstream keeps working.

## Design

- **Charts are worked out at a bake.** Starting from the biggest triangle, a chart grows across shared edges to triangles within 60 degrees of it. It stops at 2048 triangles, 96 samples across, or a triangle that would lie over another. Flat polygons come back together this way: one chart, one light map. A chart's plane is what a second set of UVs would have said.
- **`Face::light_axes`** places a face on its chart. Without it a face's light goes by its texture, as a brush's does, so nothing else changes.
- **`Map::mesh_groups`** says which faces came from which placement, as charts don't cross them.
- **Each sample finds its place on the surface,** on the nearest triangle, and takes that point and the smooth normal there. Samples past a chart's edge land on its edge, which gives the padding filtering needs.
- **A chart's faces share one `FaceLight`.** The atlas, tiles and the split into tiles hold those samples once.
- **Sharp light:** a chart has no crisp edge. One sample has to serve a curve, and the shadow edge detail is for flat surfaces.
- **Probes:** `light = probe` on a placement makes the whole mesh one sample, in its middle, lit from all round, for what's small or doesn't need more. A vertex says so, and the shader gives that light its shape from the direction it came from and the normal there.
- **Bumps:** the shader turned light by how a bump faced, against how the surface faced. On a chart with a curve in it, that went black where a surface faced away from the light next to it. It now goes by how far the bump turned from the surface, which is the same where it's lit, and nothing with no bump changes.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** no second UV set is stored, so an artist's own can't be used. The note chose generating them, and nothing needs one yet. Charts are planes, so a shape that can't lie flat in 60 degrees takes more of them, which costs samples but not correctness.
- **Acceptance criteria:** a flat surface of many triangles is one light map. A curve has no step where triangles meet. A round mesh looks round. A prop can use a probe. Tiles still work.
- **Future extension:** none planned.

## Review

| A level with a well of 288 triangles | Before | After |
|---|---|---|
| Light samples | 59,466 | 16,876 |
| Bake | 1.8 s | 0.5 s |

The well and statues were looked at. The well is smooth all round, where it was striped.
