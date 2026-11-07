# Fog

A level can have fog all over, and fog in places. See [proposal 0147](proposals/0147-fog-and-water.md).

## All over

```text
fog { color = [0.5 0.55 0.6] density = 0.002 height = 0 falloff = 0.01 }
```

It thickens with distance, and it lies low: `height` is where it's thickest, and `falloff` how fast it thins going up. Where a level has a grid of probes it takes their light.

## In a place

```text
fog_volume {
    low = -300 -400 0
    high = 90 400 200
    color = 0.55 0.6 0.65
    density = 0.012
    edge = 40
    wisps = 0.8
    wisp_size = 90
    drift = 30 8 0
    lit = true
}
```

| Field | Means |
|---|---|
| `low`, `high` | Two corners of its box |
| `color` | Its colour |
| `density` | How thick it is. 0.01 hides things in about 300 units |
| `edge` | How far in from its sides it takes to get that thick. 0 is a hard side |
| `wisps` | How much of it is wisps, from 0 for even fog to 1 for gaps between them |
| `wisp_size` | How big they are, in units |
| `drift` | Which way they go, in units a second |
| `lit` | `true` to be lit by the level and what moves through it, and dark otherwise. Without it, it's its own colour everywhere |

The 8 nearest the eye show, and the 4 moving lights nearest it light fog. Fog stops at water.

## What it costs

Even fog with a hard side is free. Anything else is worked out at a quarter of the pixels, at about 0.4 ms a frame at 1280 by 720 for each one that fills the view. `gpu_times` shows it as `air`.
