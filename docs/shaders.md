# Writing a shader

A shader is a small WGSL file in `shaders/`. It replaces part of how a surface is drawn, and what it leaves alone goes on working: light, bumps, shadows and fog all still happen. See [proposal 0107](proposals/0107-shaders.md).

## Using one

```text
format 1 material

surface {
    base = textures/uc01_blight_sewers_wall_blocks.png
    shader = pulse
    shader_params = 0.8 2.0
}
```

`shader_params` is up to eight numbers. `shader_image_a` and `shader_image_b` are up to two more pictures, read as measures, not colours.

A material that only differs a little is an instance of another:

```text
format 1 material

surface {
    parent = glowing_wall
    shader_params = 1.5 6.0
}
```

It has everything its parent has, except what it says itself. Layers come from the parent unless it has its own.

## What a shader can replace

Write any of these. The rest stay as they are.

| Function | What it does |
|---|---|
| `fn shader_vertex(world: vec3<f32>) -> vec3<f32>` | Moves a surface's corners |
| `fn shader_uv(uv: vec2<f32>, world: vec3<f32>) -> vec2<f32>` | Moves where it reads its pictures |
| `fn shader_surface(surface: Surface) -> Surface` | Changes `base`, `normal` and `emissive`, ahead of its light |
| `fn shader_whole(in: VertexOut) -> vec4<f32>` | Draws the whole surface itself, which is for rare cases |

For a level's final pass, set `post_shader = <name>` on `worldspawn` and write:

| Function | What it does |
|---|---|
| `fn shader_post(color: vec3<f32>, uv: vec2<f32>) -> vec3<f32>` | Changes the picture, in linear light |

## What a shader can use

- `shader_param(index)`: one of the material's numbers, from 0.
- `shader_image(index, uv)`: one of its pictures, from 0.
- `shader_time()`: seconds since the level began.
- In `shader_whole`, also `light(in)` for the light here, `read_slot` for pictures, `scene_color` to finish a colour, and `surface` for the base picture.

`shader_image` has to be called where every pixel gets to it, not inside an `if`.

A surface that moves its corners or uses `discard` doesn't go through the depth pass, so it costs more than one that doesn't.

## Checking

```bash
target/release/engine-tool content - check_shaders "{}"
```

A shader that's missing or won't compile shows bright green in the game, and says why in the console. `shaders/` holds a default set to start from: `ripple`, `sway`, `pulse`, `ghost` and `dusk`.

The engine's own shaders for old pictures with no material are `classic`, `classic_cutout`, `classic_liquid` and `classic_sky`. A file of the same name in `shaders/` replaces one.
