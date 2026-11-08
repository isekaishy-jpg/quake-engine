# Proposal 0150: light on screen

Status: accepted for items 164, 163, 165, 166, 167 and 168, 2026-11-07. Items 163 and 165 to 168 of the [work list](../research/work-list.md) are added to this as each is reached.

## Why

Scenes are drawn in 16 bit floats, so there's room for a sun and a dark cellar in one view. Then what's over 1 had its colour scaled down to fit. It kept its hue, but anything brighter than the top looked the same as the top, and a sun went to a hard flat white. A day will go through far more brightness than a level does today, so the curve has to come first: then a day is tuned once, on the curve it'll have.

## How others do it

| Curve | What it does | What's wrong with it for us |
|---|---|---|
| Reinhard | Simple, and keeps hue | It dims everything, so every level would change, and it's used for quick previews |
| ACES | Filmic, with strong contrast and a warm roll-off | It changes every colour, and very bright blues drift towards turquoise. Bevy says its hue shifts are deliberate and dramatic |
| AgX | Very neutral, with little hue shift | It takes a lot of saturation away. Model Viewer's users complain of that for both this and ACES |
| Khronos PBR Neutral | One to one up to 0.8, then rolls off and goes towards white | It has a small offset that takes 0.04 off nearly everything |

The sources are practitioners' notes and not side by side tests, so we looked for ourselves.

## What changes

A level's world says `tone`:

| `tone` | Means |
|---|---|
| `neutral` | What's below 0.8 stays as it is, and what's above rolls off and goes towards white. It's the usual |
| `film` | Narkowicz's fit of the ACES curve, for more contrast and a look of its own. It changes all of a level, so it's for an author to choose |
| `hold` | What it was: the top is the top |

Neutral is Khronos's, without the offset. Levels are built by eye on what they show today, and it's their highlights that need help, so anything else that a level has stays exactly as it is.

What's carried over from the fixture has no say, and holds. It has its own look, and that's fine for it.

## Corners (163)

### How others do it

| Kind | What it does | What it costs |
|---|---|---|
| Hemisphere sampling | Looks at points round each pixel's normal, and counts those that are behind what's seen. The first kind, from Crytek, and what Unity's usual one is | The least. Its looks are noisy, so it's blurred |
| Horizon based (HBAO) | Walks out from each pixel in a few directions, and finds how high what's round it rises | More. Smoother, and darker in deep corners |
| Ground truth (GTAO) | Works out the horizon in each direction exactly, to match what a ray tracer finds | The most. A guide puts it at 1 to 2 ms, which is most of a frame here |

Those who've compared them say the first is the most even from scene to scene and the cheapest, and that the others aren't always worth their cost. The numbers are practitioners' and not measured side by side.

What matters more here is what it darkens. Darkening the whole picture is cheap, as it's done at the end, but it puts dark halos in sunlight, where the sun's own shadows already say what's shaded, and it darkens fog. What's usual where it's done properly is to darken only light that comes from all round. Unity has a strength for how much it touches direct light, and warns that doing it at the end darkens what baked light already shaded twice.

Baked light here is a sample every 16 units, so it has soft corners at that size, and nothing smaller. What's darkened in a corner is smaller than that, so the two go together. A live sun has shadows of its own, and the light a day adds everywhere has none at all, so that's what needs it most.

### What changes

- **Hemisphere sampling,** at half the size each way, 8 or 16 looks a pixel, and smoothed by what surfaces read round them.
- **Depth is drawn first, and how open each pixel is worked out from it,** and then surfaces are shaded with it. That puts what's open into the shading, so only light from all round is darkened: baked light and a day's. A sun's own layer, a live sun and moving lights aren't. Neither is fog.
- **Both choose.** A level says how much and how far round, as `ssao` and `ssao_radius`, and a player says how much they'll spend, with `r_ssao`.

## Exposure that goes by itself (165)

### How others do it

Unreal and Unity measure how bright a scene is from the average of its brightness as powers of two, from a histogram or straight, and leave out the darkest and brightest few percent so that a lamp or a black corner doesn't sway it. Unreal can weight the middle of the view more, as that's where a player looks, and says that steadies it. The average is taken as middle grey, held between a least and a most an author sets, and exposure goes there at so many stops a second, with one speed into light and another into the dark. Unreal's volumes set all of it in places, and setting the least and most the same turns it off.

### What changes

- **The scene's measured** in 16 cells each way, each the average of its brightness as a power of two at 16 points, and read back once its frame's done. The cells near the middle count most, and the darkest and brightest tenth are left out.
- **Exposure goes towards middle grey,** as far as the level's `exposure_range` lets it, down and up in stops from its `exposure`, at `exposure_speed`: 3 stops a second into light and 1 into the dark unless it says. A level starts where it should be, and so does a long gap.
- **It's the author's.** Levels are built by eye on what they show, so one adjusts only if it says how far. A zone can say how far in it, in place of the level's, and a zone's light still multiplies after, so a zone that darkens stays darker. A player can turn it off with `r_exposure_auto 0`.
- **`exposure` at the console** says how bright what's seen is and how far it's gone, to tune a range by.

## Bloom (166)

### How others do it

Jimenez's way for Call of Duty: Advanced Warfare is what Unity and Blender's EEVEE use. A chain of images, each half the last, is made going down with 13 looks a pixel, and the first step weights each look down by how bright it is, so that one bright pixel doesn't flicker as it moves. Going back up, each image adds what's below it, spread by a tent of nine. Newer engines blend that into the picture by a small share with no threshold, as that keeps energy, and Unity says a threshold breaks it. Bevy has both, a blend and an add with a threshold.

### What changes

- **The chain goes down to six images,** from half the scene's size, and back up. The final pass adds it before exposure.
- **Only what's brighter than a screen shows glows.** A blend with no threshold softens every pixel a little, which would change every level as it was built. With a soft threshold at 1, once exposed, only what a screen can't show glows: the sun, and bright lamps. A test holds a picture with nothing brighter to the same bytes.
- **A level says how much** with `bloom`, 1 unless it says, and from how bright with `bloom_threshold`. What's carried over from the fixture has none. A player can turn it off with `r_bloom 0`.

## HDR all the way through (167)

### What cut brightness off

Scenes are drawn in 16 bit floats, and the sky, live lights, a live sun, fog and what's emissive go into them as bright as they are. Four things stopped light before the final pass:

| What | Where it stopped |
|---|---|
| Light on a surface | All of it, baked and live together, brought down to 2 with its hue kept |
| Models | Quake's limits: ambient at 128 and all of it at 192, a half again on what their skin shows |
| Baked light | A byte a colour, so 2 once doubled. A bake brought a colour down whole if it was brighter |
| Probes | The same, which is what models and the fog grid are lit by |

The first two are the fixture's look, and keep it. They needn't be on a level made for this engine.

### How others store baked light

| Way | What it holds | What's wrong with it for us |
|---|---|---|
| A byte a colour, over 0 to 2 | What a screen shows, twice over | What's brighter stops. It's what Unity does on phones, and what we had |
| RGBM | A colour and a multiplier, in four bytes | The fourth byte is where a sample's shadow edge is, and where an arc sun's last eight bits are |
| BC6H | 16 bit floats, compressed to a byte a texel | Compressing would garble an arc sun's bits, and tiles are written as they stream |
| RGB9E5 | Three colours and an exponent, in four bytes | The same four bytes, so no room for edges or bits |
| 16 bit floats | Anything | Twice the size, and the same want of room |

Every way of holding more needs the fourth byte, or more bytes, and the fourth byte's taken.

### What changes

- **Light goes past 2 on a surface,** and models past Quake's limits, on a level made for this engine. Light goes on to the tone curve and bloom as it is. What's carried over from the fixture keeps both, as `Look::light_most`.
- **A bake brings light down by a power of two** when some of it is brighter than a byte holds, all of a level's alike, and its probes with it, and the game multiplies it back. A level whose light fits is as it always was. An arc sun's bits aren't light, and are left as they are. A saved bake says how far.

## HDR to a screen that has it (168)

### What it takes

Vulkan says what a screen takes as pairs of a format and a colour space, once `VK_EXT_swapchain_colorspace` is on. Two are HDR:

| Output | Sent as | What 1 is |
|---|---|---|
| scRGB | Linear light in 16 bit floats, Rec. 709's colours | 80 nits, and more is brighter. Windows composes in it |
| HDR10 | 10 bits a colour on the PQ curve, Rec. 2020's colours | Nits, from 0 to 10,000 along the curve |

On either, white isn't the top: a player says how bright white is, and the tone curve rolls off at the brightest the screen shows instead of at white. `VK_EXT_hdr_metadata` tells the screen that much, where it's there. What's drawn over the view, the HUD and menus, is made for a standard screen, so it's taken as sRGB and put at white. A screen that takes neither, or a system that doesn't say, is sent what it always was.

### What changes

- **`r_hdr 1`** sends HDR to a screen that takes it, scRGB if it does, HDR10 if not, and standard otherwise. `r_hdr_white` is white in nits, 200 unless a player says, and `r_hdr_peak` the brightest, 1000.
- **`screen` at the console** says what's being sent and what the screen takes.
- **Pictures stay standard.** `screenshot` and what tests read are what they were, and tests can ask for HDR as it would be sent, in floats.

## What came of it

### A tone curve (164)

- **Tried in the game:** with a sun's flare, `hold` has a hard white middle, `neutral` the same colours with a soft edge to the glare, and `film` is richer, with the sun's disc showing in it.
- **What it costs:** too little to see. It's a few sums on each pixel, in a pass that was already there.
- **Below 0.8, a level is the same to the byte.** A test holds it to that.
- **It's after exposure, and before the player's brightness,** so a zone that lets in less light brings a highlight off the curve.
- **Light on surfaces still stops at 2** before it gets here, which is item 167.

### Corners (163)

- **Tried in the game:** a brick wall standing on a floor, with the sun live. With it, the crease darkens softly along the shaded side, the sunlit floor past the wall's end is untouched, and there's no halo against the sky.
- **At first it darkened the whole picture** at the end, in the pass that draws light in the air. It was cheap, but it darkened sunlit corners and fog with the rest, and a wall's edge had a halo. So depth comes first now.
- **What it costs:** about 0.2 ms on this card, with 8 looks or 16. Breaking the scene's pass in two costs nothing that shows.
- **The normal is from the nearer side** each way. From one side only, a pixel at an edge took the slope across to what's behind it.
- **What wasn't seen isn't darkened.** What's kept is how open a pixel is and how far it is, and a surface reads only what's as far as it is, so what's see-through doesn't take the shade of what's behind it, and none bleeds across an edge.

### Exposure that goes by itself (165)

- **Tried in the game:** a yard at noon measures 2^-2.46, almost exactly middle grey, so it stays as it is. A closed shed measures 2^-7.85 and goes up the whole 2 stops, and reads clearly. Stepping out, exposure comes down in about two thirds of a second, and stepping back in it takes two. At night it goes up as far as it's let, and the night's lighter but still a night.
- **At first every level adjusted.** Vesper Deep's start looks out of a dark doorway, which measures 2^-7.4, and the whole level came out two stops brighter than it was built. So a level only adjusts if it says how far, as the tone curve leaves levels as they were built.
- **What it costs:** too little to show. It's 256 pixels and a copy of a kilobyte, read a frame or two late, and in the yard the scene took 2.0 to 2.1 ms with it on and with it off.
- **What's measured is the scene,** before fog and what's drawn in the final pass, and before a zone's light, so a zone isn't undone.
- **A setting a player saves** carries over to the next run, which made a level look as if it ignored its range while trying it.

### Bloom (166)

- **Tried in the game:** looking at the sun, it glows round its disc, and more as `bloom` goes up: 0.5 is faint, 1 clear, 2 strong. Vesper Deep looks the same with it and without, as nothing there is brighter than a screen shows.
- **At first the glow was a second sun,** across the picture from the first. The scene's held upside down and the final pass turns it over, so the chain has to turn the same way. A test now puts a spot off to one side and looks for its glow round it, not across from it.
- **What a single bright pixel adds is weighted down** on the first step, so it doesn't flicker as it moves. It works: a moon smaller than a pixel glows 16% less with it than without. No test sees that by itself yet, as light on a surface stops at 2 and a spot can't be made bright enough, and the sky's are either too dim to glow or bright enough to fill what's measured. Item 167 takes away the stop, and then the test of a spot ten times as bright has to see it, with the bug put back.
- **What it costs:** about 0.1 ms in a window, and nothing with it off. `gpu_times` shows it as glow.
- **Exposure comes first:** what's measured against the threshold is the scene as exposed, by the level and by exposure that goes by itself.
- **Screens show light on a curve,** so twice the bloom is less than twice the bytes, which a test had to allow for.

### HDR all the way through (167)

- **Tried in the game:** a lamp of 300 by a wall at night, then 600, then 1500. They go from lit, to bright with a glow, to the wall washed out and glowing all round, where before all three stopped at the same brightness near it. Vesper Deep looks the same, as its light's within a byte.
- **What it costs:** nothing to draw. A bake looks at all its light once more for the brightest.
- **Where light's brought down, the dark loses a bit** for each power of two, as a byte covers more. It's only on a level with light that bright, and only by as much as it needs.
- **A probe brighter still** than the brightest surface is brought down whole, as before. Probes are brought down by what surfaces need, as both are read together.
- **Bloom's weighting of bright looks can be seen now.** With light stopped at 2, no test could make a spot bright enough to see it. It's only for what's smaller than its looks: a spot a few pixels across glows as much again when it's ten times as bright, as it should. A lamp lighting a pixel or so at 30,000 glows 3632 with it and 6713 without, and a test holds it between the two, so it fails with the weighting taken out.
- **Light brought down in a dim place** shows what a byte loses: a floor at 3 of 255, brought down by four, is 0 or 1. A test compares rooms by their brightest, where it doesn't.
- **Three tests of the bake were reading bytes,** and now read light as the game does, multiplied back. One of them, of a shadow round a corner, had passed only because bright light stopped at a byte: it read whichever floor face was biggest, which next to the post wasn't the one holding the point, and stopped bytes hid that. It reads the face that holds the point now.
- **A model in bright light** has ambient and directional light going on as they were at Quake's 128, the directional half the ambient, so it's as bright as what's round it.

### HDR to a screen that has it (168)

- **Tried in the game:** this screen takes only standard, so with `r_hdr 1` it's sent standard, `screen` says so, and the picture is the same to the byte as with it off. **HDR itself needs a person with an HDR screen** to see, with HDR on in Windows: tests check what's sent, but not how a screen shows it.
- **What's sent is checked in numbers.** A sky of 0.5, which a sky takes as sRGB, is 0.214 of white in light: 0.535 as scRGB at 200 nits, and 0.426 up the PQ curve as HDR10. Three times white is three times as bright, and what's brighter stops at the peak. Pure red has green and blue in Rec. 2020. The HUD's grey is white's share of it.
- **The scene isn't drawn twice.** Only the final pass and the HUD know the screen, and their pipelines for HDR are made when it's wanted.
- **A part only a person can try:** a window sending HDR tells its final pass so each frame, and no test has a window. If HDR looks wrong on an HDR screen, that's where to look.

## Sources

- [PBR Neutral, Khronos](https://github.com/KhronosGroup/ToneMapping/blob/main/PBR_Neutral/README.md)
- [Tonemapping in Bevy](https://doc.qu1x.dev/bevy_trackball/bevy_core_pipeline/tonemapping/enum.Tonemapping.html)
- [Tone mapping in Model Viewer](https://modelviewer.dev/examples/tone-mapping)
- [Tone mapping overview, three.js forum](https://discourse.threejs.org/t/tone-mapping-overview/75204)
- [Tone mapping, OpenRV](https://lifeart.github.io/openrv-web/docs/color/tone-mapping.html)
- [Advanced Techniques and Optimization of HDR Color Pipelines, Lottes](https://www.gdcvault.com/play/1023512/Advanced-Graphics-Techniques-Tutorial-Day)
- [Ambient occlusion explained: SSAO, HBAO and GTAO](https://superrendersfarm.com/article/ambient-occlusion-explained-ssao-hbao-gtao-2026)
- [SSAO, HBAO+ and XeGTAO compared, Steam discussion](https://steamcommunity.com/app/2215430/discussions/0/7093810350790445377)
- [Do you apply SSAO to the whole image or to ambient only, GameDev.net](https://www.gamedev.net/forums/topic/653661-do-you-apply-ssao-to-rendered-image-or-to-ambient-light-term-only/)
- [SSAO, Unity URP](https://docs.unity3d.com/Manual/urp/ssao-renderer-feature-reference.html)
- [Automatic exposure, Unreal Engine 4.27](https://docs.unrealengine.com/4.27/RenderingAndGraphics/PostProcessEffects/AutomaticExposure)
- [Auto exposure in Unreal Engine](https://dev.epicgames.com/documentation/en-us/unreal-engine/auto-exposure-in-unreal-engine)
- [Auto exposure, Unity post processing](https://docs.unity3d.com/Packages/com.unity.postprocessing@3.4/manual/Auto-Exposure.html)
- [Next generation post processing in Call of Duty: Advanced Warfare, Jimenez](https://www.iryoku.com/next-generation-post-processing-in-call-of-duty-advanced-warfare)
- [Bloom, Unity HDRP](https://docs.unity3d.com/Packages/com.unity.render-pipelines.high-definition@17.0/manual/Post-Processing-Bloom.html)
- [Bloom, GameDev.net](https://gamedev.net/forums/topic/693388-bloom/)
- [Bloom in Bevy](https://docs.rs/bevy/0.15.2/src/bloom_3d/bloom_3d.rs.html)
- [Bloom, Diligent Engine](https://diligentgraphics.github.io/docs/df/de0/DiligentFX_PostProcess_Bloom_README.html)
- [Lightmaps: technical information, Unity](https://docs.unity3d.com/Manual/Lightmaps-TechnicalInformation.html)
- [VK_EXT_swapchain_colorspace, Khronos](https://registry.khronos.org/vulkan/specs/latest/man/html/VK_EXT_swapchain_colorspace.html)
- [VK_EXT_hdr_metadata, Khronos](https://registry.khronos.org/vulkan/specs/latest/man/html/VK_EXT_hdr_metadata.html)
- [SMPTE ST 2084, the PQ curve](https://en.wikipedia.org/wiki/Perceptual_quantizer)
- [Tweaking automatic exposure in Unreal, Versluis](https://www.versluis.com/2020/03/how-to-tweak-automatic-exposure-control-in-unreal-engine/)
- [Ambient occlusion and baked light, Godot forum](https://forum.godotengine.org/t/global-ambient-light-levels/138068)
