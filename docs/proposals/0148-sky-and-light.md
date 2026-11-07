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

`rays` in a sky says what shapes them: `clouds` for the clouds' own gaps, or a picture. `rays_reach = level` lets them in, past the sky. They have a march of their own, and a grid like the one from 62, of how low the sun gets.

### How a sun works (65, 68)

`sun = fixed`, `baked`, `live` or `arc` in the world.

- **`fixed`** is what a sun always was, baked in with everything else, and what a level has unless it says.
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

`engine-compile --bake final` runs what's too slow for each build, today finer samples and more of what bounces, and the heat map once there is one. Its results go in a file of their own, and an everyday build leaves that file alone, so a level keeps its last final bake. `--area` with two corners only takes that care over what's in a box. `--bake quick` and `--bake none` are for seeing a level soon. The editor gets a button for it in stage 15, and the `compile` tool takes the same now.

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

### Rays from the sky (64)

- **It isn't a stencil after all.** One high up would have needed a picture of the clouds made every frame. Reading the clouds where a ray goes needs none, and it's the same noise the sky uses, so they match. It needs a cheaper version of it, with less detail, as rays are soft.
- **Tried in the game, and the first try was wrong.** Marching every pixel of sky turned all of it a lilac haze. Away from the sun, what's along a view is spread over a lot of sky and averages out. Near it, gaps stay lined up along the view, and that's what a shaft is. So rays gather round the sun, and pixels looking away cost nothing.
- **The flare came from the human.** A version that was too strong blew out round the sun, and the human liked how that looked, looking right at it. So it stayed as `rays_flare`, tighter than the rays.
- **A grid can't go down from the sky.** Outside a level is solid, so looking from up there finds nothing. Each cell looks both ways from the height of the view: up to see if the sun gets there, and down to see where it lands. Under a roof it's the roof.
- **What it costs:** about 1 ms at 1280 by 720, looking towards the sun with the sky filling the view, and nothing looking away. It's taken half the steps a window's rays take to get there. That's a lot for what it is, and it's for the performance sweep to look at again.
- **Light bleeds a little over edges.** Rays are worked out in big pixels, so bright sky shows a short way over what's in front of it. It looks like glare, so it stays.

### Final bake (69)

- **Tried in the game, on one small level:** a quick bake took 0.5 s and is blocky and dark, as nothing bounces. A build took 0.7 s, and a final bake 4.4 s, with 3.6 times the samples and sharper shadows. A build after it left the final bake alone, and the game still used it. `--bake none` took 0.1 s, and the level plays flat and unlit.
- **A build is what it was.** `usual` is the same bake, to the byte, and a test holds it to that.
- **A part goes by whole faces.** Anything touching the box counts, so a floor that runs under a whole level brings all of it. That level's one big floor made a part cost 3.7 s, and a box clear of it took 1.4 s. Levels are made of smaller faces, and land has a grid of its own, which only a whole level changes.
- **What's outside the part doesn't look at more of what bounces.** At first it did, which cost nearly what all of it did.
- **A stale final bake is passed over,** and the compiler says so. A level is never shown with light from an older version of itself.
- **It ships.** A pack takes what's in the folder, so `.efinal` goes with a level.
- **What isn't there yet:** the heat map, which is item 68, and the editor's button, in stage 15.

### A sun that moves, and a moon (65)

- **Tried in the game:** the same yard with a fixed, a baked and a live sun looks the same, the live one a little sharper, so it's on the same scale as a bake. Moved round with `sun_toward`, its shadows go the other way and grow long as it gets low. Below the horizon it's the moon, from the far side, blue and dimmer, with its own shadows.
- **What it costs:** about 0.1 ms for its shadows, in a small yard. The nearest of three maps is drawn each frame, and the two far ones take turns, as a sun moves slowly.
- **`fixed` is what a level gets unless it says,** so nothing baked changes. The other kinds are the author's.
- **What a sun bounces is on a layer of its own,** apart from what the rest bounces, or a night would still be lit by the day's. That's two layers on a face for a sun, so other lights get fewer, and the sun is always given its two first.
- **A sun of its own can't be crisp.** Crisp layers go to whichever light needs them on a face, so nothing could find the sun's to change it.
- **Models get a live sun** if they can see the sky towards it, looked for once for each, before anything's drawn. Probes keep how much of their light is a baked sun's, for a day to work with.
- **What faces away from the sun is skipped before its shadow is looked at.** It would come out dark either way, as it's in its own shadow, but that's nine samples of a map saved, and no test can tell the two apart.
- **A shadow from straight up is under what casts it,** which is right, and it's what made a test look wrong at first.
- **What isn't done yet is day's:** the sky's colour, and turning down what a sun bounced as it sets, are item 66. `sun_toward` is for trying it, and a day drives it.

Filled in as each part lands.
