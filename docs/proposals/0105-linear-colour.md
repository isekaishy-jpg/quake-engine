# Proposal 0105: linear colour

Status: accepted and implemented, 2026-11-06, the sprint through the list.

## Research

The human asked for sRGB and the best colour quality we can get, with a murky look like Quake's still available.

- Everything was in gamma space. Pictures went up as plain bytes, light multiplied them there, the target was 8 bits, and `pow(color, gamma)` ran at the end of every surface shader. Adding light in gamma space makes overlaps too bright and shade too dark.
- Colour pictures want decoding to linear light, which the GPU does for free in an sRGB format. Normals, specular, heights, masks and flow are measures, not colours, and have to stay as they are.
- Light was on a scale tuned by eye to multiply a gamma-space picture. A picture `p` lit by `l` used to show as `p * l`. In linear light that's the same on screen if `l` is raised to about 2.2. So raising baked light to a power runs from true linear light at 1 to the old look at 2.2.
- At a curve of 1, levels looked washed out: their soft bounced light was tuned for the old curve. Compared on the same views in Vesper Deep: 1.0 lifts shade to a milky grey, 1.5 keeps colour but little mood, 1.8 keeps depth with colour in shade, and 2.2 crushes shade to black.
- 8 bits can't hold linear light: darks band. A 16 bit float target holds it, and more than the screen can show.

## Design

- **Scenes are linear.** They draw into `R16G16B16A16_SFLOAT`, as does light in the air. Colour pictures upload as `R8G8B8A8_SRGB`, and `slot_format` keeps measures as they are. Canvas pictures stay in screen space, as they're drawn after the scene.
- **One step to the screen.** `to_screen` ends the final pass: exposure, bringing what's too bright down as a whole so it keeps its hue, the player's `gamma`, sRGB, and a little noise under a byte's step so dark gradients don't band. Surface shaders lose their `pow`.
- **A map's look.** `Map::look` holds `light_curve` and `exposure`, from `worldspawn`. The curve raises baked light in `light()`, in models and in fog's probes. It defaults to 1.8. Original levels use 2.2, so the fixture looks as it did. That's the murk, and any level can ask for it.
- Colours set as numbers are as they're seen, so they're converted: fog, outlines, pools, particles, and the sky at the end of its shader.
- Edge smoothing goes by how things look, so it reads the square root of linear brightness.

## Proposal

- **Selected approach:** the above.
- **Tradeoffs:** the 16 bit target takes twice the memory and bandwidth of the old one. Taking `pow` out of every surface more than paid for it. Mips are still averaged in gamma space until textures are compiled (sprint item 5).
- **Acceptance criteria:** colour reads true, light adds in linear space, original levels look as they did, the murky look is one setting, and frame rate doesn't fall.
- **Future extension:** a filmic curve could replace the plain hue-keeping limit, if a level wants one.

## Review

| Where, at about 2560 by 1400 | Before | After |
|---|---|---|
| Square | 216 to 224 FPS | 247 |
| Market | 170 to 175 | 188 to 200 |
| Quay | 176 to 186 | 197 to 207 |
| Terrace | 174 to 186 | 196 to 211 |

Vesper Deep, the Ember Keep and the original start map were looked at.
