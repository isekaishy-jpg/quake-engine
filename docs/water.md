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

Each setting left out keeps what that liquid usually has. Water is clear and mirrors, slime is thick and dull, and lava mirrors nothing.

## A murky look

Quake's water is a flat warped picture you can hardly see through. For that, turn the rest off, and use a liquid with no material, which swims as the original's does:

```text
water { absorption = 0.03 caustics = 0 reflect = 0 waves = 0 refraction = 0 }
```

## Its surface

The surface can have a material with a [shader](shaders.md), such as `ripple`, and blends as any material does.

## Effects

`effects/` has `water_splash`, `bubbles` and `waterfall`. Place them as any [effect](effects.md) is. A fall of water is a `waterfall` at its top, with a mesh behind it if it wants a body.
