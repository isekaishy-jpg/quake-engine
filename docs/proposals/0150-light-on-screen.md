# Proposal 0150: light on screen

Status: accepted for items 164, 163 and 165, 2026-11-07. Items 163 and 165 to 168 of the [work list](../research/work-list.md) are added to this as each is reached.

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
- [Tweaking automatic exposure in Unreal, Versluis](https://www.versluis.com/2020/03/how-to-tweak-automatic-exposure-control-in-unreal-engine/)
- [Ambient occlusion and baked light, Godot forum](https://forum.godotengine.org/t/global-ambient-light-levels/138068)
