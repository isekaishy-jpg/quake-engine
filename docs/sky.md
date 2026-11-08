# Sky

A level's sky is a `sky` block. See [proposal 0148](proposals/0148-sky-and-light.md).

```text
sky {
    zenith = 0.12 0.28 0.6
    horizon = 0.8 0.78 0.72
    ground = 0.32 0.3 0.28
    sun_direction = 0.5 0.4 0.5
    sun_color = 1 0.8 0.55
    clouds { coverage = 0.45 scale = 0.8 height = 3000 lit = 0.8 }
    clouds { coverage = 0.6 scale = 0.25 height = 9000 curve = 0.2 lit = 0.5 }
}
```

| Field | Means |
|---|---|
| `zenith`, `horizon`, `ground` | The colours overhead, at the horizon and below it |
| `horizon_height` | How high the horizon's band reaches, from 0 to 1 |
| `sun_direction`, `sun_color`, `sun_size` | Towards the sun, its colour, and how big it looks |

## Clouds

A sky has up to four `clouds` blocks. The first is the lowest, so it's in front of the others. Each is made up in the shader, or comes from a picture, and more is an error.

| Field | Means |
|---|---|
| `image` | A picture to use, by how bright it is. Without one they're made up |
| `color` | Their colour |
| `coverage` | How much of the sky they cover, from 0 to 1 |
| `scale` | How often they repeat, so small numbers are big clouds |
| `speed` | How they drift, in repeats a second, as two numbers |
| `height` | How high they are in the world, so that they go by as the view moves. At 0 they stay with it, as if they're very far off |
| `curve` | 0 for a flat plane going on to the horizon, 1 for a dome. Unless told it's 0.03, which is nearly flat overhead and comes down at the horizon |
| `lit` | How much they take the sun's colour, go dark where they're thick, and shine where they're thin near the sun, from 0 to 1 |

Layers that are made up cost the most, as each is six layers of noise for each pixel of sky. Pictures are nearly free.

## Rays

Rays come down through the sky if `rays` says so. They gather round the sun, and looking away from it there are none. They're seen with the rays setting on, like a window's.

```text
sky {
    sun_direction = 0.5 0.1 0.6
    rays = clouds
    rays_reach = level
    clouds { coverage = 0.55 scale = 1.4 height = 2500 lit = 0.7 }
}
```

| Field | Means |
|---|---|
| `rays` | `clouds` to come through the gaps in the clouds the sky shows, or the name of a picture, bright where light gets through, for a cheap sky |
| `rays_reach` | `sky` to stay in the sky, which is what's usual, or `level` to come down over what's in the level. A roof stops them |
| `rays_strength` | How bright they are |
| `rays_flare` | How much the sun flares when it's looked at, and 0 for none. What's in front of it dims it |
| `rays_size` | How wide a repeat of a picture is, in world units |
| `rays_speed` | How a picture drifts, in repeats a second, as two numbers |

`rays = clouds` with no clouds is an error. They cost about 1 ms at 1280 by 720, looking towards the sun with the sky filling the view, and nothing looking away.

## The sun and the moon

The world says how its sun works, with `sun_intensity` for how bright and the sky's `sun_direction` for where.

| `sun` | Means |
|---|---|
| `fixed` | Baked in with everything else, as it always was. It's what a level gets unless it says |
| `baked` | Baked on layers of its own, with what it bounces, so that a day can change its colour with no new bake |
| `live` | Not baked, apart from what it bounces, so that it can go anywhere and cast shadows as it goes |
| `arc` | Its shadows baked at 32 angles along an arc through the day, so that it moves along it and they go with it, with no new bake and no maps of them to draw. It needs a day, and a final bake. Until there is one, it's `baked` |

A live sun lights surfaces and their highlights, and models that can see the sky towards it. Its shadows come from three maps round the view, the nearest finest. When it sets it's the moon, from the far side of the sky, if the sky has one:

| Field | Means |
|---|---|
| `moon_color` | Its colour, which is its light as well. Black, the usual, is no moon |
| `moon_size` | How large it looks |

`sun_toward x y z` at the console moves it, to try a level, and on its own puts it back.

### A sun on an arc

`sun_arc = 8 14` says which hours the arc goes through, from 6 to 18. All of a day, sunrise to sunset, unless it says. The moon's across the sky from the sun, so at night it goes along the same arc, twelve hours on. A narrower arc has its angles closer together: all of a day is an angle each 5.8 degrees, and 8 to 14 one each 2.9.

Between two angles, a shadow is between what they say, and between two samples, its edge is where their blend crosses the middle, so it moves smoothly. Its edges are as fine as the level's light: a finer `light_spacing` makes them finer. What's smaller than an angle's slice of the arc doesn't come and go between angles, and is left out.

Models take it as a live sun, from whether they can see the sky towards it. Off its arc, which is only where `sun_toward` puts it, its shadows are drawn as a live one's. What it bounces is baked once, from the sky's `sun_direction`, as a baked or live sun's is.

## A day

A level has a day if its world says when it starts or how long it takes:

| Field | Means |
|---|---|
| `day_start` | The hour a level starts at, from 0 to 24. Noon unless it says |
| `day_length` | How many seconds a whole day takes. 0, the usual, is a day that stands still |
| `sun_heading` | Which way the sun comes up, in degrees round from `+x` |
| `sun_height` | How high it gets at noon, in degrees |
| `light_set` | The set of light the whole level has. Without one, it has one made from its sky |

The sun comes up at six, is highest at noon and goes down at six, and the moon has the night. Everyone in a game has the same time, which the server keeps. `time_of_day 18` at the console sets it, as do `engine.set_time_of_day(18)` and `engine.time_of_day()` in a game's scripts.

A sky has `stars`, which show only where it's dark, from 0 for none.

### Sets of light

```text
light_set autumn {
    key 6 { zenith = 0.3 0.2 0.3 horizon = 1 0.6 0.4 sun = 1 0.55 0.3 }
    key 12 { zenith = 0.1 0.3 0.6 sun = 1 0.95 0.85 fog = 0.6 0.65 0.7 }
    key 20 { zenith = 0.02 0.03 0.08 sun = 0 0 0 clouds = 0.1 ambient = 0.02 }
}
```

A key is named by its hour, and keys go round, so what's after the last blends into the first. What a key leaves out comes from the keys that have it, and what no key has from the sky. One number is all three colours.

| In a key | Means |
|---|---|
| `zenith`, `horizon`, `ground` | The sky's colours |
| `sun`, `moon` | Their colours, and their light |
| `clouds` | What the clouds' colours are multiplied by |
| `fog` | Fog's colour |
| `water` | What water's colour is multiplied by |
| `ambient` | Light added everywhere, on the scale of a sample of baked light |
| `shine` | What highlights are multiplied by |

A zone can have its own with `light_set`, and `light_blend` says how far in it takes for it to be all there is, 256 unless it says. A zone inside another goes over it. A sun that's baked on its own layers, live or on an arc goes with the day, and what it bounced goes down with it. A fixed sun can't, as it's baked in with everything else.

## How light comes out

These go in the world, not the sky. See [proposal 0150](proposals/0150-light-on-screen.md).

```text
entity world {
    type = worldspawn
    tone = film
    exposure = 1.2
}
```

| Field | Means |
|---|---|
| `exposure` | How much light reaches the eye |
| `light_curve` | What baked light is raised to. Higher makes dim light dimmer |
| `tone` | How what's too bright comes down, below |
| `ssao` | How much corners and creases darken, from 0 for none to 1. 0.6 unless a level says. What's carried over from the fixture has none |
| `ssao_radius` | How far round a point that looks, in world units. 32 unless it says |
| `exposure_range` | How far exposure goes by itself, down and up in stops from `exposure`, as eyes do going from the dark into the sun: `exposure_range = -1 2`. None unless it says, as levels are built by eye |
| `exposure_speed` | How fast it goes, in stops a second, into light and then into the dark. `3 1` unless it says |
| `bloom` | How much what's brighter than a screen shows glows, from 0 for none. 1 unless it says. What's carried over from the fixture has none |
| `bloom_threshold` | How bright light has to be, once exposed, to glow. 1, the brightest a screen shows, unless it says |

| `tone` | Means |
|---|---|
| `neutral` | What's below 0.8 stays as it is, and what's above rolls off and goes towards white. It's the usual |
| `film` | A film's curve all the way up, with more contrast and a look of its own. It changes all of a level |
| `hold` | The top is the top, and anything brighter looks the same |

### Corners

Darkening in corners is on light that comes from all round: baked light, and the light a day adds everywhere. Light from one way, which is a sun's own, baked, live or on an arc, and moving lights, has shadows of its own and is left alone, as is fog. A fixed sun is baked in with everything else, so it's darkened with the rest.

It's worked out from what's solid before anything's shaded. What's see-through, cut out, or moved by its shader isn't in that, so it isn't darkened, and nor are models, which have their capsules' shade.

A player sets `r_ssao` to 0 for none, 1 for some, which is the usual, or 2 for more and smoother.

### Bloom

What's brighter than a screen shows glows, blurred wide, so the sun and bright lamps do and the rest is as it was. A player can turn it off with `r_bloom 0`.

### Exposure that goes by itself

What's seen is measured, the middle of the view most, leaving out the darkest and brightest tenth, and exposure goes towards middle grey as far as `exposure_range` lets it. A level starts where it should be. A zone can have its own `exposure_range`, and its light still multiplies after. `exposure` at the console says how bright what's seen is and how far exposure's gone, to tune a range by. A player can turn it off with `r_exposure_auto 0`.

