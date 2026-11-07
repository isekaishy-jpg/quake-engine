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
| `kind` | `sprite`, `square`, `ribbon`, `beam`, `light`, `decal` or `mesh` |
| `blend` | `over`, as smoke, or `add`, as fire |
| `rate`, `burst` | How many a second, and how many at once when it starts |
| `duration`, `delay` | How long it runs, 0 for ever, and how long it waits |
| `shape` | `point`, `ball`, `ring` and `disc` with `radius`, `cone` with `radius` and `height`, `box` and `line` with `half_size` |
| `mesh`, `surface` | The mesh each one is, with `kind = mesh`. It's what they start on with `shape = mesh`, unless `surface` names another |
| `life`, `speed` | One number, or the lowest and highest |
| `direction`, `spread` | Which way they head, and how far from it, in degrees |
| `gravity`, `drag`, `flat_drag`, `turbulence` | What moves them. `flat_drag` is sideways only |
| `size` | One number, or pairs of when and how big, from 0 to 1 of a life |
| `color` | Red, green, blue and alpha, or a when before each four. Past 1 glows |
| `reach` | How far a light reaches, or a beam goes |
| `gpu` | `true` or `sim` for great numbers. See below |
| `motion`, `field` | With `gpu = sim`, a shader that moves them, and a field it can read. See below |
| `on_death` | An emitter that starts where each one ends, by its `burst` |
| `image` | A picture, in place of a soft dot. Its alpha is what shows |
| `flipbook`, `flip_rate` | The columns and rows of frames in `image`, and frames a second, or 0 to go through them over a life |
| `shader` | A shader of its own, with `shader_particle` |
| `rotation`, `spin` | How far it starts turned, and how fast it turns, in degrees. A low and a high number |
| `stretch` | Draws it out along how it moves, this long for each unit a second |
| `soft` | How far it fades into what's behind it, in world units. Smoke wants 30 or so |
| `lit` | `true` to take light from the level, which is what smoke wants and fire doesn't |
| `collide` | `bounce`, `stick` or `die` when it hits something, with `bounce` for how much speed it keeps |
| `on_hit` | An emitter that starts where it hits, by its `burst` |
| `outward` | `true` to head away from the middle of its shape, for shock waves |
| `far` | How far from the view it thins out to nothing |

An emitter another starts doesn't run by itself. That's how emitters join into a graph: sparks that burst into embers, or a shell that ends in a flash and smoke.

## Forces and timelines

An emitter can have `force` blocks:

```text
force { kind = attract at = 0 0 64 strength = 300 reach = 400 }
force { kind = vortex axis = 0 0 1 strength = 200 inward = 40 }
force { kind = wind blowing = 80 0 0 gusts = 0.6 }
```

Below 0, `attract` pushes away. `at` is from the effect's middle.

An effect can have `timeline { at = 0.5 start = sparks }`, which starts that emitter's `burst` at that time. An emitter that a timeline, `on_hit` or `on_death` starts doesn't run by itself.

## From a script

```lua
local fire = engine.effect("campfire", origin, nil, { color = { 0.4, 0.6, 1 }, size = 1.5, strength = 2 })
engine.effect_stop(fire, 2)
```

`engine.effect` gives back a handle. `engine.effect_stop` ends it: it makes no more, and what it's made fades over that many seconds. Its last argument is what this one is of its own: `color` multiplies its colour, `size` its size, and `strength` how many it makes.

## How many

`r_effect_budget` is how many particles can be going, up to 8192. Past about half of it, what's far from the view makes less, so what's near keeps its look.

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

## Meshes

With `kind = mesh`, each particle is a mesh, which is how rocks and bits of wood come off an explosion:

```text
emitter rubble {
    kind = mesh
    mesh = stone_chip
    burst = 12
    speed = 200 500
    gravity = 1
    spin = 90 400
    size = 0.8 0.3
    collide = bounce
}
```

`size` is how big it is, with 1 as it was made, and it can go down to nothing, as a mesh has no alpha to fade by. `spin` turns it, each about a way of its own. `color` tints its light. It's drawn as a moving mesh, in its rest pose, so a level can have up to about 4,000 of those and things that move, all told. A level loads what its effects are made of by itself.

With `shape = mesh`, they start on a mesh's surface, evenly over it, as it stands at rest. With `outward`, they go the way the surface faces there. It's for something's whole shape catching fire or dissolving, so start the effect following it.

## Great numbers

There are two ways.

With `gpu = true`, the GPU works every particle out from its number and the time, so 100,000 cost the CPU nothing. Nothing is kept from frame to frame, so what follows from what came before can't be done: no `drag`, no `on_death`, no curve past its two ends, and only `sprite`. Use it for dust that hangs in the air.

With `gpu = sim`, compute runs them. Each emitter has a ring of slots, and each frame steps all of them, so a particle goes on from where it was. It has everything an emitter says: gravity, drag, turbulence, forces, every shape, meshes' surfaces among them, `outward`, `image` and its frames, `rotation`, `spin`, `stretch`, `soft`, `shader`, `far`, and what an instance says of itself. `size` and `color` are read at eight steps along a life. Use it for rain, snow, and anything that wants a lot of what moves.

Three things differ.

- `collide` hits what the view saw last frame, not the world, so it's free, and what can't be seen can't be hit. `stick`, `bounce` and `die` all work.
- `lit` takes one light, from where the effect is.
- An effect file that asks for another `kind`, `on_death` or `on_hit` is an error, as nothing hears of what ends in compute. An emitter that another starts runs in the usual way, as it starts where that one was.

`motion = name` adds a shader of your own to the step:

```wgsl
fn shader_motion(particle: SimParticle, seconds: f32) -> SimParticle {
    var out = particle;
    let flow = field_at(fract(out.place.xy / 512.0));
    out.moving = vec4<f32>(out.moving.xyz + flow.xyz * seconds, out.moving.w);
    return out;
}
```

`place` is where it is, with its age in `w`, and `moving` how it's going, with how long it has in `w`. `field_at` reads the level's field named by `field`, from 0 to 1 across it. A level without that field reads 0. `shader_time` is there too. A motion that won't compile says so, and they move as if it wasn't there.

100,000 at once cost about 0.2 ms a frame to step and a fraction of a millisecond to draw. A million cost about 1.5 ms to step. A level can have 2 million slots and 1,024 emitters going.

## Changing one while the game runs

Save an effect file, a picture it shows or a shader it names, and the game reads them all again within a second. What's going starts over as the file now says, and particles already made stay where they are and take up what their emitter now says. Those of an emitter that's gone go too. A file that's wrong leaves things as they were, and the console says why.

## Checking

```bash
target/release/engine-tool content - check_effects "{}"
```

## What's there to start from

`effects/` has these. Copy one and change it, or start them as they are.

| Effect | What it is | How it's meant to go |
|---|---|---|
| `explosion`, `explosion_small` | A fireball, a ring, sparks that come off things, smoke, and a flash | Once, where something goes up |
| `spell_charge` | Light drawn in to a point | Going, until it's stopped |
| `spell_bolt` | A bright head, a tail and motes | Following what's thrown |
| `spell_burst` | A star, a ring, and motes flung out | Once, where a spell lands |
| `spell_ward` | A turning circle of runes on the floor | Going, a little above the floor |
| `spell_heal` | Light and small stars rising | Following who's being mended |
| `rain` | 48,000 drops, in compute | At the top of what it falls through |
| `snow` | Flakes on a wind that settle, in compute | At the top of what it falls through |
| `ash` | Ash and embers going by, in compute | In the middle of the air they fill |
| `fog_wisps` | Low fog that drifts | On the floor |
| `sparks_hit` | A short spray and a flash | Once, where metal's struck |
| `sparks_fountain` | A shower that comes off the floor | Going, from what's broken |
| `embers`, `torch`, `campfire` | What's burning | Going |
| `smoke`, `dust`, `bubbles`, `water_splash`, `waterfall` | What their names say | Going |
| `blood_hit` | Drops and a dark puff | Once, on a wound |

Their pictures are `textures/fx_*.png`, which `tools/make_effect_pictures.py` makes from noise.
