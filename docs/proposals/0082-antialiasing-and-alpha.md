# Proposal 0082: anti-aliasing and alpha

Status: accepted, 2026-11-05, increment 11.

## Research

- [The research note](../research/maps-materials-lighting.md) asks for anti-aliasing that's cheap without being blurry, with MSAA and SMAA as settings, and for alpha with sorting and coverage.
- MSAA is sharp and stable on geometry, and does nothing for what's inside a surface. It costs memory and bandwidth, not much shading.
- SMAA 1x is morphological: find edges, search along them for their ends, and blend by the pattern. Most of its code is lookup tables that sharpen the corner cases. A search along each edge gets the part that matters here, the long shallow stairs of brush geometry, with no tables and no extra passes, and leaves texture detail alone, which is what makes cheap filters look blurry. So `r_aa 2` is that search, in place of SMAA proper. It can become full SMAA if a side-by-side shows it's worth the tables.
- Alpha to coverage writes alpha, which we use for reflections, and needs a `vulkane` flag. A fragment's `sample_mask` does the same job, leaves alpha free, and needs nothing new from it.

## Design

- `vulkane` gains sample counts: `Image::new_2d_multisampled`, `RenderPass::new_multisampled` with a resolve attachment, `GraphicsPipelineBuilder::samples`, and `PhysicalDevice::max_samples`.
- `r_msaa` sets samples a pixel, as far as the device goes. Changing it rebuilds the scene pass, the scene pipelines, the final pipeline and the target.
- Colour resolves in the pass. Depth isn't resolved, so the final pass reads it as multisampled, in a variant of its shader.
- `r_aa` is 0 for none, 1 for the quick blend, 2 for the edge search.
- Masked materials get a pipeline of their own, which covers samples by how far alpha is past the cutoff here, using its screen rate of change. With one sample this is the same as a cutoff.
- Blended faces each stand alone in a batch with a centre, and draw far to near, along with those on moving models.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** sorting by centre can't untangle faces that cross. The final pass reads one sample of depth, so an outline or fog at an edge belongs to one side of it.
- **Acceptance criteria:** samples put shades between a slanted edge's two sides and leave flat surfaces alone, the edge search changes only edges, a mask with samples has soft edges, and blended faces sort, with a side-by-side capture of the settings.
- **Future extension:** full SMAA tables, if the comparison asks for them.

## Review

See the increment 11 review.
