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
