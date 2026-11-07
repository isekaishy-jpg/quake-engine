# Proposal 0150: light on screen

Status: accepted for item 164, 2026-11-07. Items 163 and 165 to 168 of the [work list](../research/work-list.md) are added to this as each is reached.

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

## What came of it

### A tone curve (164)

- **Tried in the game:** with a sun's flare, `hold` has a hard white middle, `neutral` the same colours with a soft edge to the glare, and `film` is richer, with the sun's disc showing in it.
- **What it costs:** too little to see. It's a few sums on each pixel, in a pass that was already there.
- **Below 0.8, a level is the same to the byte.** A test holds it to that.
- **It's after exposure, and before the player's brightness,** so a zone that lets in less light brings a highlight off the curve.
- **Light on surfaces still stops at 2** before it gets here, which is item 167.

## Sources

- [PBR Neutral, Khronos](https://github.com/KhronosGroup/ToneMapping/blob/main/PBR_Neutral/README.md)
- [Tonemapping in Bevy](https://doc.qu1x.dev/bevy_trackball/bevy_core_pipeline/tonemapping/enum.Tonemapping.html)
- [Tone mapping in Model Viewer](https://modelviewer.dev/examples/tone-mapping)
- [Tone mapping overview, three.js forum](https://discourse.threejs.org/t/tone-mapping-overview/75204)
- [Tone mapping, OpenRV](https://lifeart.github.io/openrv-web/docs/color/tone-mapping.html)
- [Advanced Techniques and Optimization of HDR Color Pipelines, Lottes](https://www.gdcvault.com/play/1023512/Advanced-Graphics-Techniques-Tutorial-Day)
