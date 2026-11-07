# Writing an effect

An effect is a file in `effects/`, with a stack of settings for each of its emitters. See [proposal 0114](proposals/0114-effects.md).

```text
format 1 effect

emitter flame {
    blend = add
    rate = 40
    shape = ball
    radius = 10
    life = 0.5 1.1
    speed = 30 60
    spread = 20
    turbulence = 30
    size = 0 10 0.6 16 1 2
    color = 0 1.2 0.5 0.1 0.5 0.5 0.9 0.3 0.05 0.3 1 0.3 0.05 0 0
}
```

## Using one

- In a level: `entity fire { type = effect effect = campfire origin = 0 0 8 }`.
- From a script: `engine.effect("campfire", origin)`, or `engine.effect("campfire", origin, id)` to follow an entity.

## What an emitter says

| Field | Means |
|---|---|
| `kind` | `sprite`, `ribbon`, `beam`, `light` or `decal` |
| `blend` | `over`, as smoke, or `add`, as fire |
| `rate`, `burst` | How many a second, and how many at once when it starts |
| `duration`, `delay` | How long it runs, 0 for ever, and how long it waits |
| `shape` | `point`, `ball` with `radius`, or `box` with `half_size` |
| `life`, `speed` | One number, or the lowest and highest |
| `direction`, `spread` | Which way they head, and how far from it, in degrees |
| `gravity`, `drag`, `turbulence` | What moves them |
| `size` | One number, or pairs of when and how big, from 0 to 1 of a life |
| `color` | Red, green, blue and alpha, or a when before each four. Past 1 glows |
| `reach` | How far a light reaches, or a beam goes |
| `gpu` | `true` for great numbers. See below |
| `on_death` | An emitter that starts where each one ends, by its `burst` |
| `image` | A picture, in place of a soft dot. Its alpha is what shows |
| `flipbook`, `flip_rate` | The columns and rows of frames in `image`, and frames a second, or 0 to go through them over a life |
| `shader` | A shader of its own, with `shader_particle` |
| `rotation`, `spin` | How far it starts turned, and how fast it turns, in degrees. A low and a high number |
| `stretch` | Draws it out along how it moves, this long for each unit a second |
| `soft` | How far it fades into what's behind it, in world units. Smoke wants 30 or so |
| `lit` | `true` to take light from the level, which is what smoke wants and fire doesn't |

An emitter another starts doesn't run by itself. That's how emitters join into a graph: sparks that burst into embers, or a shell that ends in a flash and smoke.

## Pictures

`image` can be any picture, and a level's all come to one size, up to 512. `tools/make_effect_pictures.py` makes those that are in `textures/fx_*`. A sheet of frames goes left to right and then down. Between two frames it's a mix of them, so it doesn't step.

Everything's drawn far to near in one list, so what covers and what adds sort together. What's behind glass goes in ahead of the glass.

A shader's one function:

```wgsl
fn shader_particle(look: ParticleLook) -> vec4<f32> {
    return look.color * vec4<f32>(1.0, 1.0, 1.0, 1.0 - look.life);
}
```

`look` has `color`, with its picture, `uv`, `life` from 0 to 1, and `world`. `shader_time` and `shader_noise` are there. A graph's output `particle` does the same, with `tinted` and `life` to read.

## Great numbers

With `gpu = true`, the GPU works every particle out from its number and the time, so 100,000 cost the CPU nothing. Nothing is kept from frame to frame, so what follows from what came before can't be done: no `drag`, no `on_death`, no curve past its two ends, and only `sprite`. Use it for dust, snow and rain.

## Checking

```bash
target/release/engine-tool content - check_effects "{}"
```

`effects/` holds some to start from: `campfire`, `smoke` and `dust`.
