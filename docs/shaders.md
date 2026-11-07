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

## Graphs

A graph is a shader made of nodes. It's `shaders/<name>.egraph`, and a material names it as it would a shader. If a `.wgsl` of the same name is there, that goes ahead of it, so you can take the code a graph made and go on by hand.

```text
format 1 graph
node { id = time kind = time }
node { id = wave kind = sin a = time }
outputs { emissive = wave }
```

- **In the editor,** the **Graph** tab has the kinds to add, the boxes under the view, and the code it makes. Drag from a box's output on its right to an input on the left. Click an input to take what's joined to it off, and drag a box by its title. **Save** builds the map again, so what's in the level is the preview.
- **With tools,** `graph_kinds` lists what there is, `write_graph` writes one and refuses one that's wrong, and `describe_graph` says what it comes to.
- **Outputs** say where a node lands: `vertex` where corners go, `uv` where pictures are read, `base`, `normal` and `emissive` what a surface is, and `color` a level's final pass, which goes alone. `vertex` and `uv` are the new place, not how far it's moved.
- **Types** go wider and narrower by themselves. A number fills what it goes into, and more goes narrower by dropping its end.
- **An input left out** has a default, and a picture left with no place reads where the pixel is.
- **`code`** takes an expression in WGSL with inputs `a` to `d`, for what the other kinds can't do. Say what it `gives`: `number`, `vec2`, `vec3` or `vec4`.

A graph has no branches. `step` and `mix` do most of that, and `code` the rest. One that's wrong shows green, and the console says which node and why.

## Checking

```bash
target/release/engine-tool content - check_shaders "{}"
```

A shader that's missing or won't compile shows bright green in the game, and says why in the console. `shaders/` holds a default set to start from: `ripple`, `sway`, `pulse`, `ghost` and `dusk`.

The engine's own shaders for old pictures with no material are `classic`, `classic_cutout`, `classic_liquid` and `classic_sky`. A file of the same name in `shaders/` replaces one.

## In a finished game

Shaders are compiled ahead of time, into `compiled/shaders`, and packing does it. While you work, what isn't compiled is compiled as it's needed. A finished game is built without a compiler:

```bash
cargo build --release -p engine_app --no-default-features
```

It reads only what's in its `common.pkt`. A shader that doesn't compile stops a pack. See [proposal 0133](proposals/0133-precompiled-shaders.md).

`in.color` is what a mesh's corners' colours say there, which is white on what has none, so a surface shader can use them to mask something.

