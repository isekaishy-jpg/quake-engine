# Sky and light

Research for items 62 to 69. It says what we have, how others do it, and what suits us. [Proposal 0148](../proposals/0148-sky-and-light.md) says what we'll do.

## What we have

- **The sun is baked.** `sky.sun_direction` and `sun_intensity` make one more bake light, on style 0 with everything else. Nothing can tell its light from a torch's after the bake.
- **Rays** march the view ray through a stencil's beam, in a quarter of the pixels. Each step looks towards the light and reads the picture. Nothing looks the other way, from the stencil to the point, so a beam carries on through a floor (item 62). The boxes of a stencil only ask whether light gets to the stencil.
- **Clouds** are one layer, on `direction.xy / (height + 0.12)`, from a picture or from six layers of noise.
- **Moving lights** have shadows in an atlas of 512 pixel tiles, drawn from the world's opaque faces, models and land. That's most of what a sun's shadows need, apart from the shape of the map a sun wants.
- **Faces hold four layers** of baked light, each with a style. A style is one number a frame.
- **Zones** already blend how much light reaches the eye, and they're the client's to know about.

## Rays and floors (62)

There are three ways to stop a beam.

| Way | What it takes | What's wrong with it |
|---|---|---|
| A shadow map for each stencil | A depth tile per stencil, read by each step | The right answer for a moving light, but a stencil is mostly still, and it adds a draw of the world per stencil |
| A grid of how far light gets, by stencil | One trace per cell, a few each frame, and one read per step | Coarse: 16 cells a side |
| Trace in the shader | Nothing to store | There's no scene for a shader to trace |

A floor is flat, so how far light gets changes evenly across a stencil, and a coarse grid with blending between its cells follows it closely. Beams are soft, so the steps it can't follow don't show. It goes round on the same few traces a frame as the boxes, so a door shutting shows in a moment. The grid is what we want.

## Clouds (63)

Older engines stack scrolling layers on a dome or on flat planes, and each layer's height sets its curve and how fast it seems to go. Torque's has up to three layers, with higher ones hidden by lower ones, and notes that they take no light from the scene, which makes them poor for day and night. So ours should take colour from the sun. A curve can be worked out exactly: where a ray meets a shell round a world of some radius. A huge radius is a flat plane and a small one a dome, so one number does both.

## Rays from the sky (64)

| Way | Cost | Good for | Bad for |
|---|---|---|---|
| Blur outward from the sun on screen | One or two passes | Sun in view | It goes when the sun's behind you, and looks like fog in the open, which is what people found with Crysis |
| March against a shadow map | Steps a pixel | Everything | Needs a live sun, and most levels won't have one |
| A stencil in the sky | One more stencil | Works with what's there | Only as good as its picture and grid |

Our rays are a march already, so a stencil high up that follows the view costs one more stencil. Its picture can be anything: a painted one for a cheap sky, or one worked out from the generated clouds, so that rays come through the actual gaps. It's the author's choice for each sky, as asked. Keeping it in the sky just means leaving out pixels with something in front. Letting it into the level uses the grid from item 62, so a roof stops it.

We won't do the blur. It fails exactly where a sky is most seen, with the sun behind you.

## A sun that moves (65)

Lightmaps can't follow a moving sun: Unity's manual says so of its baked light, and points to realtime light for a day and night cycle. The ways people do it:

| Way | How | Cost |
|---|---|---|
| Cascaded shadow maps | A few maps of the view at growing sizes, each redrawn | Reports are about 0.5 to 2 ms for three or four cascades at 1024 pixels. They're blogs on their own machines, so we measure |
| Cached cascades | Far ones are drawn rarely. CryEngine's are nearly free once drawn, and redraw as the view nears their edge | Far shadows lag, and they don't show what moves. Cyberpunk 2077 splits static from moving things to keep as much as it can |
| A baked sun with colours only | Direction stays, colour changes | Nothing at all, but shadows never move |
| Entry and exit times | See [item 68](#the-shadow-heat-map-68) | One read a pixel, and a long bake |

A sun moves slowly, so its far cascades can be drawn in turn, one a frame. No source measures that for a moving sun, so it's ours to measure.

All three of ours can sit side by side: a level picks how its sun works. Baked for a level that doesn't want a day, or wants colour only, and live or arc where shadows have to go round.

Where the sun's direct light is live, its bounce can't be, but it's soft, so a baked layer scaled by the sun's colour holds up. It won't be right when the sun's somewhere else. That's a fair trade, as the same layer is all a level has to go on, and light sets can raise or lower it.

## Day, night and sets of light (66, 67)

The reference in `solarityclient` is what the item says: a map has a global set, local volumes have a falloff, each channel has keys through the day, and the light's direction is worked out from the time. Almost everything is a colour, so all of it is a few lerps on the CPU each frame. We can do the same. The list of channels asked for is sky, cloud, fog, water, ambient, sun and specular.

Time has to agree for everyone in a game, so it has to come from the server, in the same way as light styles.

## The shadow heat map (68)

This has been done, in a small way. Simon Green's demo kept, for each point, when it goes into shadow and when it comes out as a sun moves, and a shader worked out how shaded it was from those. A Unity answer gives the same idea, with a picture of two values. Horizon maps do it with angles for terrain.

Two values only hold one shadow. A sample that's shaded twice as the sun moves, by two pillars, needs more. So we'd rather have bits: a 32 bit number for each sample, with one bit for each of 32 sun angles across the arc.

- **How many angles:** 32 is one number a sample, the same as a colour. 60 degrees of arc is then a bit each 1.9 degrees. A shadow moves about 3.3 units for every 100 units of height it falls from, between bits.
- **What it costs to bake:** 32 rays a sample where one was, and more for thin things. It's why it's for the final bake.
- **Thin things:** something narrower than about 1/30 of how far away it is falls between two angles. So each bit takes several rays through its slice of the arc, and is lit if most are.
- **Going soft:** two neighbouring bits, blended by where the sun is between them, and four samples of a map blended as usual, make the edge move smoothly.

## Final bakes (69)

Today each compile bakes in full. What's slow is what a level only needs when it's done. That wants a switch on the compiler, with quick the usual, and a way to keep what's outside a part of a level.

## Sources

- [Cached Shadows, CRYENGINE](https://www.cryengine.com/docs/static/engines/cryengine-3/categories/1114113/pages/21267738)
- [Shadows Optimizations in Cyberpunk 2077, Dybisz and Witanowski](https://history.siggraph.org/learning/shadows-optimizations-in-cyberpunk-2077-by-dybisz-and-witanowski)
- [Budgeting the pretty, a blog with cascade timings](https://app.cinevva.com/blog/2026-02-25-open-world-browser-part-05-budgeting-the-pretty.html)
- [Fixing shadow rendering performance, a blog with cascade timings](https://bugnet.io/blog/how-to-fix-shadow-rendering-performance)
- [Lighting in Unity](https://docs.unity.cn/2017.4/Documentation/Manual/LightingInUnity.html)
- [Day cycle and lightmapping, Unity forum](https://discussions.unity.com/t/day-cycle-and-lightmapping/17304)
- [Nvidia's Last Chance Gas, Khronos forum](https://community.khronos.org/t/nvidias-last-chance-gas/42071)
- [Fun with horizon maps](https://dasilvagf.github.io/posts/2020/08/fun-with-horizon-maps)
- [Sunbeams in Crysis, Khronos forum](https://community.khronos.org/t/sunbeams-in-crysis/54219)
- [Volumetric lighting](https://en.wikipedia.org/wiki/Volumetric_lighting)
- [Torque3D BasicClouds](https://torque3d.readthedocs.io/en/latest/script/class/BasicClouds.html)
- [Clouds and Stars, Parberry](https://ianparberry.com/pubs/clouds_and_stars.pdf)
