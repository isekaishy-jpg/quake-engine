# Proposal 0152: sky light

Status: accepted for item 178, 2026-11-08.

## Why

The human, looking at a grate's shadow in a yard under a live sun: the shadows aren't taking the light round them, and are too dark for such a well lit scene. What they were after is sky light: outdoors, what's in the sun's shadow is lit by the whole sky above it, and by what the sun lights round it.

## What there was

- **The sun**, from one way, baked or live.
- **Bounce**, light off what the sun and the lamps light, in a layer of its own, and the sun's apart so that a day can take it down.
- **`ambient`** in a level, and a day's `ambient`, each a flat amount added everywhere: in a shut room as much as out under the sky.

Flat ground in shadow gets nothing bounced from flat ground round it, which faces the same way, so out in the open a shadow had only the flat ambient: nearly black beside sunlit ground.

## What's done elsewhere

Sky light is how much of the sky dome a point sees, by the angle each part of it comes in at, times the sky's colour from there: a hemisphere of rays, each weighted by how squarely it lights the surface, or taken that many more times towards straight on, so each counts the same. Baked, as engines that bake light do (Quake 3's tools and their followers have "sky light" and "ambient occlusion" from the sky); live, as "sky visibility" or a bent normal, which is the same thing kept with less in it.

## What's done here

- **The bake** sends rays from each sample over the half of a sphere its face looks out on, as many towards each part as light from there lights it, 32 for a build, 12 for a quick bake and 64 for a final one. Each that gets to the top of the world, past what's solid, through glass and the holes of what's cut out as the sun's light does, brings the sky's colour from where it points: the horizon's low down and the zenith's overhead. What's below the horizon is the ground's light, which bouncing brings. It's worked out at some of a face's samples, as bounce is, and blended between them.
- **Its colour** is the sky's, brought to as bright as white, so that how bright it is says how much light: a blue sky lights blue, more so overhead.
- **How bright** is `sky_light` in a level, on a sun's scale. A level that doesn't say gets a quarter of its sun's `sun_intensity`, and with no sun none, so a level lit only by lamps is as it was.
- **It goes into the bounce layers**, so it takes no layer of a face's four: into what's bounced, which never changes, for a sun that doesn't move; and into what the sun bounces when a day moves it, which the game already takes down as the sun goes, so the sky's light dims at dusk and is gone at night with it. The game draws it as it draws them, with nothing new, and corners darken it as they darken the rest of light from all round.
- **Probes** take it too, where they're out in the open, so a model in shadow outdoors is lit by it. A probe doesn't change with a day, as what the sun bounces in it doesn't either.
- **Live shadows** take away only the sun's own light, so a shadow keeps the sky's.

## What came of it

- **Tried in the game:** the yard of grates under a live sun at noon. Without it the grate's shadow is a dark brown-grey, about 92 on the screen, where with it it's a blue-grey, about 125, and the sunlit ground is brighter too, as the sky lights it as well as the sun. That yard has an `ambient` of 40 standing in for the sky; a level with sky light can have less.
- **Tests:** open ground under a blue sky is lit, bluer than a wall, which gets about half; none with `sky_light 0`; a quarter of a sun's without a word, and none without a sun; with a day it's in what the sun bounces and not what never changes; a shut room and a probe in it get none, and a probe outside does; red glass in a roof makes it red. Each bug put back fails one.
- **What it doesn't do:** the sky's light isn't bounced again, as it's mostly what's in shadow that it lights, and that's dim. Its colour is the level's sky as baked, and a day's changing sky colour changes it only by how the sun's colour changes, as it goes with what the sun bounces. A level baked before this keeps its light until it's baked again.

## Sources

- [Proposal 0148: sky and light](0148-sky-and-light.md)
- [Proposal 0072: the light compiler](0072-the-light-compiler.md)
