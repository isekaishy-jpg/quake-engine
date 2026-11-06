# Shadow rendering

Status: research, not a design. Nothing here is approved to build. It goes through design and proposal later.

Started 2026-11-06, from the human playing Vesper Deep.

## How shadows are made now

- **Baked light:** each face has samples 32 units apart, set by `light_spacing`. One ray per light decides whether a sample is lit. Faces on one plane then share the samples they have in common (review 0016, finding 39).
- **The sun:** the first crisp light on a face also keeps how far each sample is from its shadow's edge, from four looks per sample each way. That keeps the edge sharp. Every other light is only its samples.
- **Moving lights:** a shadow map of six 512 pixel faces per light, in a 4096 atlas. It's looked up nine times, a surface is lifted off itself by its normal, and there's no noise.
- **Normals:** world and material shaders work a normal out of the flat triangle, per pixel. `Vertex` and the `.emesh` format have none. Only the fixture's alias models carry normals.

## Thin things and lamps

A lantern post in Vesper Deep has an odd shadow. The human asked whether it's the 32 unit issue. It mostly is.

- The shadow is real. `lantern_post` hangs its light 28 units from a post 10 units wide, so the post blocks a fan about 20 degrees wide, away from the lamp.
- The fan is narrower than one sample for its first 60 units or so past the post.
- A sample is lit or it isn't. Beside the post at (3396, 1416), 24 units off its centre line, samples go 0, 0, 87, 111 along the fan's edge, where the true edge tapers.
- So a slim wedge bakes as a blunt block about two rows deep, and neighbouring lamps light its far end unevenly.

Ways to deal with it, by what they'd cost:

| Way | What it does | Cost |
|---|---|---|
| Several rays per sample | A sample keeps how much of its area is lit, so the fan tapers and stays soft. Helps all lamp light | Longer bakes, not measured. Nothing more to store or draw |
| Sharp edges for more lights | The crispest result | A sample holds one edge now, so more data for each light |
| Finer `light_spacing` | 16 halves the problem | About four times the samples |
| A longer arm, or a lantern on top | Narrows or removes the fan | A choice of look. Anything else thin still blocks up |

Several rays per sample looks like the first thing to try.

## Curved surfaces

The human saw sawtooth and bands from moving lights, mostly on curved surfaces, and asked whether an actual mesh fixes that.

A throwaway level put one ring, of radius 176 and 72 high, under a moving light in three forms, compared at the same moments of the light's loop:

| Form | Bands | Sawtooth on a facet nearly edge on to the light |
|---|---|---|
| Brushes, 16 sides | One hard step per side | Scallops, clear once the darks are lifted |
| Mesh, 16 sides | The same | The same |
| Mesh, 128 sides | Thin strips | None at that moment |

- Being a mesh changes nothing. A placed mesh is flat triangles, drawn as brushes are.
- Finer facets help two ways. Each step in brightness gets small, and a facet edge on to the light gets almost no light, so what's wrong on it can't be seen.
- This was one light, one ring, at 1280 by 720. It wasn't checked at full size, or for what 128 sides costs in a real level. Each mesh triangle is its own baked face, with at least 3 by 3 samples in each layer, so a fine mesh may cost more light than its triangles suggest.

What a real answer probably needs:

- **Normals on meshes.** `.emesh` and `Vertex` would carry them, and shaders would use them for light. That's what makes a low mesh look round, and it would take away bands. It wasn't tested, as the engine can't do it yet.
- **It has its own fault.** [Smooth normals on few triangles put the shadow's edge in the wrong place](https://forums.unrealengine.com/t/virtual-shadow-maps-with-spiky-edges-for-low-poly-mesh-but-no-normal-maps/2004435), which offline renderers call the shadow terminator problem. [Cycles treats it with offsets](https://developer.blender.org/rB9c6a382), which it says still leave cases. [Its issue on this](https://developer.blender.org/T68920) is worth reading before design.
- **Quads or polygons in `.emesh`.** It holds triangles, and `add_triangle` makes each one a face with its own sample grid, worked out from its own corners. A flat quad is two faces with a join down its diagonal. Faces only share samples when their mappings match exactly, which those two may not, and that wasn't checked. Keeping a flat polygon as one face would halve the faces and remove the join. The GPU still draws triangles, and it wouldn't change what a curve looks like: a 16 sided ring is the same 16 flat sides either way.
- **Round brushes** would need smoothing groups, or an editor that turns curves into meshes. That's open.
- Softer shadow maps would blur the scallops, not remove the cause, so they're a separate question from this one.

## Moving lights

- Shadow maps show sawtooth and bands at a large window size, and more resolution only makes them smaller.
- The human expects some combination of soft shadows and shadow volumes, and that regular shadows have the same trouble to some extent.

## Notes for design

- [Variance shadow maps](https://uwspace.uwaterloo.ca/handle/10012/3640) can be blurred like a picture, which removes stair steps, and their exponential form leaks far less light where shadows overlap. It needs a floating point format.
- [Shadow volumes are exact](https://mail.fabiensanglard.net/doom3_documentation/CarmackOnShadowVolumes.txt), and cost up to three times a model's triangles per light, with fill rate that can get out of hand. [Penumbra wedges](https://www.cse.chalmers.se/~uffe/soft_gfxhw2003.pdf) soften them, and are limited by fill rate. Whether blurring a volume's mask on screen is common practice wasn't confirmed.
- [Distance field shadows](https://docs.unrealengine.com/udk/Three/DistanceFieldShadows.html) keep baked edges sharp at low resolution. `light_edges` already does this, for the first crisp light on a face only.
- Volumes need closed geometry, so what meshes are allowed to be matters to them.

## Still open

- Whether lamps should stay soft, or some should be sharp.
- How much longer several rays per sample makes a bake.
- Whether curves become meshes with normals, and what happens to curved brushes.
- Which way moving lights go, and whether baked and moving shadows should match in softness.
- Which light gets a sharp edge, and what happens past three layers. Both go by the order of lights now, and neither shows in a level yet.
- Sample sharing covers faces on one plane, so a shadow over a fold or a curve's facets isn't matched.
