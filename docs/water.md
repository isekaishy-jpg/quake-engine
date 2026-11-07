# Water

Any brush with `contents = water`, `slime` or `lava` is a body of liquid. A level says how its water looks. See [proposal 0115](proposals/0115-water.md).

```text
water {
    color = [0.03 0.07 0.08]
    absorption = 0.006
    caustics = 0.3
    reflect = 0.6
    waves = 1
    refraction = 1
}
```

| Field | Means |
|---|---|
| `color` | What depth tints things towards |
| `absorption` | How fast it does, per unit. Higher is murkier |
| `caustics` | How strong the light on what's under it, and shafts through it, are |
| `reflect` | How much its surface mirrors, at a glance |
| `waves` | How tall its waves look |
| `refraction` | How far its surface bends what's seen through it |
| `tint`, `tint_strength` | The colour over the view with your eyes in it, and how strong, from 0 to 1. With no `tint` it's `color`. Cheap, and goes well with fog and particles, so that none has to do it all |

| `deep` | How much thicker it gets with each unit of depth, so that a deep pool goes dark towards its bottom |
| `view_effect` | An [effect](effects.md) that goes round the view with your eyes in it, such as `underwater_motes` |

Each setting left out keeps what that liquid usually has. Water is clear and mirrors, slime is thick and dull, and lava mirrors nothing.

## More than one

A level can have as many as it wants, each with a name, and a liquid brush says which it is:

```text
water canal { color = [0.05 0.3 0.35] absorption = 0.004 }
water bog { color = [0.2 0.25 0.05] absorption = 0.03 waves = 0.3 }
brush { material = bog_top contents = water water = bog cuboid = 150 -200 -30 400 200 -2 }
```

A `water` with no name goes for liquid brushes that don't say. A name no block has is an error.

## Any shape

A mesh can be the top of a body of water, so that a river can run downhill. Say so where it's placed, and it's no longer drawn as a mesh:

```text
water stream { color = [0.03 0.25 0.32] absorption = 0.012 depth = 400 cell = 16 }
mesh { source = river water = stream }
```

It's seen, swum in and looked out of as a pool is. `depth` is how far down it goes from its surface, and `cell` how finely its shape is followed, in units. `contents` says if it's `slime` or `lava`. What's under it is a grid of heights, so it can't overhang, and where it falls it wants a `waterfall`.

The 8 bodies nearest the eye show, pools and shaped ones alike.

## On land

Land can have water on it. In the editor the `flood` land tool fills what's under the brush up to where you first pressed, which makes a lake. The `water` and `dry` tools paint how deep it is, for what runs with the land, and `water_terrain` does either from a tool. The land says which water it is:

```text
terrain { source = hills material = ground water = lake }
```

The land hides what's under it, so the shore is exactly where the two meet, and a pond in a dip is as wide as the dip.

## What's on it, and how it meets the shore

```text
water bog {
    foam = 14
    foam_image = textures/fx_puff.png
    foam_size = 128
    shore = 8
    overlay { image = textures/scum.png size = 300 flow = 6 2 strength = 0.5 }
}
```

| Field | Means |
|---|---|
| `foam` | How deep it can be and still have foam, in units. It goes by how deep it is under the surface, so it follows any shore |
| `foam_image`, `foam_size` | What foam looks like, by the picture's alpha, and how wide that is. With none it has a noise of its own |
| `shore` | How deep it has to be before it stops fading into what's under it, so that the edge doesn't show as a line |
| `overlay` | A picture over its surface, by its alpha. A water can have two |
| `size` | How wide an overlay's picture is, in units |
| `flow` | Which way it goes, in units a second |
| `strength` | How much of it shows, from 0 to 1 |

Depth is only known where the view can see what's under the water, so an edge that's off the screen has no foam.

## What goes through it

Ripples go out over water near the view, about 500 units each way, and they're seen in what it mirrors and bends. Anything moving through a surface leaves a wake, shots and what falls in leave rings, and these come by themselves. A level can say more:

| Field | Means |
|---|---|
| `splash`, `splash_sound` | An [effect](effects.md) and a sound for something going in |
| `spray` | An effect for something going fast through it |
| `trail` | A picture to push into the water behind what's going through it, with its point forward. `textures/fx_wake.png` is a V |
| `trail_size` | How far that reaches |
| `rain_rings` | How many rings a second rain makes on it, near the view. It's for you to say, as nothing knows it's raining |

## A murky look

Quake's water is a flat warped picture you can hardly see through. For that, turn the rest off, and use a liquid with no material, which swims as the original's does:

```text
water { absorption = 0.03 caustics = 0 reflect = 0 waves = 0 refraction = 0 }
```

## Its surface

The surface can have a material with a [shader](shaders.md), such as `ripple`, and blends as any material does.

## Effects

`effects/` has `water_splash`, `bubbles`, `underwater_motes` and `waterfall`. Place them as any [effect](effects.md) is. A fall of water is a `waterfall` at its top, with a mesh behind it if it wants a body.
