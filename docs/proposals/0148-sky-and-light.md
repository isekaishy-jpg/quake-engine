# Proposal 0148: sky and light

Status: proposed, 2026-11-07, for items 62 to 69 of the [work list](../research/work-list.md). The research is in [sky and light](../research/sky-and-light.md).

## Why

Beams go through floors. The sky has one layer of cloud and nothing comes down from it. The sun is set once and baked in with everything else, so a level can't have a day.

## What changes

### Beams stop at what's in the way (62)

Each stencil gets a grid of how far light gets past each of its cells, by the same 16 by 16 cells as its occlusion. The client traces a few cells a frame, so doors count. A step along a view ray reads it, blended between cells, and goes dark past that distance.

### Cloud planes (63)

A sky has up to four `clouds` blocks, and the first, the lowest, is in front. New fields:

| Field | Means |
|---|---|
| `height` | How high it is, which sets how fast it goes by as the view moves |
| `curve` | 0 for a flat plane, up to 1 for a dome |
| `lit` | How much the sun's colour falls on it, and how dark its middle is |

One block with no new fields looks much as it did.

### Rays from the sky (64)

`sky_rays` in a sky says what shapes them: `clouds` for the generated clouds' own gaps, or a picture. `sky_rays_reach = level` lets them in, past the sky. It's a stencil that follows the view, on one more slot, and it uses the grid from 62.

### How a sun works (65, 68)

`sun = baked`, `live` or `arc` in the world.

- **`baked`** goes on a layer of its own, along with what it bounces. Its colour can change with no new bake.
- **`live`** isn't baked at all, apart from what it bounces. Three cascades of shadows follow the view, the nearest each frame and the others in turn.
- **`arc`** gives each sample of the sun's layer 32 bits, one for each angle through `sun_arc`, and the sun moves along it. It needs a final bake. Until there is one it's drawn as `baked`.

The moon works the same way as the sun, from the other side of the day, with a `moon_color`, `moon_size` and `stars`. Whichever is higher casts the shadows.

### A day (66)

`day_length` in the world says how many seconds a day takes, and `day_start` when a level starts. 0 stops the clock. The server keeps the time and sends it, and scripts can read and set it with `engine.time_of_day`. The sun goes round by `sun_path`, a tilt and how far off overhead it gets.

### Sets of light (67)

```text
light_set autumn {
    key 6 { zenith = [..] horizon = [..] sun_color = [..] fog_color = [..] }
    key 12 { .. }
}
```

Keys go round through a day, with what's between blended. They can say sky, cloud, fog, water, ambient, sun and specular colours, and any left out are blended from the keys that have them. The world's `light_set` names one for a map. A zone's `light_set` names its own, and `light_blend` says how far in it takes to get all of it. Sub zones go on top of what they're in.

### Final bake (69)

`engine-compile --final` runs what's too slow for each build, today the heat map and more samples. Its results go in a side file, and an everyday build leaves that file alone, so a level keeps its last final bake. `--final-area low high` only redoes what's in a box. The editor gets a button for it in stage 15, and a tool op now.

## What it doesn't do

- Clouds aren't volumes.
- Rays from the sky only follow a live sun's shadows through their own grid.
- What the sun bounces doesn't move with it.

Each of those comes from the research, and none needs more for what the items ask.

## Order

62, 63, 64, 69, 164, 65, 163, 66, 67, 68, then 165 to 168, each committed by itself. Items 163 to 168 were added by the human after this was written, and a tone curve comes before the moving sun so that a day is tuned once, on the curve it'll have. They'll have their own research.

## What came of it

### Beams and floors (62)

- **Tried in the game:** in Vesper Deep, the crypt under the Minster had coloured beams through its ceiling, and now it's dark. Up above they stop at the ground. In a two storey room, a beam stops at the upper floor and goes on past its edge.
- **What it costs:** too little to tell from noise. The pass with rays took 0.18 to 0.25 ms before and about 0.25 ms after, at 1280 by 720, with a beam filling much of the view. There are 32 traces a frame.
- **Light starts inside glass.** A stained window's glass is solid and its stencil is in the middle of it, so a trace starting there never got anywhere. It now looks for the way out, for up to 48 units.
- **Characters don't stop a beam,** though they do close a box. A beam's cells are too coarse for one to show.

### Cloud planes (63)

- **Tried in the game:** a high, slow, broad layer behind a low, small, lit one, in a yard with the sun low. They go by at different rates and come down to the horizon.
- **What it costs:** one layer costs what it did. With the sky filling the view, a frame's scene took about 0.5 to 1 ms with one layer made up, and 1.2 to 1.4 ms with four. The numbers move about from run to run, so that's rough.
- **A level's `speed` was being ignored.** It's two numbers, but it was read as three, so anything that gave two got the default, with no word about it. `outdoors` did. It takes two or three now.
- **A shell does both shapes.** Where a ray meets a shell round a world gives a plane for a big world and a dome for none, with one number between them. It's arranged so a flat plane doesn't lose its precision.
- **Pictures go in material slots,** so a sky uses the layout materials do, and has room for four of them.
- **A fifth layer is an error,** and doesn't vanish.

Filled in as each part lands.
